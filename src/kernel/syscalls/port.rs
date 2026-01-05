// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Port System Calls
//!
//! This module implements the port-related system calls for inter-process
//! communication via packet queues.
//!
//! # Syscalls Implemented
//!
//! - `rx_port_create` - Create a new port
//! - `rx_port_queue` - Queue a packet to a port
//! - `rx_port_wait` - Wait for a packet from a port
//! - `rx_port_cancel` - Cancel a pending packet
//!
//! # Design
//!
//! - Ports are packet queues for IPC
//! - Support for asynchronous notification
//! - Deadline-based waiting
//! - Key-based packet cancellation

#![no_std]

use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::sync::Mutex;
use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use crate::kernel::sync::spin::SpinMutex;

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Port Packet Structure
/// ============================================================================

/// Port packet type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// User-defined packet
    User = 0,

    /// Signal packet
    Signal = 1,

    /// Exception packet
    Exception = 2,

    /// Guest bell packet
    GuestBell = 3,

    /// Guest VCPU packet
    GuestVcpu = 4,

    /// Single-shot event
    EventSingle = 5,

    /// Event pair
    EventPair = 6,
}

/// Port packet
#[repr(C)]
#[derive(Debug, Clone)]
pub struct PortPacket {
    /// Packet key
    pub key: u64,

    /// Packet type
    pub packet_type: PacketType,

    /// Packet status
    pub status: i32,

    /// Union payload
    pub payload: PacketPayload,
}

/// Port packet payload
#[repr(C)]
pub union PacketPayload {
    /// User data
    pub user: UserData,

    /// Signal data
    pub signal: SignalData,

    /// Exception data
    pub exception: ExceptionData,

    /// Raw bytes
    pub bytes: [u8; 32],
}

/// User packet data
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UserData {
    /// Data field 1
    pub u64: u64,

    /// Data field 2
    pub u64_2: u64,

    /// Data field 3
    pub u64_3: u64,

    /// Data field 4
    pub u64_4: u64,
}

/// Signal packet data
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SignalData {
    /// Trigger count
    pub count: u64,

    /// Reserved
    pub reserved: u64,

    /// Signals observed
    pub signals: u64,

    /// Timestamp
    pub timestamp: u64,
}

/// Exception packet data
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ExceptionData {
    /// PID
    pub pid: u64,

    /// TID
    pub tid: u64,

    /// Reserved
    pub reserved: u64,

    /// Timestamp
    pub timestamp: u64,
}

/// ============================================================================
/// Port Options
/// ============================================================================

/// Port options
pub mod port_options {
    /// No special options
    pub const NONE: u32 = 0x00;
}

/// ============================================================================
/// Port Registry
/// ============================================================================

/// Maximum number of ports in the system
const MAX_PORTS: usize = 16384;

/// Port entry
struct PortEntry {
    /// Port ID
    id: u64,

    /// Packet queue
    packets: Mutex<VecDeque<PortPacket>>,

    /// Waiters count
    waiters: AtomicUsize,
}

impl PortEntry {
    /// Create a new port entry
    pub fn new(id: u64) -> Self {
        Self {
            id,
            packets: Mutex::new(VecDeque::new()),
            waiters: AtomicUsize::new(0),
        }
    }
}

/// Global port registry
struct PortRegistry {
    /// Port entries
    entries: [Option<Arc<PortEntry>>; MAX_PORTS],

    /// Next port index to allocate
    next_index: AtomicUsize,

    /// Number of active ports
    count: AtomicUsize,
}

