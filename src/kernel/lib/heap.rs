// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Heap Allocator Wrapper
//!
//! This module provides a wrapper around the compact malloc allocator.
//! It implements standard C allocation functions (malloc, free, calloc, etc.)
//! with statistics tracking and page allocation support.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Enable heap statistics collection
#[cfg(debug_assertions)]
const HEAP_COLLECT_STATS: bool = true;

#[cfg(not(debug_assertions))]
const HEAP_COLLECT_STATS: bool = false;

/// Panic on allocation failure in debug builds
#[cfg(debug_assertions)]
const HEAP_PANIC_ON_ALLOC_FAIL: bool = true;

#[cfg(not(debug_assertions))]
const HEAP_PANIC_ON_ALLOC_FAIL: bool = false;

/// Enable heap tracing in debug builds
#[cfg(debug_assertions)]
static HEAP_TRACE: AtomicBool = AtomicBool::new(false);

/// Allocation statistic entry
#[derive(Debug, Clone)]
struct AllocStat {
    /// Caller address
    caller: usize,
    /// Allocation size
    size: usize,
    /// Allocation count
    count: AtomicU64,
    /// Next stat in list
    next: Option<*mut AllocStat>,
}

/// Allocation statistics
struct HeapStats {
    /// List of allocation statistics
    stats: Vec<AllocStat>,
    /// Maximum number of statistics to track
    max_stats: usize,
    /// Next unused statistic index
    next_unused: AtomicUsize,
}

impl HeapStats {
    /// Create new heap statistics
    fn new(max_stats: usize) -> Self {
        Self {
            stats: Vec::with_capacity(max_stats),
            max_stats,
            next_unused: AtomicUsize::new(0),
        }
    }

