// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Memory Statistics
//!
//! This module provides memory statistics tracking for the kernel.
//! It tracks total, free, wired, active, and inactive memory.
//!
//! # Design
//!
//! - **Global statistics**: Track system-wide memory usage
//! - **Per-process statistics**: Track memory usage per process
//! - **Page fault tracking**: Track page ins/outs
//! - **Thread-safe**: All operations are atomic
//!
//! # Usage
//!
//! ```rust
//! let stats = memory_stats();
//! println!("Total: {} MB", stats.total_bytes / 1024 / 1024);
//! ```


use crate::kernel::pmm;
use crate::rustux::types::*;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::log_info;

/// ============================================================================
/// Memory Statistics
/// ============================================================================

/// Memory statistics snapshot
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
    /// Total physical memory in bytes
    pub total_bytes: usize,

    /// Free memory in bytes
    pub free_bytes: usize,

    /// Wired (non-pageable) memory in bytes
    pub wired_bytes: usize,

    /// Active (recently used) memory in bytes
    pub active_bytes: usize,

    /// Inactive (not recently used) memory in bytes
    pub inactive_bytes: usize,

    /// Compressed memory in bytes (future)
    pub compressed_bytes: usize,

    /// Page-ins from disk (counter)
    pub page_ins: u64,

    /// Page-outs to disk (counter)
    pub page_outs: u64,

    /// Page faults (counter)
    pub page_faults: u64,

    /// COW page faults (counter)
    pub cow_faults: u64,

    /// Total pages allocated
    pub pages_allocated: u64,

    /// Total pages freed
    pub pages_freed: u64,
}

impl MemoryStats {
    /// Create zero statistics
    pub const fn zero() -> Self {
        Self {
            total_bytes: 0,
            free_bytes: 0,
            wired_bytes: 0,
            active_bytes: 0,
            inactive_bytes: 0,
            compressed_bytes: 0,
            page_ins: 0,
            page_outs: 0,
            page_faults: 0,
            cow_faults: 0,
            pages_allocated: 0,
            pages_freed: 0,
        }
    }

    /// Get total bytes in MB
    pub const fn total_mb(&self) -> usize {
        self.total_bytes / 1024 / 1024
    }

    /// Get free bytes in MB
    pub const fn free_mb(&self) -> usize {
        self.free_bytes / 1024 / 1024
    }

    /// Get wired bytes in MB
    pub const fn wired_mb(&self) -> usize {
        self.wired_bytes / 1024 / 1024
    }

    /// Get active bytes in MB
    pub const fn active_mb(&self) -> usize {
        self.active_bytes / 1024 / 1024
    }

    /// Get inactive bytes in MB
    pub const fn inactive_mb(&self) -> usize {
        self.inactive_bytes / 1024 / 1024
    }

    /// Calculate memory usage percentage
    pub const fn usage_percent(&self) -> u8 {
        if self.total_bytes == 0 {
            return 0;
        }

        let used = self.total_bytes - self.free_bytes;
        ((used * 100) / self.total_bytes) as u8
    }
}

/// ============================================================================
/// Global Memory Statistics
/// ============================================================================

/// Global memory statistics tracker
pub struct MemoryStatsTracker {
    /// Wired pages (non-pageable)
    wired_pages: AtomicUsize,

    /// Active pages
    active_pages: AtomicUsize,

    /// Inactive pages
    inactive_pages: AtomicUsize,

    /// Compressed pages (future)
    compressed_pages: AtomicUsize,

    /// Page-ins counter
    page_ins: AtomicU64,

    /// Page-outs counter
    page_outs: AtomicU64,

    /// Page fault counter
    page_faults: AtomicU64,

    /// COW fault counter
    cow_faults: AtomicU64,

    /// Pages allocated counter
    pages_allocated: AtomicU64,

    /// Pages freed counter
    pages_freed: AtomicU64,
}

impl MemoryStatsTracker {
    /// Create a new statistics tracker
    pub const fn new() -> Self {
        Self {
            wired_pages: AtomicUsize::new(0),
            active_pages: AtomicUsize::new(0),
            inactive_pages: AtomicUsize::new(0),
            compressed_pages: AtomicUsize::new(0),
            page_ins: AtomicU64::new(0),
            page_outs: AtomicU64::new(0),
            page_faults: AtomicU64::new(0),
            cow_faults: AtomicU64::new(0),
            pages_allocated: AtomicU64::new(0),
            pages_freed: AtomicU64::new(0),
        }
    }

