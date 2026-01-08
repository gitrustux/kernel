// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Hypervisor Trap Map
//!
//! This module provides trap mapping functionality for the hypervisor.
//! It handles memory and I/O traps, allowing the hypervisor to intercept
//! and handle guest accesses to specific address ranges.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Maximum number of port packets per range
const MAX_PACKETS_PER_RANGE: usize = 256;

/// Guest trap kind: Memory-mapped I/O trap
pub const ZX_GUEST_TRAP_MEM: u32 = 1;

/// Guest trap kind: Bell trap for notification
pub const ZX_GUEST_TRAP_BELL: u32 = 2;

/// Guest trap kind: I/O port trap (x86 only)
pub const ZX_GUEST_TRAP_IO: u32 = 3;

/// Guest physical address type
pub type GuestPaddr = u64;

/// Port packet for hypervisor events
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PortPacket {
    /// Packet data
    pub packet: Packet,
    /// Allocator reference
    allocator: Option<*mut BlockingPortAllocator>,
}

unsafe impl Send for PortPacket {}
unsafe impl Sync for PortPacket {}

/// Port packet data
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Packet {
    /// Header data
    pub key: u64,
    pub type_: u32,
    pub status: i32,
    /// Packet payload
    pub payload: [u8; 32],
}

/// Blocking port allocator for managing port packets
pub struct BlockingPortAllocator {
    /// Semaphore for blocking allocation
    semaphore: Semaphore,
    /// Arena for packet allocation
    arena: Arena,
}

unsafe impl Send for BlockingPortAllocator {}
unsafe impl Sync for BlockingPortAllocator {}

impl BlockingPortAllocator {
    /// Create a new blocking port allocator
    pub fn new() -> Self {
        println!("BlockingPortAllocator: Creating");

        Self {
            semaphore: Semaphore::new(MAX_PACKETS_PER_RANGE),
            arena: Arena::new("hypervisor-packets", MAX_PACKETS_PER_RANGE),
        }
    }

    /// Initialize the allocator
    pub fn init(&mut self) -> Result<(), i32> {
        self.arena.init()
    }

    /// Allocate a port packet (blocking)
    pub fn alloc_blocking(&self) -> Option<*mut PortPacket> {
        ktrace_vcpu(TAG_VCPU_BLOCK, VCPU_PORT);

        let result = self.semaphore.wait();

        ktrace_vcpu(TAG_VCPU_UNBLOCK, VCPU_PORT);

        if result.is_err() {
            return None;
        }

        self.alloc()
    }

    /// Allocate a port packet (non-blocking)
    pub fn alloc(&self) -> Option<*mut PortPacket> {
        // TODO: Allocate from arena
        None
    }

    /// Free a port packet
    pub fn free(&self, port_packet: *mut PortPacket) {
        // TODO: Delete from arena
        self.semaphore.post();
    }
}

impl Default for BlockingPortAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Hypervisor trap
pub struct Trap {
    /// Trap kind
    kind: u32,
    /// Base address
    addr: GuestPaddr,
    /// Length of the trapped region
    len: usize,
    /// Port dispatcher for notifications
    port: Option<Arc<PortDispatcher>>,
    /// Key for port packets
    key: u64,
    /// Port allocator
    port_allocator: Mutex<BlockingPortAllocator>,
}

unsafe impl Send for Trap {}
unsafe impl Sync for Trap {}

impl Trap {
    /// Create a new trap
    pub fn new(
        kind: u32,
        addr: GuestPaddr,
        len: usize,
        port: Option<Arc<PortDispatcher>>,
        key: u64,
    ) -> Self {
        println!(
            "Trap: Creating trap kind {} at {:#x} ({} bytes, key: {})",
            kind, addr, len, key
        );

        Self {
            kind,
            addr,
            len,
            port,
            key,
            port_allocator: Mutex::new(BlockingPortAllocator::new()),
        }
    }

    /// Initialize the trap
    pub fn init(&mut self) -> Result<(), i32> {
        let mut allocator = self.port_allocator.lock();
        allocator.init()
    }

    /// Queue a packet to this trap
    pub fn queue(&self, packet: Packet, invalidator: Option<&StateInvalidator>) -> Result<(), i32> {
        if let Some(invalidator) = invalidator {
            invalidator.invalidate();
        }

        let port = self.port.as_ref().ok_or(-2)?; // ZX_ERR_NOT_FOUND

        let allocator = self.port_allocator.lock();
        let port_packet = allocator.alloc_blocking().ok_or(-1)?; // ZX_ERR_NO_MEMORY

        unsafe {
            // Set packet data
            (*port_packet).packet = packet;
        }

        // TODO: Queue to port dispatcher
        let _ = (port, port_packet);

        Ok(())
    }

    /// Check if an address is within this trap's range
    pub fn contains(&self, addr: GuestPaddr) -> bool {
        addr >= self.addr && addr < self.addr + self.len as u64
    }

    /// Get the trap kind
    pub fn kind(&self) -> u32 {
        self.kind
    }

    /// Get the base address
    pub fn addr(&self) -> GuestPaddr {
        self.addr
    }

    /// Get the length
    pub fn len(&self) -> usize {
        self.len
    }

    /// Get the key
    pub fn key(&self) -> u64 {
        self.key
    }
}

impl Drop for Trap {
    fn drop(&mut self) {
        if let Some(port) = &self.port {
            // TODO: Cancel queued packets
            let _ = port;
        }
    }
}