impl PortRegistry {
    /// Create a new port registry
    const fn new() -> Self {
        const INIT: Option<Arc<PortEntry>> = None;

        Self {
            entries: [INIT; MAX_PORTS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert a port into the registry
    pub fn insert(&mut self, id: u64, port: Arc<PortEntry>) -> Result {
        let idx = (id as usize) % MAX_PORTS;

        if self.entries[idx].is_none() {
            self.entries[idx] = Some(port);
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NO_RESOURCES)
        }
    }

    /// Get a port from the registry
    pub fn get(&self, id: u64) -> Option<Arc<PortEntry>> {
        let idx = (id as usize) % MAX_PORTS;
        self.entries[idx].as_ref().filter(|p| p.id == id).cloned()
    }

    /// Remove a port from the registry
    pub fn remove(&mut self, id: u64) -> Result {
        let idx = (id as usize) % MAX_PORTS;

        if self.entries[idx].is_some() {
            self.entries[idx] = None;
            self.count.fetch_sub(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NOT_FOUND)
        }
    }

    /// Get the number of active ports
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global port registry
static PORT_REGISTRY: PortRegistry = PortRegistry::new();

/// ============================================================================
/// Port ID Allocation
/// ============================================================================

/// Next port ID counter
static mut NEXT_PORT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new port ID
fn alloc_port_id() -> u64 {
    unsafe { NEXT_PORT_ID.fetch_add(1, Ordering::Relaxed) }
}

/// ============================================================================
/// Syscall: Port Create
/// ============================================================================

/// Create a new port syscall handler
///
/// # Arguments
///
/// * `options` - Creation options (must be 0)
///
/// # Returns
///
/// * On success: Port handle
/// * On error: Negative error code
pub fn sys_port_create_impl(options: u32) -> SyscallRet {
    log_debug!("sys_port_create: options={:#x}", options);

    // Validate options (must be 0)
    if options != port_options::NONE {
        log_error!("sys_port_create: invalid options {:#x}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Allocate new port ID
    let port_id = alloc_port_id();
    let port = Arc::new(PortEntry::new(port_id));

    // Insert into port registry
    if let Err(err) = PORT_REGISTRY.insert(port_id, port.clone()) {
        log_error!("sys_port_create: failed to insert port: {:?}", err);
        return err_to_ret(err);
    }

    log_debug!("sys_port_create: success port_id={}", port_id);

    // Pack result as handle value
    ok_to_ret(port_id as usize)
}

/// ============================================================================
/// Syscall: Port Queue
/// ============================================================================

/// Queue a packet to a port syscall handler
///
/// # Arguments
///
/// * `handle_val` - Port handle value
/// * `packet_in` - User pointer to packet
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_port_queue_impl(handle_val: u32, packet_in: usize) -> SyscallRet {
    log_debug!("sys_port_queue: handle={:#x} packet={:#x}", handle_val, packet_in);

    // Look up port
    let port_id = handle_val as u64;
    let port = match PORT_REGISTRY.get(port_id) {
        Some(p) => p,
        None => {
            log_error!("sys_port_queue: port not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Copy packet from user
    let mut packet = PortPacket {
        key: 0,
        packet_type: PacketType::User,
        status: 0,
        payload: PacketPayload { bytes: [0; 32] },
    };

    let user_ptr = UserPtr::<PortPacket>::new(packet_in);
    unsafe {
        if let Err(err) = copy_from_user(
            &mut packet as *mut PortPacket as *mut u8,
            user_ptr,
            core::mem::size_of::<PortPacket>(),
        ) {
            log_error!("sys_port_queue: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Queue the packet
    {
        let mut packets = port.packets.lock();
        packets.push_back(packet);
    }

    // Wake up waiters
    let waiters = port.waiters.load(Ordering::Relaxed);
    if waiters > 0 {
        // TODO: Implement proper waiter wakeup
        log_debug!("sys_port_queue: waking {} waiters", waiters);
    }

    log_debug!("sys_port_queue: success");

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Port Wait
/// ============================================================================

/// Wait for a packet from a port syscall handler
///
/// # Arguments
///
/// * `handle_val` - Port handle value
/// * `deadline` - Deadline for timeout (in nanoseconds)
/// * `packet_out` - User pointer to store received packet
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_port_wait_impl(handle_val: u32, deadline: u64, packet_out: usize) -> SyscallRet {
    log_debug!(
        "sys_port_wait: handle={:#x} deadline={} packet={:#x}",
        handle_val, deadline, packet_out
    );

    // Look up port
    let port_id = handle_val as u64;
    let port = match PORT_REGISTRY.get(port_id) {
        Some(p) => p,
        None => {
            log_error!("sys_port_wait: port not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Try to get a packet
    let packet = loop {
        let mut packets = port.packets.lock();

        if let Some(packet) = packets.pop_front() {
            break packet;
        }

        // No packet available - check deadline
        // TODO: Implement proper deadline checking
        // For now, just return timeout
        if deadline == 0 {
            log_debug!("sys_port_wait: no packet available");
            return err_to_ret(RX_ERR_TIMED_OUT);
        }

        // Register as waiter
        port.waiters.fetch_add(1, Ordering::Relaxed);
        drop(packets);

        // TODO: Implement proper waiting with condition variable
        // For now, simulate immediate timeout
        port.waiters.fetch_sub(1, Ordering::Relaxed);

        log_debug!("sys_port_wait: timeout");
        return err_to_ret(RX_ERR_TIMED_OUT);
    };

    // Copy packet to user
    let user_ptr = UserPtr::<PortPacket>::new(packet_out);
    unsafe {
        if let Err(err) = copy_to_user(
            user_ptr,
            &packet as *const PortPacket as *const u8,
            core::mem::size_of::<PortPacket>(),
        ) {
            log_error!("sys_port_wait: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_port_wait: success");

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Port Cancel
/// ============================================================================

/// Cancel a pending packet syscall handler
///
/// # Arguments
///
/// * `handle_val` - Port handle value
/// * `source_handle` - Source object handle
/// * `key` - Packet key to cancel
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_port_cancel_impl(handle_val: u32, source_handle: u32, key: u64) -> SyscallRet {
    log_debug!(
        "sys_port_cancel: handle={:#x} source={:#x} key={:#x}",
        handle_val, source_handle, key
    );

    // Look up port
    let port_id = handle_val as u64;
    let port = match PORT_REGISTRY.get(port_id) {
        Some(p) => p,
        None => {
            log_error!("sys_port_cancel: port not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // TODO: Validate source handle
    if source_handle == 0 {
        return err_to_ret(RX_ERR_BAD_HANDLE);
    }

    // Cancel packets with matching key
    let mut packets = port.packets.lock();
    let original_len = packets.len();

    packets.retain(|p| p.key != key);

    let removed = original_len - packets.len();

    if removed > 0 {
        log_debug!("sys_port_cancel: removed {} packets", removed);
        ok_to_ret(0)
    } else {
        log_debug!("sys_port_cancel: no matching packets found");
        err_to_ret(RX_ERR_NOT_FOUND)
    }
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get port subsystem statistics
pub fn get_stats() -> PortStats {
    PortStats {
        total_ports: PORT_REGISTRY.count(),
        total_queued: 0, // TODO: Track total queued packets
        total_waiters: 0, // TODO: Track total waiters
    }
}

/// Port subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PortStats {
    /// Total number of ports
    pub total_ports: usize,

    /// Total queued packets
    pub total_queued: u64,

    /// Total waiters
    pub total_waiters: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the port syscall subsystem
pub fn init() {
    log_info!("Port syscall subsystem initialized");
    log_info!("  Max ports: {}", MAX_PORTS);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_create() {
        let result = sys_port_create_impl(0);
        assert!(result >= 0);
    }

    #[test]
    fn test_port_create_invalid_options() {
        let result = sys_port_create_impl(0xFF);
        assert!(result < 0);
    }

    #[test]
    fn test_port_queue_invalid_handle() {
        let result = sys_port_queue_impl(0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_port_wait_invalid_handle() {
        let result = sys_port_wait_impl(0, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_port_cancel_invalid_handle() {
        let result = sys_port_cancel_impl(0, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_packet_size() {
        assert!(core::mem::size_of::<PortPacket>() >= 40);
    }
}