    /// Add a statistic entry
    fn add_stat(&mut self, caller: usize, size: usize) {
        if !HEAP_COLLECT_STATS {
            return;
        }

        // Look for existing stat and update it
        for stat in &mut self.stats {
            if stat.caller == caller && stat.size == size {
                stat.count.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }

        // Allocate new stat if space available
        let next_unused = self.next_unused.load(Ordering::Relaxed);
        if next_unused >= self.max_stats {
            return;
        }

        self.stats.push(AllocStat {
            caller,
            size,
            count: AtomicU64::new(1),
            next: None,
        });
        self.next_unused.fetch_add(1, Ordering::Relaxed);
    }

    /// Dump statistics
    fn dump_stats(&self) {
        if !HEAP_COLLECT_STATS {
            return;
        }

        let mut sorted_stats = self.stats.clone();
        sorted_stats.sort_by(|a, b| b.size.cmp(&a.size));

        for stat in &sorted_stats {
            let count = stat.count.load(Ordering::Relaxed);
            println!(
                "size {:8} count {:8} caller {:#x}",
                stat.size, count, stat.caller
            );
        }

        if self.next_unused.load(Ordering::Relaxed) >= self.max_stats {
            println!(
                "WARNING: max number of unique records hit, some statistics were likely lost"
            );
        }
    }
}

/// Global heap statistics
static HEAP_STATS: Mutex<HeapStats> = Mutex::new(HeapStats::new(1024));

/// Get caller address
///
/// This returns the address of the caller for tracking purposes.
#[inline(always)]
fn caller_address() -> usize {
    // In a real implementation, this would use frame pointer walking
    // or platform-specific return address register
    0
}

/// Add allocation statistic
fn add_stat(caller: usize, size: usize) {
    if !HEAP_COLLECT_STATS {
        return;
    }

    let mut stats = HEAP_STATS.lock();
    stats.add_stat(caller, size);
}

/// Initialize the heap
pub fn heap_init() {
    println!("Heap: Initializing");

    // Initialize the compact malloc allocator
    cmpct_init();

    println!("Heap: Initialized");
}

/// Trim the heap
///
/// This returns unused pages to the system.
pub fn heap_trim() {
    println!("Heap: Trimming");
    cmpct_trim();
}

/// Allocate memory
///
/// # Arguments
///
/// * `size` - Size in bytes
///
/// # Returns
///
/// Pointer to allocated memory, or null if allocation failed
pub fn malloc(size: usize) -> *mut u8 {
    let caller = caller_address();
    add_stat(caller, size);

    let ptr = cmpct_alloc(size);

    #[cfg(debug_assertions)]
    {
        if HEAP_TRACE.load(Ordering::Relaxed) {
            println!("caller {:#x} malloc {} -> {:p}", caller, size, ptr);
        }
    }

    if HEAP_PANIC_ON_ALLOC_FAIL && ptr.is_null() && size > 0 {
        panic!("malloc of size {} failed", size);
    }

    ptr
}

/// Allocate memory with explicit caller
///
/// # Arguments
///
/// * `size` - Size in bytes
/// * `caller` - Caller address for tracking
///
/// # Returns
///
/// Pointer to allocated memory, or null if allocation failed
pub fn malloc_debug_caller(size: usize, caller: usize) -> *mut u8 {
    add_stat(caller, size);

    let ptr = cmpct_alloc(size);

    #[cfg(debug_assertions)]
    {
        if HEAP_TRACE.load(Ordering::Relaxed) {
            println!("caller {:#x} malloc {} -> {:p}", caller, size, ptr);
        }
    }

    if HEAP_PANIC_ON_ALLOC_FAIL && ptr.is_null() && size > 0 {
        panic!("malloc of size {} failed", size);
    }

    ptr
}

/// Allocate aligned memory
///
/// # Arguments
///
/// * `boundary` - Alignment boundary in bytes (must be power of 2)
/// * `size` - Size in bytes
///
/// # Returns
///
/// Pointer to allocated memory, or null if allocation failed
pub fn memalign(boundary: usize, size: usize) -> *mut u8 {
    let caller = caller_address();
    add_stat(caller, size);

    let ptr = cmpct_memalign(size, boundary);

    #[cfg(debug_assertions)]
    {
        if HEAP_TRACE.load(Ordering::Relaxed) {
            println!(
                "caller {:#x} memalign {}, {} -> {:p}",
                caller, boundary, size, ptr
            );
        }
    }

    if HEAP_PANIC_ON_ALLOC_FAIL && ptr.is_null() && size > 0 {
        panic!("memalign of size {} align {} failed", size, boundary);
    }

    ptr
}

/// Allocate and zero memory
///
/// # Arguments
///
/// * `count` - Number of elements
/// * `size` - Size of each element
///
/// # Returns
///
/// Pointer to allocated and zeroed memory, or null if allocation failed
pub fn calloc(count: usize, size: usize) -> *mut u8 {
    let caller = caller_address();
    add_stat(caller, size);

    let realsize = count.saturating_mul(size);

    let ptr = cmpct_alloc(realsize);

    if !ptr.is_null() {
        // Zero the memory
        unsafe {
            core::ptr::write_bytes(ptr, 0, realsize);
        }
    }

    #[cfg(debug_assertions)]
    {
        if HEAP_TRACE.load(Ordering::Relaxed) {
            println!(
                "caller {:#x} calloc {}, {} -> {:p}",
                caller, count, size, ptr
            );
        }
    }

    ptr
}

/// Reallocate memory
///
/// # Arguments
///
/// * `ptr` - Existing pointer (or null for new allocation)
/// * `size` - New size in bytes
///
/// # Returns
///
/// Pointer to reallocated memory, or null if allocation failed
pub fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
    let caller = caller_address();
    add_stat(caller, size);

    let ptr2 = cmpct_realloc(ptr, size);

    #[cfg(debug_assertions)]
    {
        if HEAP_TRACE.load(Ordering::Relaxed) {
            println!(
                "caller {:#x} realloc {:p}, {} -> {:p}",
                caller, ptr, size, ptr2
            );
        }
    }

    if HEAP_PANIC_ON_ALLOC_FAIL && ptr2.is_null() && size > 0 {
        panic!("realloc of size {} old ptr {:p} failed", size, ptr);
    }

    ptr2
}

/// Free memory
///
/// # Arguments
///
/// * `ptr` - Pointer to memory to free (null is safe)
pub fn free(ptr: *mut u8) {
    #[cfg(debug_assertions)]
    {
        if HEAP_TRACE.load(Ordering::Relaxed) {
            let caller = caller_address();
            println!("caller {:#x} free {:p}", caller, ptr);
        }
    }

    cmpct_free(ptr);
}

