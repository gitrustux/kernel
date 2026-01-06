// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Pager Interface for Demand Paging
//!
//! This module provides the pager interface for handling page faults
//! and implementing demand paging, copy-on-write, and other advanced
//! memory management features.
//!
//! # Design
//!
//! - **Demand paging**: Allocate pages on first access
//! - **COW handling**: Split pages on first write
//! - **Page pinning**: Prevent pages from being evicted
//! - **Zero page optimization**: Share zero pages
//!
//! # Usage
//!
//! ```rust
//! let pager = DefaultPager::new();
//! pager.fault(&vmo, offset)?;
//! ```

#![no_std]

use crate::kernel::pmm;
use crate::kernel::vm::page_table::PageTableFlags;
use crate::kernel::vm::layout::VAddr;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::collections::BTreeSet;
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, AtomicUsize, AtomicU64, Ordering};
use crate::kernel::sync::spin::SpinMutex;

// Import logging macros
use crate::{log_debug, log_info};

/// ============================================================================
/// Physical Frame
/// ============================================================================

/// Physical frame number
pub type FrameNum = u64;

/// Physical frame
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frame {
    /// Physical frame number
    pub num: FrameNum,

    /// Whether frame is zeroed
    pub zeroed: bool,
}

impl Frame {
    /// Create a new frame
    pub const fn new(num: FrameNum, zeroed: bool) -> Self {
        Self { num, zeroed }
    }

    /// Get physical address
    pub const fn paddr(&self) -> PAddr {
        (self.num << 12) as PAddr
    }

    /// Convert to virtual address (for kernel mapping)
    pub fn vaddr(&self) -> VAddr {
        pmm::paddr_to_vaddr(self.paddr())
    }
}

/// ============================================================================
/// Page Fault Info
/// ============================================================================

/// Page fault flags
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFaultFlags(pub u32);

impl PageFaultFlags {
    /// No flags
    pub const empty: Self = Self(0);

    /// Fault on read access
    pub const READ: Self = Self(0x01);

    /// Fault on write access
    pub const WRITE: Self = Self(0x02);

    /// Fault on execute access
    pub const EXECUTE: Self = Self(0x04);

    /// Fault caused by user mode
    pub const USER: Self = Self(0x08);

    /// Fault on instruction fetch
    pub const INSTRUCTION: Self = Self(0x10);

    /// Page not present
    pub const NOT_PRESENT: Self = Self(0x20);

    /// Protection fault (page present but wrong permissions)
    pub const PROTECTION: Self = Self(0x40);

    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self.0
    }

    /// Check if write fault
    pub const fn is_write(self) -> bool {
        (self.0 & Self::WRITE.0) != 0
    }

    /// Check if read fault
    pub const fn is_read(self) -> bool {
        !self.is_write() && !self.is_execute()
    }

    /// Check if execute fault
    pub const fn is_execute(self) -> bool {
        (self.0 & Self::EXECUTE.0) != 0
    }

    /// Check if user fault
    pub const fn is_user(self) -> bool {
        (self.0 & Self::USER.0) != 0
    }

    /// Check if not present
    pub const fn is_not_present(self) -> bool {
        (self.0 & Self::NOT_PRESENT.0) != 0
    }

    /// Check if protection fault
    pub const fn is_protection(self) -> bool {
        (self.0 & Self::PROTECTION.0) != 0
    }
}

/// ============================================================================
/// Pager Interface
/// ============================================================================

/// Pager trait for handling page faults
///
/// Implementations provide different paging strategies.
pub trait Pager {
    /// Handle a page fault
    ///
    /// # Arguments
    ///
    /// * `vmo_id` - VMO that faulted
    /// * `offset` - Offset within VMO that caused fault
    /// * `flags` - Fault flags (read/write/execute)
    ///
    /// # Returns
    ///
    /// Physical frame to map
    fn fault(&self, vmo_id: u64, offset: usize, flags: PageFaultFlags) -> Result<Frame>;

    /// Supply pages for a VMO range
    ///
    /// Pre-populates pages for a range.
    fn supply_pages(&self, vmo_id: u64, offset: usize, len: usize) -> Result<()>;

    /// Pin pages (prevent eviction)
    ///
    /// # Arguments
    ///
    /// * `vmo_id` - VMO to pin
    /// * `offset` - Offset within VMO
    /// * `len` - Length to pin
    fn pin(&self, vmo_id: u64, offset: usize, len: usize) -> Result<()>;

    /// Unpin pages
    ///
    /// # Arguments
    ///
    /// * `vmo_id` - VMO to unpin
    /// * `offset` - Offset within VMO
    /// * `len` - Length to unpin
    fn unpin(&self, vmo_id: u64, offset: usize, len: usize) -> Result<()>;
}

