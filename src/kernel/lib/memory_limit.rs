// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Memory Limit
//!
//! This module provides memory limiting functionality for the kernel.
//! It allows restricting system memory usage via command line arguments
//! and manages reserved memory regions accordingly.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::rustux::types::*;

/// Page size (4KB)
const PAGE_SIZE: usize = 4096;

/// Megabyte in bytes
const MB: usize = 1024 * 1024;

/// Maximum number of reserved regions
const MAX_RESERVED_REGIONS: usize = 64;

/// System memory limit in bytes
static SYSTEM_MEMORY_LIMIT: AtomicUsize = AtomicUsize::new(0);

/// System memory remaining to be allocated
static SYSTEM_MEMORY_REMAINING: AtomicUsize = AtomicUsize::new(0);

/// Debug flag for memory limit
static MEMORY_LIMIT_DBG: AtomicBool = AtomicBool::new(false);

/// Reserved memory region entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ReserveEntry {
    /// Start of the reserved range
    pub start: u64,
    /// Length of the reserved range
    pub len: usize,
    /// End of the reserved range
    pub end: u64,
    /// Space before the region that is available
    pub unused_front: usize,
    /// Space after the region that is available
    pub unused_back: usize,
}

impl ReserveEntry {
    /// Create a new reserve entry
    pub fn new(start: u64, len: usize) -> Self {
        Self {
            start,
            len,
            end: start + len as u64,
            unused_front: 0,
            unused_back: 0,
        }
    }

    /// Check if this entry intersects with a given range
    pub fn intersects(&self, base: u64, size: usize) -> bool {
        let range_end = base + size as u64;
        self.start < range_end && self.end > base
    }
}

/// PMM arena information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PmmArenaInfo {
    /// Base address of the arena
    pub base: u64,
    /// Size of the arena
    pub size: usize,
    /// Arena priority
    pub priority: u32,
}

/// Reserved region tracking
static RESERVED_REGIONS: Mutex<Vec<ReserveEntry>> = Mutex::new(Vec::new());

/// Initialize memory limit system
///
/// This should be called during early boot to set up the memory limit
/// based on command line arguments.
///
/// # Returns
///
/// Ok(()) on success, Err(status) if already initialized or no limit specified
pub fn memory_limit_init() -> Result<(), i32> {
    if SYSTEM_MEMORY_LIMIT.load(Ordering::Acquire) != 0 {
        return Err(-2); // ZX_ERR_BAD_STATE
    }

    // TODO: Get from command line: kernel.memory-limit-mb
    // For now, return not supported
    // let limit_mb = cmdline_get_uint64("kernel.memory-limit-mb", 0);
    let limit_mb: u64 = 0;

    if limit_mb == 0 {
        return Err(-1); // ZX_ERR_NOT_SUPPORTED
    }

    let limit = (limit_mb as usize) * MB;
    SYSTEM_MEMORY_LIMIT.store(limit, Ordering::Release);
    SYSTEM_MEMORY_REMAINING.store(limit, Ordering::Release);

    // TODO: Get from command line: kernel.memory-limit-dbg
    MEMORY_LIMIT_DBG.store(false, Ordering::Release);

    println!(
        "MemoryLimit: Initialized with limit of {} MB",
        limit_mb
    );

    Ok(())
}