/// Trap map for managing multiple traps
pub struct TrapMap {
    /// Memory traps (includes bell traps)
    mem_traps: Mutex<BTreeMap<GuestPaddr, Trap>>,
    /// I/O traps (x86 only)
    #[cfg(target_arch = "x86_64")]
    io_traps: Mutex<BTreeMap<GuestPaddr, Trap>>,
}

unsafe impl Send for TrapMap {}
unsafe impl Sync for TrapMap {}

impl TrapMap {
    /// Create a new trap map
    pub fn new() -> Self {
        println!("TrapMap: Creating");

        Self {
            mem_traps: Mutex::new(BTreeMap::new()),
            #[cfg(target_arch = "x86_64")]
            io_traps: Mutex::new(BTreeMap::new()),
        }
    }

    /// Insert a trap into the map
    pub fn insert_trap(
        &mut self,
        kind: u32,
        addr: GuestPaddr,
        len: usize,
        port: Option<Arc<PortDispatcher>>,
        key: u64,
    ) -> Result<(), i32> {
        let traps = self.tree_of(kind).ok_or(-1)?; // ZX_ERR_INVALID_ARGS

        // Check for overlapping trap
        if let Some(existing) = traps.get(&addr) {
            println!(
                "Trap: Trap for kind {} (addr {:#x} len {} key {}) already exists (addr {:#x} len {} key {})",
                kind, addr, len, key, existing.addr(), existing.len(), existing.key()
            );
            return Err(-3); // ZX_ERR_ALREADY_EXISTS
        }

        let mut trap = Trap::new(kind, addr, len, port, key);
        trap.init()?;

        let mut traps = traps.lock();
        traps.insert(addr, trap);

        println!("TrapMap: Inserted trap kind {} at {:#x}", kind, addr);
        Ok(())
    }

    /// Find a trap for a given address
    pub fn find_trap(&self, kind: u32, addr: GuestPaddr) -> Result<*const Trap, i32> {
        let traps = self.tree_of(kind).ok_or(-1)?; // ZX_ERR_INVALID_ARGS

        let traps = traps.lock();
        let trap = traps
            .range(..=addr)
            .next_back()
            .filter(|(_, t)| t.contains(addr))
            .map(|(_, t)| t as *const Trap)
            .ok_or(-2)?; // ZX_ERR_NOT_FOUND

        Ok(trap)
    }

    /// Get the trap tree for a given kind
    fn tree_of(&self, kind: u32) -> Option<&Mutex<BTreeMap<GuestPaddr, Trap>>> {
        match kind {
            ZX_GUEST_TRAP_BELL | ZX_GUEST_TRAP_MEM => Some(&self.mem_traps),
            #[cfg(target_arch = "x86_64")]
            ZX_GUEST_TRAP_IO => Some(&self.io_traps),
            _ => None,
        }
    }
}

impl Default for TrapMap {
    fn default() -> Self {
        Self::new()
    }
}

/// State invalidator callback trait
pub trait StateInvalidator {
    /// Invalidate state
    fn invalidate(&self);
}

/// Port dispatcher (opaque type)
#[repr(C)]
pub struct PortDispatcher {
    _private: [u8; 0],
}

/// Semaphore for blocking operations
struct Semaphore {
    count: AtomicUsize,
    max: usize,
}

impl Semaphore {
    fn new(max: usize) -> Self {
        Self {
            count: AtomicUsize::new(max),
            max,
        }
    }

    fn wait(&self) -> Result<(), i32> {
        // TODO: Implement blocking wait
        // For now, just check if count > 0
        if self.count.load(Ordering::Acquire) > 0 {
            self.count.fetch_sub(1, Ordering::AcqRel);
            Ok(())
        } else {
            Err(-1) // ZX_ERR_NO_RESOURCES
        }
    }

    fn post(&self) {
        let old = self.count.fetch_add(1, Ordering::AcqRel);
        if old >= self.max {
            // Count would exceed max, roll back
            self.count.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Arena for packet allocation
struct Arena {
    name: String,
    size: usize,
}

impl Arena {
    fn new(name: &str, size: usize) -> Self {
        Self {
            name: name.to_string(),
            size,
        }
    }

    fn init(&mut self) -> Result<(), i32> {
        println!("Arena: Initializing {} (size: {})", self.name, self.size);
        // TODO: Initialize arena
        Ok(())
    }
}

/// Ktrace tags for VCPU events
const TAG_VCPU_BLOCK: u32 = 1;
const TAG_VCPU_UNBLOCK: u32 = 2;
const VCPU_PORT: u32 = 0;

/// Ktrace VCPU event
fn ktrace_vcpu(tag: u32, port: u32) {
    // TODO: Implement ktrace
    let _ = (tag, port);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trap_contains() {
        let trap = Trap::new(ZX_GUEST_TRAP_MEM, 0x1000, 0x1000, None, 0);
        assert!(trap.contains(0x1000));
        assert!(trap.contains(0x1fff));
        assert!(!trap.contains(0xfff));
        assert!(!trap.contains(0x2000));
    }

    #[test]
    fn test_trap_map_new() {
        let map = TrapMap::new();
        // Basic creation test
        let _ = map;
    }

    #[test]
    fn test_semaphore() {
        let sem = Semaphore::new(2);
        assert!(sem.wait().is_ok());
        assert!(sem.wait().is_ok());
        assert!(sem.wait().is_err());
        sem.post();
        assert!(sem.wait().is_ok());
    }
}
