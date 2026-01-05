// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Physical Memory Manager (PMM)
//!
//! This module provides physical memory allocation services for the kernel.
//! It manages physical memory pages, tracking which pages are free or allocated.
//!
//! # Design
//!
//! The PMM uses a bitmap allocator to track physical memory pages. Each bit
//! in the bitmap represents one physical page (typically 4KB). This design
//! prioritizes simplicity over raw performance for the initial implementation.
//!
//! # Memory Layout
//!
//! - Base addresses and sizes are platform-specific, provided via boot info
//! - Multiple memory arenas can be registered (e.g., low memory, high memory)
//! - Pages are tracked in `Page` structures with state information
//!
//! # Usage
//!
//! ```rust
//! // Allocate a single page
//! let page = pmm_alloc_page()?;
//!
//! // Allocate multiple contiguous pages
//! let pages = pmm_alloc_contiguous(10, 12)?; // 10 pages, 4KB aligned
//!
//! // Free pages
//! pmm_free_page(page);
//! ```

#![no_std]

use crate::rustux::types::*;
use crate::rustux::types::err::*;
use crate::rustux::errors::*;
// Use fully qualified Result to avoid ambiguity
use crate::rustux::types::Result;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::sync::atomic::{compiler_fence, fence};

/// Page size in bytes (4KB is standard for all architectures)
pub const PAGE_SIZE: usize = 4096;

/// Page size shift for quick division/multiplication
pub const PAGE_SIZE_SHIFT: u8 = 12;

/// Mask for page-aligned addresses
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

/// Maximum number of physical memory arenas
const MAX_ARENAS: usize = 8;

/// Check if an address is page-aligned
#[inline]
pub const fn is_page_aligned(addr: usize) -> bool {
    (addr & PAGE_MASK) == 0
}

/// Align an address down to page boundary
#[inline]
pub const fn align_page_down(addr: usize) -> usize {
    addr & !PAGE_MASK
}

/// Align an address up to page boundary
#[inline]
pub const fn align_page_up(addr: usize) -> usize {
    (addr + PAGE_MASK) & !PAGE_MASK
}

/// Convert bytes to number of pages (rounding up)
#[inline]
pub const fn bytes_to_pages(bytes: usize) -> usize {
    (bytes + PAGE_MASK) / PAGE_SIZE
}

/// Convert pages to bytes
#[inline]
pub const fn pages_to_bytes(pages: usize) -> usize {
    pages << PAGE_SIZE_SHIFT
}

/// Page state enumeration
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageState {
    /// Page is free and can be allocated
    Free = 0,
    /// Page is allocated (general purpose)
    Allocated = 1,
    /// Page is reserved (cannot be allocated)
    Reserved = 2,
    /// Page is used for kernel image
    Kernel = 3,
    /// Page is used for MMU structures
    Mmu = 4,
    /// Page is used for IOMMU
    Iommu = 5,
}

/// Physical memory arena information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArenaInfo {
    /// Arena name (e.g., "low_mem", "high_mem")
    pub name: [u8; 16],

    /// Arena flags (e.g., LOW_MEM, HIGH_MEM)
    pub flags: u32,

    /// Arena allocation priority (lower = higher priority)
    pub priority: u32,

    /// Base physical address
    pub base: PAddr,

    /// Size in bytes
    pub size: usize,
}

impl ArenaInfo {
    /// Create a new arena info structure
    pub const fn new(name: &[u8], flags: u32, priority: u32, base: PAddr, size: usize) -> Self {
        let mut name_bytes = [0u8; 16];
        let mut i = 0;
        while i < 16 && i < name.len() {
            name_bytes[i] = name[i];
            i += 1;
        }

        Self {
            name: name_bytes,
            flags,
            priority,
            base,
            size,
        }
    }

    /// Get the number of pages in this arena
    pub fn page_count(&self) -> usize {
        self.size / PAGE_SIZE
    }

    /// Get the end physical address (exclusive)
    pub fn end(&self) -> PAddr {
        self.base + self.size as PAddr
    }
}

/// Arena allocation flags
pub const ARENA_FLAG_LOW_MEM: u32 = 0x1;   // Arena is in "low memory" (< 4GB)
pub const ARENA_FLAG_HIGH_MEM: u32 = 0x2;  // Arena is in "high memory" (>= 4GB)