/// Add a memory range to be managed by the memory limit system
///
/// # Arguments
///
/// * `range_base` - Base address of the range
/// * `range_size` - Size of the range
/// * `reserved_ranges` - Reserved memory ranges that must be included
///
/// # Returns
///
/// Ok(()) on success, Err(status) on failure
pub fn memory_limit_add_range(
    range_base: u64,
    range_size: usize,
    reserved_ranges: &[ReserveEntry],
) -> Result<(), i32> {
    let mut regions = RESERVED_REGIONS.lock();

    for &reserve in reserved_ranges {
        // Check if reserved region intersects with this range
        if intersects(range_base, range_size, reserve.start, reserve.len) {
            if regions.len() >= MAX_RESERVED_REGIONS {
                println!("MemoryLimit: Too many reserved regions");
                return Err(-3); // ZX_ERR_OUT_OF_RANGE
            }

            let mut entry = ReserveEntry::new(reserve.start, reserve.len);

            // Calculate unused space
            if regions.is_empty() {
                // First region - unused space is before it
                entry.unused_front = (entry.start - range_base) as usize;
                entry.unused_back = 0;
            } else {
                let prev = regions.last().unwrap();
                let start = range_base.max(prev.end);

                if start == prev.end {
                    // Next to previous region - split the gap
                    let spare_pages = (reserve.start - start) as usize / PAGE_SIZE;
                    entry.unused_front = (spare_pages / 2) * PAGE_SIZE;
                    prev.unused_back = (spare_pages / 2) * PAGE_SIZE;

                    if spare_pages & 1 == 1 {
                        entry.unused_front += PAGE_SIZE;
                    }
                } else {
                    entry.unused_front = (reserve.start - start) as usize;
                }
            }

            regions.push(entry);
        }
    }

    // Account for space after the last region
    if !regions.is_empty() {
        let last_entry = regions.last_mut().unwrap();
        if last_entry.intersects(range_base, range_size) {
            last_entry.unused_back =
                (range_base + range_size as u64 - last_entry.end) as usize;
        }
    }

    if MEMORY_LIMIT_DBG.load(Ordering::Acquire) {
        println!(
            "MemoryLimit: Processed arena [{:#x} - {:#x}]",
            range_base,
            range_base + range_size as u64
        );
        print_reserve_state(&regions);
    }

    Ok(())
}

/// Add arenas to the physical memory manager based on memory limits
///
/// # Arguments
///
/// * `arena_template` - Template for arena creation
///
/// # Returns
///
/// Ok(()) on success, Err(status) on failure
pub fn memory_limit_add_arenas(arena_template: PmmArenaInfo) -> Result<(), i32> {
    let mut regions = RESERVED_REGIONS.lock();

    // First pass: calculate required memory for reserved regions
    let mut required_for_reserved: usize = 0;
    for entry in regions.iter() {
        required_for_reserved += (entry.end - entry.start) as usize;
    }

    let remaining = SYSTEM_MEMORY_REMAINING.load(Ordering::Acquire);

    println!(
        "MemoryLimit: Limit of {} bytes provided by kernel.memory-limit-mb",
        remaining
    );

    if required_for_reserved > remaining {
        println!(
            "MemoryLimit: Reserved regions need {} bytes at a minimum!",
            required_for_reserved
        );
        return Err(-1); // ZX_ERR_NO_MEMORY
    }

    let mut remaining = remaining - required_for_reserved;

    if MEMORY_LIMIT_DBG.load(Ordering::Acquire) {
        println!(
            "MemoryLimit: First pass, {:#x} remaining",
            remaining
        );
        print_reserve_state(&regions);
    }

    // Second pass: expand regions to take memory from front/back
    for entry in regions.iter_mut() {
        // Expand from front
        let available = remaining.min(entry.unused_front);
        if available > 0 {
            remaining -= available;
            entry.unused_front -= available;
            entry.start = page_align_down(entry.start - available as u64);
        }

        // Expand from back
        let available = remaining.min(entry.unused_back);
        if available > 0 {
            remaining -= available;
            entry.unused_back -= available;
            entry.end = page_align(entry.end + available as u64);
        }

        // Ensure enough space for bookkeeping
        let needed = round_up_to_page((entry.len * 101) / 100);
        let current_size = (entry.end - entry.start) as usize;

        if needed > current_size {
            let diff = needed - current_size;
            println!(
                "MemoryLimit: Region needs {:#x} more bytes for bookkeeping",
                diff
            );

            if entry.unused_front > diff {
                entry.unused_front -= diff;
                entry.start -= diff as u64;
            } else if entry.unused_back > diff {
                entry.unused_back -= diff;
                entry.end += diff as u64;
            } else {
                println!("MemoryLimit: Unable to fit bookkeeping!");
                return Err(-1); // ZX_ERR_NO_MEMORY
            }
        }
    }

    if MEMORY_LIMIT_DBG.load(Ordering::Acquire) {
        println!(
            "MemoryLimit: Second pass, {:#x} remaining",
            remaining
        );
        print_reserve_state(&regions);
    }

    // Third pass: coalesce adjacent regions
    let mut i = 0;
    while i < regions.len() - 1 {
        if regions[i].end == regions[i + 1].start {
            println!(
                "MemoryLimit: Merging |{:#x} - {:#x}| and |{:#x} - {:#x}|",
                regions[i].start, regions[i].end, regions[i + 1].start, regions[i + 1].end
            );
            regions[i].end = regions[i + 1].end;
            regions.remove(i + 1);
        } else {
            i += 1;
        }
    }

    if MEMORY_LIMIT_DBG.load(Ordering::Acquire) {
        println!("MemoryLimit: Third pass (coalesced)");
        print_reserve_state(&regions);
        println!("MemoryLimit: Fourth pass (adding arenas)");
    }

    // Fourth pass: add arenas to PMM
    for entry in regions.iter() {
        let size = (entry.end - entry.start) as usize;

        if MEMORY_LIMIT_DBG.load(Ordering::Acquire) {
            println!(
                "MemoryLimit: Adding arena [{:#x} - {:#x}]",
                entry.start, entry.end
            );
        }

        // TODO: Call pmm_add_arena
        // let mut arena = arena_template;
        // arena.base = entry.start;
        // arena.size = size;
        // pmm_add_arena(&arena)?;
    }

    println!("MemoryLimit: Arenas added successfully");
    Ok(())
}