    /// Record a page fault
    pub fn record_page_fault(&self) {
        self.page_faults.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a COW fault
    pub fn record_cow_fault(&self) {
        self.cow_faults.fetch_add(1, Ordering::Relaxed);
        self.page_faults.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a page-in
    pub fn record_page_in(&self) {
        self.page_ins.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a page-out
    pub fn record_page_out(&self) {
        self.page_outs.fetch_add(1, Ordering::Relaxed);
    }

    /// Record page allocation
    pub fn record_alloc(&self) {
        self.pages_allocated.fetch_add(1, Ordering::Relaxed);
    }

    /// Record page free
    pub fn record_free(&self) {
        self.pages_freed.fetch_add(1, Ordering::Relaxed);
    }

    /// Add wired pages
    pub fn add_wired(&self, count: usize) {
        self.wired_pages.fetch_add(count, Ordering::Relaxed);
    }

    /// Subtract wired pages
    pub fn sub_wired(&self, count: usize) {
        self.wired_pages.fetch_sub(count, Ordering::Relaxed);
    }

    /// Add active pages
    pub fn add_active(&self, count: usize) {
        self.active_pages.fetch_add(count, Ordering::Relaxed);
    }

    /// Subtract active pages
    pub fn sub_active(&self, count: usize) {
        self.active_pages.fetch_sub(count, Ordering::Relaxed);
    }

    /// Add inactive pages
    pub fn add_inactive(&self, count: usize) {
        self.inactive_pages.fetch_add(count, Ordering::Relaxed);
    }

    /// Subtract inactive pages
    pub fn sub_inactive(&self, count: usize) {
        self.inactive_pages.fetch_sub(count, Ordering::Relaxed);
    }

    /// Get current statistics snapshot
    pub fn snapshot(&self) -> MemoryStats {
        let total_pages = pmm::pmm_count_total_pages() as usize;
        let free_pages = pmm::pmm_count_free_pages() as usize;

        let wired = self.wired_pages.load(Ordering::Relaxed);
        let active = self.active_pages.load(Ordering::Relaxed);
        let inactive = self.inactive_pages.load(Ordering::Relaxed);
        let compressed = self.compressed_pages.load(Ordering::Relaxed);

        MemoryStats {
            total_bytes: total_pages * 4096,
            free_bytes: free_pages * 4096,
            wired_bytes: wired * 4096,
            active_bytes: active * 4096,
            inactive_bytes: inactive * 4096,
            compressed_bytes: compressed * 4096,
            page_ins: self.page_ins.load(Ordering::Relaxed),
            page_outs: self.page_outs.load(Ordering::Relaxed),
            page_faults: self.page_faults.load(Ordering::Relaxed),
            cow_faults: self.cow_faults.load(Ordering::Relaxed),
            pages_allocated: self.pages_allocated.load(Ordering::Relaxed),
            pages_freed: self.pages_freed.load(Ordering::Relaxed),
        }
    }
}

/// ============================================================================
/// Global Statistics
/// ============================================================================

/// Global memory statistics tracker
static mut GLOBAL_STATS: MemoryStatsTracker = MemoryStatsTracker::new();

/// Initialize memory statistics
pub fn init_stats() {
    unsafe {
        GLOBAL_STATS = MemoryStatsTracker::new();
    }

    log_info!("Memory statistics initialized");
}

/// Get global memory statistics
pub fn memory_stats() -> MemoryStats {
    unsafe { GLOBAL_STATS.snapshot() }
}

/// Record a page fault
pub fn record_page_fault() {
    unsafe {
        GLOBAL_STATS.record_page_fault();
    }
}

/// Record a COW fault
pub fn record_cow_fault() {
    unsafe {
        GLOBAL_STATS.record_cow_fault();
    }
}

/// Record a page-in
pub fn record_page_in() {
    unsafe {
        GLOBAL_STATS.record_page_in();
    }
}

/// Record a page-out
pub fn record_page_out() {
    unsafe {
        GLOBAL_STATS.record_page_out();
    }
}

/// Record page allocation
pub fn record_page_alloc() {
    unsafe {
        GLOBAL_STATS.record_alloc();
    }
}

/// Record page free
pub fn record_page_free() {
    unsafe {
        GLOBAL_STATS.record_free();
    }
}

/// ============================================================================
/// Per-Process Memory Statistics
/// ============================================================================

/// Process ID
pub type ProcessId = u64;

/// Per-process memory statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessMemoryStats {
    /// Process ID
    pub pid: ProcessId,

    /// Virtual memory size in bytes
    pub vsize: usize,

    /// Resident set size in bytes
    pub rss: usize,

    /// Shared memory size in bytes
    pub shared: usize,

    /// Text size in bytes
    pub text: usize,

    /// Data size in bytes
    pub data: usize,

    /// Stack size in bytes
    pub stack: usize,

    /// Number of page faults
    pub page_faults: u64,
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_zero() {
        let stats = MemoryStats::zero();
        assert_eq!(stats.total_bytes, 0);
        assert_eq!(stats.usage_percent(), 0);
    }

    #[test]
    fn test_stats_calculations() {
        let mut stats = MemoryStats::zero();
        stats.total_bytes = 1024 * 1024 * 100; // 100 MB
        stats.free_bytes = 1024 * 1024 * 50;  // 50 MB free

        assert_eq!(stats.total_mb(), 100);
        assert_eq!(stats.free_mb(), 50);
        assert_eq!(stats.usage_percent(), 50); // 50% used
    }

    #[test]
    fn test_tracker() {
        let tracker = MemoryStatsTracker::new();

        tracker.record_page_fault();
        tracker.record_cow_fault();

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.page_faults, 2);
        assert_eq!(snapshot.cow_faults, 1);
    }
}