/// PMM allocation flags
pub const PMM_ALLOC_FLAG_ANY: u32 = 0x0;      // Allocate from any arena
pub const PMM_ALLOC_FLAG_LOW_MEM: u32 = 0x1; // Allocate only from low memory arenas

/// Page structure tracking a single physical page
#[repr(C)]
#[derive(Debug)]
pub struct Page {
    /// Physical address of this page
    pub paddr: PAddr,

    /// Current state of the page
    pub state: PageState,

    /// Reference count for shared pages
    pub ref_count: u32,

    /// Arena index this page belongs to
    pub arena_index: u8,

    /// Page index within arena
    pub page_index: u32,
}

impl Page {
    /// Create a new page structure
    pub const fn new(paddr: PAddr, arena_index: u8, page_index: u32) -> Self {
        Self {
            paddr,
            state: PageState::Free,
            ref_count: 0,
            arena_index,
            page_index,
        }
    }

    /// Check if this page is free
    pub fn is_free(&self) -> bool {
        self.state == PageState::Free
    }

    /// Get the virtual address for this page (for direct-mapped regions)
    ///
    /// # Safety
    ///
    /// This function assumes a direct physical mapping exists.
    pub unsafe fn virt_addr(&self) -> VAddr {
        self.paddr as VAddr
    }
}

/// Memory arena structure
struct Arena {
    /// Arena information
    info: ArenaInfo,

    /// Array of page structures
    pages: Option<&'static mut [Page]>,

    /// Bitmap tracking free pages (0 = free, 1 = allocated)
    bitmap: Option<&'static mut [u64]>,

    /// Number of free pages
    free_count: AtomicU64,

    /// Total number of pages
    total_count: u64,

    /// Lock for arena operations
    locked: AtomicBool,
}

impl Arena {
    /// Create a new arena
    const fn new(info: ArenaInfo) -> Self {
        Self {
            info,
            pages: None,
            bitmap: None,
            free_count: AtomicU64::new(0),
            total_count: 0,
            locked: AtomicBool::new(false),
        }
    }

    /// Initialize the arena with page structures and bitmap
    fn init(&mut self, pages: &'static mut [Page], bitmap: &'static mut [u64]) {
        self.total_count = pages.len() as u64;
        self.free_count.store(pages.len() as u64, Ordering::Relaxed);
        self.pages = Some(pages);
        self.bitmap = Some(bitmap);
    }

