// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Arena Allocator
//!
//! This module provides an arena-based memory allocator that manages
//! fixed-size allocations from a pre-allocated region. It uses virtual
//! memory mappings for efficient memory management.

#![no_std]

extern crate alloc;

use alloc::collections::LinkedList;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Page size (4KB)
const PAGE_SIZE: usize = 4096;

/// Pool commit increase size
const POOL_COMMIT_INCREASE: usize = 4 * PAGE_SIZE;

/// Pool decommit threshold
const POOL_DECOMMIT_THRESHOLD: usize = 8 * PAGE_SIZE;

/// Arena node for free list
#[repr(C)]
#[derive(Debug)]
pub struct ArenaNode {
    /// Slot pointer
    pub slot: *mut u8,
    /// Next node in free list
    pub next: Option<*mut ArenaNode>,
}

/// Memory pool within the arena
pub struct ArenaPool {
    /// Pool name
    pub name: &'static str,
    /// Slot size
    pub slot_size: usize,
    /// Start of pool
    pub start: *mut u8,
    /// End of pool
    pub end: *mut u8,
    /// Current top (next allocation)
    pub top: *mut u8,
    /// Committed pages end
    pub committed: *mut u8,
    /// Maximum committed
    pub committed_max: *mut u8,
}

unsafe impl Send for ArenaPool {}
unsafe impl Sync for ArenaPool {}

impl ArenaPool {
    /// Initialize a memory pool
    ///
    /// # Arguments
    ///
    /// * `name` - Pool name
    /// * `base` - Base address of the pool
    /// * `size` - Size of the pool in bytes
    /// * `slot_size` - Size of each slot
    pub fn init(&mut self, name: &'static str, base: *mut u8, size: usize, slot_size: usize) {
        self.name = name;
        self.slot_size = slot_size;
        self.start = base;
        self.end = unsafe { base.add(size) };
        self.top = base;
        self.committed = base;
        self.committed_max = base;
    }

    /// Allocate a slot from the pool
    ///
    /// # Returns
    ///
    /// Pointer to allocated slot, or null if pool is exhausted
    pub fn pop(&mut self) -> *mut u8 {
        let end = self.end as usize;
        let top = self.top as usize;
        let slot_size = self.slot_size;

        if end - top < slot_size {
            println!("{}: no room", self.name);
            return core::ptr::null_mut();
        }

        let new_top = top + slot_size;
        let committed = self.committed as usize;

        if new_top > committed {
            // Need to commit more pages
            let mut nc = committed + POOL_COMMIT_INCREASE;
            if nc > end {
                nc = end;
            }

            println!("{}: commit {:#x}..{:#x}", self.name, committed, nc);

            // TODO: Implement MapRange for committing pages
            // For now, just update the committed pointer
            self.committed = nc as *mut u8;

            let committed_max = self.committed_max as usize;
            if nc > committed_max {
                self.committed_max = nc as *mut u8;
            }
        }

        let slot = self.top;
        self.top = new_top as *mut u8;
        slot
    }