/// Dump heap information
///
/// # Arguments
///
/// * `panic_time` - Whether this is being called during a panic
pub fn heap_dump(panic_time: bool) {
    cmpct_dump(panic_time);
}

/// Get heap information
///
/// # Returns
///
/// Tuple of (total_bytes, free_bytes)
pub fn heap_get_info() -> (usize, usize) {
    let mut size_bytes = 0;
    let mut free_bytes = 0;
    cmpct_get_info(&mut size_bytes, &mut free_bytes);
    (size_bytes, free_bytes)
}

/// Run heap tests
pub fn heap_test() {
    cmpct_test();
}

/// Allocate pages from the heap
///
/// # Arguments
///
/// * `pages` - Number of pages to allocate
///
/// # Returns
///
/// Pointer to allocated pages, or null if allocation failed
pub fn heap_page_alloc(pages: usize) -> *mut u8 {
    assert!(pages > 0, "heap_page_alloc: pages must be > 0");

    // TODO: Allocate contiguous pages from PMM
    // Mark pages as HEAP state
    let _ = pages;

    println!("Heap: Page alloc requested ({} pages)", pages);

    core::ptr::null_mut()
}

/// Free pages to the heap
///
/// # Arguments
///
/// * `ptr` - Pointer to pages to free
/// * `pages` - Number of pages
pub fn heap_page_free(ptr: *mut u8, pages: usize) {
    assert!((ptr as usize) % 4096 == 0, "heap_page_free: ptr not page aligned");
    assert!(pages > 0, "heap_page_free: pages must be > 0");

    println!("Heap: Page free {:p} ({} pages)", ptr, pages);

    // TODO: Free pages back to PMM
    let _ = (ptr, pages);
}

/// Compact malloc allocator functions
/// These are stubs - the real implementation would be in cmpctmalloc.rs

/// Initialize compact malloc
fn cmpct_init() {
    println!("Heap: cmpct_init");
}

/// Trim compact malloc
fn cmpct_trim() {
    println!("Heap: cmpct_trim");
}

/// Allocate from compact malloc
fn cmpct_alloc(size: usize) -> *mut u8 {
    // TODO: Implement actual allocation
    let _ = size;
    core::ptr::null_mut()
}

/// Allocate aligned memory from compact malloc
fn cmpct_memalign(size: usize, boundary: usize) -> *mut u8 {
    // TODO: Implement actual aligned allocation
    let _ = (size, boundary);
    core::ptr::null_mut()
}

/// Reallocate with compact malloc
fn cmpct_realloc(ptr: *mut u8, size: usize) -> *mut u8 {
    // TODO: Implement actual reallocation
    let _ = (ptr, size);
    core::ptr::null_mut()
}

/// Free with compact malloc
fn cmpct_free(ptr: *mut u8) {
    // TODO: Implement actual free
    let _ = ptr;
}

/// Dump compact malloc info
fn cmpct_dump(panic_time: bool) {
    println!("Heap: cmpct_dump (panic_time: {})", panic_time);
}

/// Get compact malloc info
fn cmpct_get_info(size_bytes: &mut usize, free_bytes: &mut usize) {
    *size_bytes = 0;
    *free_bytes = 0;
}

/// Test compact malloc
fn cmpct_test() {
    println!("Heap: cmpct_test");
}

/// Enable heap tracing
#[cfg(debug_assertions)]
pub fn heap_set_trace(enabled: bool) {
    HEAP_TRACE.store(enabled, Ordering::Relaxed);
    println!("Heap: trace is now {}", if enabled { "on" } else { "off" });
}

/// Dump heap statistics
pub fn heap_dump_stats() {
    let stats = HEAP_STATS.lock();
    stats.dump_stats();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heap_stats() {
        let stats = HeapStats::new(10);
        assert_eq!(stats.stats.len(), 0);
    }

    #[test]
    fn test_add_stat() {
        let mut stats = HeapStats::new(10);
        stats.add_stat(0x1000, 64);
        assert_eq!(stats.stats.len(), 1);
        assert_eq!(stats.stats[0].caller, 0x1000);
        assert_eq!(stats.stats[0].size, 64);
    }

    #[test]
    fn test_constants() {
        assert_eq!(HEAP_COLLECT_STATS, cfg!(debug_assertions));
    }
}