    /// Allocate a single page from this arena
    fn alloc_page(&self) -> Option<PAddr> {
        if let Some(bitmap) = self.bitmap {
            for (i, &word) in bitmap.iter().enumerate() {
                if word != !0u64 {
                    // Found a free bit (0 = free)
                    let bit = (!word).trailing_zeros() as u32;
                    let index = (i * 64) + bit as usize;

                    if index < self.total_count as usize {
                        // Try to allocate using compare-and-swap
                        let mask = 1u64 << bit;
                        let old = bitmap[i].fetch_or(mask, Ordering::Acquire);

                        if (old & mask) == 0 {
                            // Successfully allocated
                            self.free_count.fetch_sub(1, Ordering::Relaxed);

                            // Update page state
                            if let Some(pages) = self.pages {
                                pages[index].state = PageState::Allocated;
                                pages[index].ref_count = 1;
                                return Some(self.info.base + (index as PAddr) * PAGE_SIZE as PAddr);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Free a page back to this arena
    fn free_page(&self, paddr: PAddr) -> Result {
        let offset = paddr - self.info.base;
        if offset % PAGE_SIZE as PAddr != 0 {
            return Err(RX_ERR_INVALID_ARGS);
        }

        let index = (offset / PAGE_SIZE as PAddr) as usize;
        if index >= self.total_count as usize {
            return Err(RX_ERR_INVALID_ARGS);
        }

        // Update bitmap
        if let Some(bitmap) = self.bitmap {
            let word_index = index / 64;
            let bit = index % 64;

            // Clear the bit to mark as free
            bitmap[word_index].fetch_and(!(1u64 << bit), Ordering::Release);
        }

        // Update page state
        if let Some(pages) = self.pages {
            pages[index].state = PageState::Free;
            pages[index].ref_count = 0;
        }

        self.free_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Get the number of free pages
    fn free_count(&self) -> u64 {
        self.free_count.load(Ordering::Relaxed)
    }

    /// Get the total number of pages
    fn total_count(&self) -> u64 {
        self.total_count
    }
}

/// Global PMM state
static mut ARENAS: [Arena; MAX_ARENAS] = [
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
    Arena::new(ArenaInfo {
        name: [0; 16],
        flags: 0,
        priority: 0,
        base: 0,
        size: 0,
    }),
];

static mut NUM_ARENAS: usize = 0;

/// Add a memory arena to the PMM
///
/// # Arguments
///
/// * `info` - Arena information (base address, size, flags)
///
/// # Returns
///
/// `RX_OK` on success, or an error code on failure
///
/// # Safety
///
/// This function modifies global PMM state and should only be called during boot.
pub unsafe fn pmm_add_arena(info: ArenaInfo) -> rx_status_t {
    if NUM_ARENAS >= MAX_ARENAS {
        return RX_ERR_NO_RESOURCES;
    }

    let page_count = info.page_count();
    if page_count == 0 {
        return RX_ERR_INVALID_ARGS;
    }

    // Allocate page structures array
    let pages_layout = core::alloc::Layout::array::<Page>(page_count);
    let pages = core::ptr::slice_from_raw_parts_mut(
        pages_alloc(pages_layout) as *mut Page,
        page_count,
    );

    // Initialize page structures
    for i in 0..page_count {
        pages[i] = Page::new(info.base + (i as PAddr) * PAGE_SIZE as PAddr, NUM_ARENAS as u8, i as u32);
    }

    // Allocate bitmap (64 bits per u64, one bit per page)
    let bitmap_count = (page_count + 63) / 64;
    let bitmap_layout = core::alloc::Layout::array::<u64>(bitmap_count);
    let bitmap = core::ptr::slice_from_raw_parts_mut(
        pages_alloc(bitmap_layout) as *mut u64,
        bitmap_count,
    );

    // Initialize bitmap to all free (all zeros)
    for word in bitmap.iter_mut() {
        *word = 0;
    }

    // Initialize arena
    let arena = &mut ARENAS[NUM_ARENAS];
    arena.info = info;
    arena.init(pages, bitmap);

    NUM_ARENAS += 1;
    RX_OK
}

/// Allocate a single physical page
///
/// # Arguments
///
/// * `flags` - Allocation flags (PMM_ALLOC_FLAG_*)
///
/// # Returns
///
/// Physical address of the allocated page, or an error
pub fn pmm_alloc_page(flags: u32) -> Result<PAddr> {
    let arenas = unsafe { &ARENAS[..NUM_ARENAS] };

    // Try to allocate from matching arenas
    for arena in arenas {
        if flags == PMM_ALLOC_FLAG_LOW_MEM && (arena.info.flags & ARENA_FLAG_LOW_MEM) == 0 {
            continue;
        }

        if let Some(paddr) = arena.alloc_page() {
            return Ok(paddr);
        }
    }

    Err(RX_ERR_NO_MEMORY)
}

/// Allocate multiple contiguous physical pages
///
/// # Arguments
///
/// * `count` - Number of pages to allocate
/// * `flags` - Allocation flags
/// * `align_log2` - Alignment as log2 (0 = page aligned, 12 = 4KB, etc.)
///
/// # Returns
///
/// Physical address of the allocated region, or an error
pub fn pmm_alloc_contiguous(count: usize, flags: u32, align_log2: u8) -> Result<PAddr> {
    if count == 0 {
        return Err(RX_ERR_INVALID_ARGS);
    }

    // For single pages, use the regular allocator
    if count == 1 && align_log2 <= PAGE_SIZE_SHIFT {
        return pmm_alloc_page(flags);
    }

    // TODO: Implement proper contiguous allocation
    // For now, fail
    Err(RX_ERR_NOT_SUPPORTED)
}

/// Free a physical page
///
/// # Arguments
///
/// * `paddr` - Physical address of the page to free
///
/// # Returns
///
/// `RX_OK` on success, or an error code
pub fn pmm_free_page(paddr: PAddr) -> rx_status_t {
    let arenas = unsafe { &ARENAS[..NUM_ARENAS] };

    // Find the arena containing this page
    for arena in arenas {
        if paddr >= arena.info.base && paddr < arena.info.end() {
            return arena.free_page(paddr);
        }
    }

    RX_ERR_INVALID_ARGS
}

/// Get the number of free pages across all arenas
pub fn pmm_count_free_pages() -> u64 {
    let arenas = unsafe { &ARENAS[..NUM_ARENAS] };
    let mut count = 0u64;

    for arena in arenas {
        count += arena.free_count();
    }

    count
}

/// Get the total number of pages across all arenas
pub fn pmm_count_total_pages() -> u64 {
    let arenas = unsafe { &ARENAS[..NUM_ARENAS] };
    let mut count = 0u64;

    for arena in arenas {
        count += arena.total_count();
    }

    count
}

/// Get the total amount of physical memory in bytes
pub fn pmm_count_total_bytes() -> u64 {
    pmm_count_total_pages() * PAGE_SIZE as u64
}

/// Allocate memory for page structures or bitmap
///
/// This is a helper function that allocates memory during PMM initialization.
/// It uses the boot allocator.
fn pages_alloc(layout: core::alloc::Layout) -> *mut u8 {
    extern "C" {
        fn boot_alloc_mem(size: usize, align: usize) -> *mut u8;
    }

    unsafe { boot_alloc_mem(layout.size(), layout.align()) }
}

/// Convert physical address to page structure
///
/// # Arguments
///
/// * `paddr` - Physical address
///
/// # Returns
///
/// Pointer to the page structure, or null if not found
pub fn paddr_to_page(paddr: PAddr) -> *mut Page {
    let arenas = unsafe { &ARENAS[..NUM_ARENAS] };

    for arena in arenas {
        if paddr >= arena.info.base && paddr < arena.info.end() {
            let offset = paddr - arena.info.base;
            let index = (offset / PAGE_SIZE as PAddr) as usize;

            if let Some(pages) = arena.pages {
                if index < pages.len() {
                    return &pages[index] as *const _ as *mut _;
                }
            }
        }
    }

    core::ptr::null_mut()
}

/// Initialize the PMM with default low and high memory arenas
///
/// This is a convenience function for typical x86-64/ARM64 systems
/// with low memory (< 4GB) and high memory regions.
///
/// # Arguments
///
/// * `low_base` - Base address of low memory
/// * `low_size` - Size of low memory
/// * `high_base` - Base address of high memory (optional)
/// * `high_size` - Size of high memory (optional)
///
/// # Safety
///
/// This function modifies global PMM state and should only be called during boot.
pub unsafe fn pmm_init_early(
    low_base: PAddr,
    low_size: usize,
    high_base: Option<PAddr>,
    high_size: Option<usize>,
) {
    // Add low memory arena
    let low_info = ArenaInfo::new(
        b"low_mem\0",
        ARENA_FLAG_LOW_MEM,
        0, // highest priority
        low_base,
        low_size,
    );
    pmm_add_arena(low_info);

    // Add high memory arena if specified
    if let (Some(base), Some(size)) = (high_base, high_size) {
        let high_info = ArenaInfo::new(
            b"high_mem\0",
            ARENA_FLAG_HIGH_MEM,
            1, // lower priority
            base,
            size,
        );
        pmm_add_arena(high_info);
    }
}

// ============================================================================
// Convenience Wrapper Functions
// ============================================================================

/// Allocate a single page
///
/// Convenience wrapper for pmm_alloc_page.
pub fn alloc_page() -> Result<PAddr> {
    pmm_alloc_page(0)
}

/// Free a single page
///
/// Convenience wrapper for pmm_free_page.
pub fn free_page(paddr: PAddr) {
    let _ = pmm_free_page(paddr);
}

/// Convert physical address to virtual address
///
/// For now, this is a simple identity mapping. In a real system,
/// this would use the kernel's direct mapping region.
pub fn paddr_to_vaddr(paddr: PAddr) -> VAddr {
    paddr as VAddr
}