    /// Return a slot to the pool
    ///
    /// # Arguments
    ///
    /// * `p` - Pointer to slot to return (must be most recently popped)
    pub fn push(&mut self, p: *mut u8) {
        let p = p as usize;
        let top = self.top as usize;
        let slot_size = self.slot_size;

        // Can only push the most-recently-popped slot
        assert!(p + slot_size == top);

        self.top = p as *mut u8;

        let top = self.top as usize;
        let committed = self.committed as usize;

        if committed - top >= POOL_DECOMMIT_THRESHOLD {
            let mut nc = (top + POOL_COMMIT_INCREASE + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            if nc > self.end as usize {
                nc = self.end as usize;
            }

            if nc >= committed {
                return;
            }

            println!("{}: decommit {:#x}..{:#x}", self.name, nc, committed);

            // TODO: Implement DecommitRange
            self.committed = nc as *mut u8;
        }
    }

    /// Check if an address is within the pool range
    pub fn in_range(&self, addr: *mut u8) -> bool {
        let addr = addr as usize;
        let start = self.start as usize;
        let end = self.end as usize;
        addr >= start && addr < end
    }

    /// Dump pool information
    pub fn dump(&self) {
        let start = self.start as usize;
        let top = self.top as usize;
        let committed = self.start as usize;
        let end = self.end as usize;
        let committed_max = self.committed_max as usize;

        let nslots = (top - start) / self.slot_size;
        let np = (committed - start) / PAGE_SIZE;
        let npmax = (committed_max - start) / PAGE_SIZE;
        let nslots_total = (end - start) / self.slot_size;

        println!(
            "  pool '{}' slot size {}, pages committed:",
            self.name, self.slot_size
        );
        println!("  |     start {:#x}", start);
        println!("  |       top {:#x} ({} slots popped)", top, nslots);
        println!(
            "  | committed {:#x} ({} pages; {} pages max)",
            committed, np, npmax
        );
        println!("  \\       end {:#x} ({} slots total)", end, nslots_total);
    }
}

/// Arena allocator
pub struct Arena {
    /// Arena name
    pub name: String,
    /// Object size
    pub ob_size: usize,
    /// Maximum count
    pub count: usize,
    /// Current allocation count
    pub allocated_count: AtomicUsize,
    /// Control pool (for metadata)
    pub control: Mutex<ArenaPool>,
    /// Data pool (for allocations)
    pub data: Mutex<ArenaPool>,
    /// Free list
    pub free: Mutex<LinkedList<ArenaNode>>,
}

unsafe impl Send for Arena {}
unsafe impl Sync for Arena {}

impl Arena {
    /// Create a new arena
    ///
    /// # Arguments
    ///
    /// * `name` - Arena name
    /// * `ob_size` - Object size (must be > 0 and <= PAGE_SIZE)
    /// * `count` - Maximum number of objects
    ///
    /// # Returns
    ///
    /// Ok(arena) on success, Err(status) on failure
    pub fn new(name: &str, ob_size: usize, count: usize) -> Result<Self, i32> {
        if ob_size == 0 || ob_size > PAGE_SIZE {
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }
        if count == 0 {
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }

        println!(
            "Arena '{}': ob_size {}, count {}",
            name, ob_size, count
        );

        let control_mem_size = (count * core::mem::size_of::<ArenaNode>() + PAGE_SIZE - 1)
            & !(PAGE_SIZE - 1);
        let data_mem_size = (count * ob_size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        // TODO: Create VMO and VMAR mappings
        // For now, we'll use a simple stub implementation

        let mut control = ArenaPool {
            name: "control",
            slot_size: core::mem::size_of::<ArenaNode>(),
            start: core::ptr::null_mut(),
            end: core::ptr::null_mut(),
            top: core::ptr::null_mut(),
            committed: core::ptr::null_mut(),
            committed_max: core::ptr::null_mut(),
        };

        let mut data = ArenaPool {
            name: "data",
            slot_size: ob_size,
            start: core::ptr::null_mut(),
            end: core::ptr::null_mut(),
            top: core::ptr::null_mut(),
            committed: core::ptr::null_mut(),
            committed_max: core::ptr::null_mut(),
        };

        // TODO: Initialize pools with actual memory
        // For now, leave as null pointers

        Ok(Self {
            name: name.to_string(),
            ob_size,
            count,
            allocated_count: AtomicUsize::new(0),
            control: Mutex::new(control),
            data: Mutex::new(data),
            free: Mutex::new(LinkedList::new()),
        })
    }

    /// Allocate an object from the arena
    ///
    /// # Returns
    ///
    /// Pointer to allocated object, or null if arena is exhausted
    pub fn alloc(&self) -> *mut u8 {
        // Prefer most-recently-freed slot for cache locality
        let allocation = {
            let mut free = self.free.lock();
            if let Some(node) = free.pop_front() {
                let slot = node.slot;
                // Return node to control pool
                let mut control = self.control.lock();
                control.push(node as *mut ArenaNode as *mut u8);
                slot
            } else {
                let mut data = self.data.lock();
                data.pop()
            }
        };

        if !allocation.is_null() {
            self.allocated_count.fetch_add(1, Ordering::AcqRel);
        }

        allocation
    }

    /// Free an object back to the arena
    ///
    /// # Arguments
    ///
    /// * `addr` - Address of object to free
    pub fn free(&self, addr: *mut u8) {
        if addr.is_null() {
            return;
        }

        self.allocated_count.fetch_sub(1, Ordering::AcqRel);

        // Get a node from control pool
        let node_ptr = {
            let mut control = self.control.lock();
            control.pop()
        };

        if node_ptr.is_null() {
            // Control pool exhausted - can't track this free
            return;
        }

        // Initialize the node
        let node = unsafe {
            let node_ref = &mut *(node_ptr as *mut ArenaNode);
            node_ref.slot = addr;
            node_ref.next = None;
            node_ref
        };

        // Add to free list
        let mut free = self.free.lock();
        free.push_back(unsafe { Arc::from_raw(node as *const ArenaNode) });
    }

    /// Get current allocation count
    pub fn allocated_count(&self) -> usize {
        self.allocated_count.load(Ordering::Acquire)
    }

    /// Dump arena information
    pub fn dump(&self) {
        println!("{} mappings:", self.name);
        println!("{} pools:", self.name);

        let control = self.control.lock();
        control.dump();
        drop(control);

        let data = self.data.lock();
        data.dump();
        drop(data);

        let free = self.free.lock();
        println!("{} free list: {} nodes", self.name, free.len());
    }
}

/// Round up to page boundary
fn round_up_to_page(size: usize) -> usize {
    (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_creation() {
        let arena = Arena::new("test", 64, 100);
        assert!(arena.is_ok());

        let arena = arena.unwrap();
        assert_eq!(arena.name, "test");
        assert_eq!(arena.ob_size, 64);
        assert_eq!(arena.count, 100);
    }

    #[test]
    fn test_invalid_args() {
        assert!(Arena::new("zero_size", 0, 100).is_err());
        assert!(Arena::new("too_large", PAGE_SIZE + 1, 100).is_err());
        assert!(Arena::new("zero_count", 64, 0).is_err());
    }

    #[test]
    fn test_constants() {
        assert_eq!(PAGE_SIZE, 4096);
        assert_eq!(POOL_COMMIT_INCREASE, 4 * PAGE_SIZE);
        assert_eq!(POOL_DECOMMIT_THRESHOLD, 8 * PAGE_SIZE);
    }

    #[test]
    fn test_round_up_to_page() {
        assert_eq!(round_up_to_page(0), 0);
        assert_eq!(round_up_to_page(1), PAGE_SIZE);
        assert_eq!(round_up_to_page(4095), PAGE_SIZE);
        assert_eq!(round_up_to_page(4096), 4096);
        assert_eq!(round_up_to_page(4097), PAGE_SIZE * 2);
    }
}