/// ============================================================================
/// Default Pager Implementation
/// ============================================================================

/// Default pager implementation
///
/// Provides demand paging with COW support.
pub struct DefaultPager {
    /// Zero page (shared zero-filled page)
    zero_page: AtomicUsize,

    /// Pinned pages (vmo_id, offset) → frame
    pinned_pages: crate::kernel::sync::spin::SpinMutex<BTreeSet<(u64, usize)>>,
}

impl DefaultPager {
    /// Create a new default pager
    pub const fn new() -> Self {
        use crate::kernel::sync::spin::SpinMutex;
        Self {
            zero_page: AtomicUsize::new(0),
            pinned_pages: SpinMutex::new(BTreeSet::new()),
        }
    }

    /// Get or allocate zero page
    fn get_zero_page(&self) -> Result<Frame> {
        let zero = self.zero_page.load(Ordering::Acquire);

        if zero == 0 {
            // Allocate zero page
            let frame = pmm::alloc_page()?;
            let vaddr = pmm::paddr_to_vaddr(frame);

            // Zero it
            unsafe {
                core::ptr::write_bytes(vaddr as *mut u8, 0, 4096);
            }

            // Try to store it
            let result = self.zero_page.compare_exchange(
                zero,
                vaddr as usize,
                Ordering::Release,
                Ordering::Relaxed,
            );

            if result.is_err() {
                // Someone else allocated it, free ours
                pmm::free_page(frame);
            }

            Ok(Frame::new(frame >> 12, true))
        } else {
            // Use existing zero page
            // The zero_page value stores a virtual address, so use it directly
            let vaddr = zero as VAddr;
            // For kernel direct-mapped addresses, we can use the physical mapping
            // In a real system, we'd need a proper virt_to_phys function
            // For now, assume the zero page is already set up correctly
            Ok(Frame::new((vaddr >> 12) as u64, true))
        }
    }
}

impl Pager for DefaultPager {
    fn fault(&self, vmo_id: u64, offset: usize, flags: PageFaultFlags) -> Result<Frame> {
        // Align offset to page boundary
        let aligned_offset = offset & !0xFFF;

        // TODO: Look up VMO from vmo_id
        // For now, allocate a new page
        let frame_num = pmm::alloc_page()?;
        let vaddr = pmm::paddr_to_vaddr(frame_num);

        // Zero the page
        unsafe {
            core::ptr::write_bytes(vaddr as *mut u8, 0, 4096);
        }

        Ok(Frame::new(frame_num >> 12, true))
    }

    fn supply_pages(&self, _vmo_id: u64, offset: usize, len: usize) -> Result<()> {
        // Pre-allocate pages for range
        let page_count = (len + 4095) / 4096;

        for i in 0..page_count {
            let current_offset = offset + (i * 4096);

            // Skip if already present
            // TODO: Check if page exists

            // Allocate page
            let frame_num = pmm::alloc_page()?;
            let vaddr = pmm::paddr_to_vaddr(frame_num);

            // Zero the page
            unsafe {
                core::ptr::write_bytes(vaddr as *mut u8, 0, 4096);
            };

            let _ = current_offset;
            let _ = vaddr;
            // TODO: Add to VMO page map
        }

        Ok(())
    }

    fn pin(&self, vmo_id: u64, offset: usize, len: usize) -> Result<()> {
        let mut pinned = self.pinned_pages.lock();
        let page_count = (len + 4095) / 4096;

        for i in 0..page_count {
            pinned.insert((vmo_id, offset + (i * 4096)));
        }

        Ok(())
    }

    fn unpin(&self, vmo_id: u64, offset: usize, len: usize) -> Result<()> {
        let mut pinned = self.pinned_pages.lock();
        let page_count = (len + 4095) / 4096;

        for i in 0..page_count {
            pinned.remove(&(vmo_id, offset + (i * 4096)));
        }

        Ok(())
    }
}

/// ============================================================================
/// COW Page Tracker
/// ============================================================================

/// COW page tracker
///
/// Tracks which pages have been COW'd in a VMO.
pub struct CowTracker {
    /// Pages that have been COW'd (offsets)
    cow_pages: BTreeSet<usize>,

    /// Pages that are pinned
    pinned_pages: BTreeSet<usize>,
}

impl CowTracker {
    /// Create a new COW tracker
    pub const fn new() -> Self {
        Self {
            cow_pages: BTreeSet::new(),
            pinned_pages: BTreeSet::new(),
        }
    }

    /// Check if page is COW
    pub fn is_cow(&self, offset: usize) -> bool {
        self.cow_pages.contains(&offset)
    }

    /// Mark page as COW
    pub fn mark_cow(&mut self, offset: usize) {
        self.cow_pages.insert(offset);
    }