/// Check if two ranges intersect
fn intersects(base1: u64, size1: usize, base2: u64, size2: usize) -> bool {
    let end1 = base1 + size1 as u64;
    let end2 = base2 + size2 as u64;
    base1 < end2 && base2 < end1
}

/// Round up to page boundary
fn round_up_to_page(size: usize) -> usize {
    (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

/// Page align address down
fn page_align_down(addr: u64) -> u64 {
    addr & !(PAGE_SIZE as u64 - 1)
}

/// Page align address up
fn page_align(addr: u64) -> u64 {
    page_align_down(addr + PAGE_SIZE as u64 - 1)
}

/// Print current reserve state for debugging
fn print_reserve_state(regions: &[ReserveEntry]) {
    for (i, entry) in regions.iter().enumerate() {
        println!(
            "{}: [f: {:#x} |{:#x} - {:#x}| (len: {:#x}) b: {:#x}]",
            i,
            entry.unused_front,
            entry.start,
            entry.end,
            entry.len,
            entry.unused_back
        );
    }
}

/// Get the system memory limit
pub fn get_system_memory_limit() -> usize {
    SYSTEM_MEMORY_LIMIT.load(Ordering::Acquire)
}

/// Get the remaining memory to be allocated
pub fn get_system_memory_remaining() -> usize {
    SYSTEM_MEMORY_REMAINING.load(Ordering::Acquire)
}

/// Enable debug output
pub fn set_debug(enabled: bool) {
    MEMORY_LIMIT_DBG.store(enabled, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_alignment() {
        assert_eq!(round_up_to_page(0), 0);
        assert_eq!(round_up_to_page(1), PAGE_SIZE);
        assert_eq!(round_up_to_page(4095), PAGE_SIZE);
        assert_eq!(round_up_to_page(4096), 4096);
        assert_eq!(round_up_to_page(4097), PAGE_SIZE * 2);
    }

    #[test]
    fn test_intersects() {
        // Non-overlapping
        assert!(!intersects(0x1000, 0x1000, 0x3000, 0x1000));

        // Overlapping
        assert!(intersects(0x1000, 0x2000, 0x2000, 0x1000));
        assert!(intersects(0x1000, 0x2000, 0x1500, 0x1000));

        // Adjacent (not intersecting)
        assert!(!intersects(0x1000, 0x1000, 0x2000, 0x1000));
    }

    #[test]
    fn test_reserve_entry() {
        let entry = ReserveEntry::new(0x1000, 0x1000);
        assert_eq!(entry.start, 0x1000);
        assert_eq!(entry.len, 0x1000);
        assert_eq!(entry.end, 0x2000);
        assert!(entry.intersects(0x1500, 0x100));
        assert!(!entry.intersects(0x3000, 0x100));
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_RESERVED_REGIONS, 64);
        assert_eq!(PAGE_SIZE, 4096);
        assert_eq!(MB, 1024 * 1024);
    }
}