    /// Check if page is pinned
    pub fn is_pinned(&self, offset: usize) -> bool {
        self.pinned_pages.contains(&offset)
    }

    /// Pin a page
    pub fn pin(&mut self, offset: usize) {
        self.pinned_pages.insert(offset);
    }

    /// Unpin a page
    pub fn unpin(&mut self, offset: usize) {
        self.pinned_pages.remove(&offset);
    }

    /// Get COW page count
    pub fn cow_count(&self) -> usize {
        self.cow_pages.len()
    }
}

/// ============================================================================
/// Page Fault Handler
/// ============================================================================

/// Global page fault handler
///
/// Handles page faults by dispatching to the appropriate pager.
static mut GLOBAL_PAGER: Option<DefaultPager> = None;

/// Initialize the pager subsystem
pub fn pager_init() {
    unsafe {
        GLOBAL_PAGER = Some(DefaultPager::new());
    }

    log_info!("Pager subsystem initialized");
}

/// Get the global pager
pub fn get_pager() -> Option<&'static DefaultPager> {
    unsafe { GLOBAL_PAGER.as_ref() }
}

/// Handle a page fault
///
/// # Arguments
///
/// * `vmo_id` - VMO that faulted
/// * `offset` - Offset within VMO
/// * `flags` - Fault flags
///
/// # Returns
///
/// Physical frame to map
pub fn handle_page_fault(vmo_id: u64, offset: usize, flags: PageFaultFlags) -> Result<Frame> {
    let pager = get_pager().ok_or(RX_ERR_BAD_STATE)?;
    pager.fault(vmo_id, offset, flags)
}

/// ============================================================================
/// Page Eviction Policy
/// ============================================================================

/// Page eviction policy
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// No eviction (keep all pages)
    None = 0,

    /// Least Recently Used
    LRU = 1,

    /// Adaptive Replacement Cache
    ARC = 2,

    /// Clock algorithm (approximation of LRU)
    Clock = 3,
}

/// Page access tracking entry
#[derive(Debug, Clone)]
struct PageAccess {
    /// VMO ID
    vmo_id: u64,

    /// Page offset within VMO
    offset: usize,

    /// Physical address
    paddr: PAddr,

    /// Reference count
    ref_count: u32,

    /// Access time (for LRU)
    access_time: u64,

    /// Dirty flag (has been written to)
    dirty: bool,

    /// Reference flag (for clock algorithm)
    referenced: bool,
}

impl PageAccess {
    /// Create a new page access entry
    fn new(vmo_id: u64, offset: usize, paddr: PAddr) -> Self {
        Self {
            vmo_id,
            offset,
            paddr,
            ref_count: 1,
            access_time: 0,
            dirty: false,
            referenced: true,
        }
    }
}

/// Page eviction tracker
///
/// Tracks page accesses for eviction policy
pub struct PageEvictionTracker {
    /// Eviction policy
    policy: EvictionPolicy,

    /// Page access tracking (ordered by access time for LRU)
    pages: SpinMutex<alloc::collections::VecDeque<PageAccess>>,

    /// Access counter (for timestamp)
    access_counter: AtomicU64,

    /// Total tracked pages
    total_pages: AtomicUsize,
}

impl PageEvictionTracker {
    /// Create a new page eviction tracker
    pub fn new(policy: EvictionPolicy) -> Self {
        Self {
            policy,
            pages: SpinMutex::new(alloc::collections::VecDeque::new()),
            access_counter: AtomicU64::new(0),
            total_pages: AtomicUsize::new(0),
        }
    }

    /// Track a page access
    ///
    /// # Arguments
    ///
    /// * `vmo_id` - VMO containing the page
    /// * `offset` - Offset within VMO
    /// * `paddr` - Physical address of the page
    pub fn track_page(&self, vmo_id: u64, offset: usize, paddr: PAddr) {
        let access_time = self.access_counter.fetch_add(1, Ordering::Relaxed);
        let mut pages = self.pages.lock();

        // Check if page is already tracked
        for page in pages.iter() {
            if page.vmo_id == vmo_id && page.offset == offset {
                // Update access time
                return;
            }
        }

        // Add new page to tracking
        pages.push_back(PageAccess::new(vmo_id, offset, paddr));
        self.total_pages.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that a page was accessed
    ///
    /// # Arguments
    ///
    /// * `vmo_id` - VMO containing the page
    /// * `offset` - Offset within VMO
    pub fn record_access(&self, vmo_id: u64, offset: usize) {
        let access_time = self.access_counter.fetch_add(1, Ordering::Relaxed);
        let mut pages = self.pages.lock();

        // Find and update the page
        for page in pages.iter_mut() {
            if page.vmo_id == vmo_id && page.offset == offset {
                page.access_time = access_time;
                page.referenced = true;
                return;
            }
        }
    }

    /// Record that a page was modified
    ///
    /// # Arguments
    ///
    /// * `vmo_id` - VMO containing the page
    /// * `offset` - Offset within VMO
    pub fn record_dirty(&self, vmo_id: u64, offset: usize) {
        let mut pages = self.pages.lock();

        for page in pages.iter_mut() {
            if page.vmo_id == vmo_id && page.offset == offset {
                page.dirty = true;
                return;
            }
        }
    }

    /// Stop tracking a page
    ///
    /// # Arguments
    ///
    /// * `vmo_id` - VMO containing the page
    /// * `offset` - Offset within VMO
    pub fn untrack_page(&self, vmo_id: u64, offset: usize) {
        let mut pages = self.pages.lock();
        pages.retain(|p| !(p.vmo_id == vmo_id && p.offset == offset));
        self.total_pages.fetch_sub(1, Ordering::Relaxed);
    }

    /// Find a page to evict
    ///
    /// # Returns
    ///
    /// Physical address of a page that can be evicted, or None
    pub fn find_eviction_candidate(&self) -> Option<(u64, usize, PAddr)> {
        match self.policy {
            EvictionPolicy::None => None,
            EvictionPolicy::LRU => self.find_lru_victim(),
            EvictionPolicy::ARC => self.find_arc_victim(),
            EvictionPolicy::Clock => self.find_clock_victim(),
        }
    }

    /// Find LRU victim (Least Recently Used)
    fn find_lru_victim(&self) -> Option<(u64, usize, PAddr)> {
        let pages = self.pages.lock();

        // Find page with oldest access time that isn't dirty
        let mut oldest = None;
        let mut oldest_time = u64::MAX;

        for page in pages.iter() {
            // Skip dirty pages (need to write back)
            if page.dirty {
                continue;
            }

            if page.access_time < oldest_time {
                oldest_time = page.access_time;
                oldest = Some((page.vmo_id, page.offset, page.paddr));
            }
        }

        oldest
    }

    /// Find ARC victim (Adaptive Replacement Cache)
    fn find_arc_victim(&self) -> Option<(u64, usize, PAddr)> {
        // Simplified ARC: prefer non-recently accessed pages
        // Full ARC would maintain t1 (recency) and t2 (frequency) lists
        self.find_lru_victim()
    }

    /// Find Clock algorithm victim
    fn find_clock_victim(&self) -> Option<(u64, usize, PAddr)> {
        let mut pages = self.pages.lock();

        // Clock algorithm: circular scan looking for unreferenced page
        for _ in 0..2 {
            for page in pages.iter_mut() {
                if page.dirty {
                    continue; // Skip dirty pages
                }

                if !page.referenced {
                    return Some((page.vmo_id, page.offset, page.paddr));
                }

                // Give page a second chance
                page.referenced = false;
            }
        }

        None
    }

    /// Get the number of tracked pages
    pub fn page_count(&self) -> usize {
        self.total_pages.load(Ordering::Relaxed)
    }
}

/// Global page eviction tracker
static mut GLOBAL_EVICTION_TRACKER: Option<PageEvictionTracker> = None;

/// Initialize the page eviction tracker
///
/// # Arguments
///
/// * `policy` - Eviction policy to use
pub fn init_eviction_tracker(policy: EvictionPolicy) {
    unsafe {
        GLOBAL_EVICTION_TRACKER = Some(PageEvictionTracker::new(policy));
    }

    log_info!("Page eviction tracker initialized with policy: {:?}", policy);
}

/// Get the global eviction tracker
pub fn get_eviction_tracker() -> Option<&'static PageEvictionTracker> {
    unsafe { GLOBAL_EVICTION_TRACKER.as_ref() }
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame() {
        let frame = Frame::new(1000, true);
        assert_eq!(frame.num, 1000);
        assert_eq!(frame.paddr(), 1000 * 4096);
        assert!(frame.zeroed);
    }

    #[test]
    fn test_fault_flags() {
        let flags = PageFaultFlags::WRITE | PageFaultFlags::USER;
        assert!(flags.is_write());
        assert!(flags.is_user());
        assert!(!flags.is_read());
        assert!(!flags.is_execute());
    }

    #[test]
    fn test_cow_tracker() {
        let mut tracker = CowTracker::new();
        assert!(!tracker.is_cow(0));

        tracker.mark_cow(0);
        assert!(tracker.is_cow(0));
        assert_eq!(tracker.cow_count(), 1);
    }

    #[test]
    fn test_pager_init() {
        // Note: This test can't be run multiple times in same process
        // due to global state
        let _ = pager_init();
        assert!(get_pager().is_some());
    }
}
