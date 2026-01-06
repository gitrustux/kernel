// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Virtual Memory Objects (VMOs)
//!
//! VMOs represent contiguous regions of physical memory that can be
//! mapped into address spaces. They support COW cloning and resizing.
//!
//! # Design
//!
//! - **Page-based**: Memory is managed in page-sized chunks
//! - **COW clones**: Copy-on-write for efficient memory sharing
//! - **Resizable**: VMOs can grow/shrink if created with RESIZABLE flag
//! - **Cache policy**: Control cache behavior (uncached, write-combining, etc.)
//!
//! # Usage
//!
//! ```rust
//! let vmo = Vmo::create(0x1000, VmoFlags::empty())?;
//! vmo.write(0, &data)?;
//! vmo.read(0, &mut buf)?;
//! ```

#![no_std]

use crate::kernel::pmm;
use crate::kernel::sync::Mutex;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// ============================================================================
/// VMO ID
/// ============================================================================

/// VMO identifier
pub type VmoId = u64;

/// Next VMO ID counter
static mut NEXT_VMO_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new VMO ID
fn alloc_vmo_id() -> VmoId {
    unsafe { NEXT_VMO_ID.fetch_add(1, Ordering::Relaxed) }
}

/// ============================================================================
/// VMO Flags
/// ============================================================================

/// VMO creation flags
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmoFlags(pub u32);

impl VmoFlags {
    /// No flags
    pub const empty: Self = Self(0);

    /// VMO is resizable
    pub const RESIZABLE: Self = Self(0x01);

    /// VMO is a COW clone
    pub const COW: Self = Self(0x02);

    /// Check if resizable
    pub const fn is_resizable(self) -> bool {
        (self.0 & Self::RESIZABLE.0) != 0
    }

    /// Check if COW clone
    pub const fn is_cow(self) -> bool {
        (self.0 & Self::COW.0) != 0
    }

    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for VmoFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// ============================================================================
/// Cache Policy
/// ============================================================================

/// Cache policy for VMO mappings
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Default caching
    Default = 0,

    /// Uncached access
    Uncached = 1,

    /// Write-combining
    WriteCombining = 2,

    /// Write-through
    WriteThrough = 3,
}

impl CachePolicy {
    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Uncached,
            2 => Self::WriteCombining,
            3 => Self::WriteThrough,
            _ => Self::Default,
        }
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self as u32
    }
}

/// ============================================================================
/// Page Map
/// ============================================================================

/// Page map entry
#[derive(Debug)]
struct PageMapEntry {
    /// Physical page address
    paddr: PAddr,

    /// Whether page is present (not committed if COW)
    present: bool,

    /// Whether page is writable
    writable: bool,
}

/// Page map for VMO
///
/// Maps offset → physical page
pub struct PageMap {
    /// Map from page offset to physical address
    pages: Mutex<BTreeMap<usize, PageMapEntry>>,

    /// Total number of pages
    total_pages: usize,

    /// Number of committed pages
    committed_pages: AtomicUsize,
}

impl PageMap {
    /// Create a new page map
    pub const fn new(total_pages: usize) -> Self {
        Self {
            pages: Mutex::new(BTreeMap::new()),
            total_pages,
            committed_pages: AtomicUsize::new(0),
        }
    }

    /// Get a page at offset
    ///
    /// Returns None if page is not committed.
    pub fn get(&self, offset: usize) -> Option<PAddr> {
        let pages = self.pages.lock();
        pages.get(&offset).map(|entry| entry.paddr)
    }

    /// Allocate and commit a page at offset
    pub fn allocate(&self, offset: usize) -> Result<PAddr> {
        // Check if page already exists
        {
            let mut pages = self.pages.lock();
            if let Some(entry) = pages.get(&offset) {
                if entry.present {
                    return Ok(entry.paddr);
                }
            }
        }

        // Allocate new page
        let paddr = pmm::alloc_page()?;
        let vaddr = pmm::paddr_to_vaddr(paddr) as PAddr;

        // Add to map
        let mut pages = self.pages.lock();
        pages.insert(offset, PageMapEntry {
            paddr: vaddr,
            present: true,
            writable: true,
        });
        self.committed_pages.fetch_add(1, Ordering::Relaxed);

        Ok(vaddr)
    }

    /// Mark a page as copy-on-write
    pub fn mark_cow(&self, offset: usize) {
        let mut pages = self.pages.lock();
        if let Some(entry) = pages.get_mut(&offset) {
            entry.writable = false;
        }
    }

    /// Get number of committed pages
    pub fn committed_count(&self) -> usize {
        self.committed_pages.load(Ordering::Relaxed)
    }

    /// Get total page count
    pub const fn total_count(&self) -> usize {
        self.total_pages
    }
}

/// ============================================================================
/// VMO Parent
/// ============================================================================

/// VMO parent (for COW clones)
#[derive(Debug)]
pub struct VmoParent {
    /// Parent VMO reference
    pub vmo: VmoId,

    /// Offset in parent
    pub offset: usize,

    /// Whether this is a COW clone
    pub is_cow: bool,
}

/// ============================================================================
/// VMO
/// ============================================================================

/// Virtual Memory Object
///
/// Represents a contiguous region of physical memory.
pub struct Vmo {
    /// VMO ID
    pub id: VmoId,

    /// VMO size in bytes
    pub size: AtomicU64,

    /// VMO flags
    pub flags: VmoFlags,

    /// Page map
    pub pages: PageMap,

    /// Parent VMO (if clone)
    pub parent: Mutex<Option<VmoParent>>,

    /// Child clones
    pub children: Mutex<Vec<VmoId>>,

    /// Cache policy
    pub cache_policy: Mutex<CachePolicy>,

    /// Reference count
    pub ref_count: AtomicUsize,

    /// Address spaces this VMO is mapped into (for shared memory tracking)
    /// Stores AddressSpace IDs (or Process IDs)
    mapped_aspaces: Mutex<alloc::collections::BTreeSet<u64>>,
}

impl Vmo {
    /// Create a new VMO
    ///
    /// # Arguments
    ///
    /// * `size` - Size in bytes (must be page-aligned)
    /// * `flags` - VMO flags
    pub fn create(size: usize, flags: VmoFlags) -> Result<Self> {
        // Validate size
        if size == 0 || (size & 0xFFF) != 0 {
            return Err(RX_ERR_INVALID_ARGS);
        }

        // Validate flags
        if flags.is_cow() {
            return Err(RX_ERR_INVALID_ARGS); // Cannot create COW directly
        }

        let page_count = size / 4096;

        Ok(Self {
            id: alloc_vmo_id(),
            size: AtomicU64::new(size as u64),
            flags,
            pages: PageMap::new(page_count),
            parent: Mutex::new(None),
            children: Mutex::new(Vec::new()),
            cache_policy: Mutex::new(CachePolicy::Default),
            ref_count: AtomicUsize::new(1),
            mapped_aspaces: Mutex::new(alloc::collections::BTreeSet::new()),
        })
    }

    /// Get size
    pub fn size(&self) -> usize {
        self.size.load(Ordering::Acquire) as usize
    }

    /// Resize the VMO
    ///
    /// # Arguments
    ///
    /// * `new_size` - New size in bytes (must be page-aligned)
    pub fn resize(&self, new_size: usize) -> Result {
        if !self.flags.is_resizable() {
            return Err(RX_ERR_NOT_SUPPORTED);
        }

        if (new_size & 0xFFF) != 0 {
            return Err(RX_ERR_INVALID_ARGS);
        }

        self.size.store(new_size as u64, Ordering::Release);
        Ok(())
    }

    /// Read from VMO
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset to read from
    /// * `buf` - Buffer to read into
    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let size = self.size();

        // Validate offset
        if offset >= size {
            return Err(RX_ERR_OUT_OF_RANGE);
        }

        // Calculate actual read length
        let len = core::cmp::min(buf.len(), size - offset);
        let end_offset = offset + len;

        // Read pages
        let mut bytes_read = 0;
        let mut current_offset = offset;

        while current_offset < end_offset {
            let page_offset = current_offset & !0xFFF;
            let offset_in_page = current_offset & 0xFFF;

            // Get or allocate page
            let paddr = self.pages.allocate(page_offset / 4096)?;

            // Copy data from page
            let src = unsafe { (paddr as *const u8).add(offset_in_page) };
            let remaining = end_offset - current_offset;
            let to_copy = core::cmp::min(remaining, 4096 - offset_in_page);

            unsafe {
                core::ptr::copy_nonoverlapping(
                    src,
                    buf.as_mut_ptr().add(bytes_read),
                    to_copy,
                );
            }

            bytes_read += to_copy;
            current_offset += to_copy;
        }

        Ok(len)
    }

    /// Write to VMO
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset to write to
    /// * `buf` - Buffer to write from
    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let size = self.size();

        // Validate offset
        if offset >= size {
            return Err(RX_ERR_OUT_OF_RANGE);
        }

        // Calculate actual write length
        let len = core::cmp::min(buf.len(), size - offset);
        let end_offset = offset + len;

        // Write pages
        let mut bytes_written = 0;
        let mut current_offset = offset;

        while current_offset < end_offset {
            let page_offset = current_offset & !0xFFF;
            let offset_in_page = current_offset & 0xFFF;

            // Get or allocate page
            let paddr = self.pages.allocate(page_offset / 4096)?;

            // Copy data to page
            let dst = unsafe { (paddr as *mut u8).add(offset_in_page) };
            let remaining = end_offset - current_offset;
            let to_copy = core::cmp::min(remaining, 4096 - offset_in_page);

            unsafe {
                core::ptr::copy_nonoverlapping(
                    buf.as_ptr().add(bytes_written),
                    dst,
                    to_copy,
                );
            }

            bytes_written += to_copy;
            current_offset += to_copy;
        }

        Ok(len)
    }

    /// Set cache policy
    pub fn set_cache_policy(&self, policy: CachePolicy) {
        *self.cache_policy.lock() = policy;
    }

    /// Get cache policy
    pub fn cache_policy(&self) -> CachePolicy {
        *self.cache_policy.lock()
    }

    /// Clone this VMO (COW)
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset in parent
    /// * `size` - Size of clone
    pub fn clone(&self, offset: usize, size: usize) -> Result<Self> {
        // Validate parameters
        if offset + size > self.size() {
            return Err(RX_ERR_OUT_OF_RANGE);
        }

        let page_count = size / 4096;

        let vmo = Self {
            id: alloc_vmo_id(),
            size: AtomicU64::new(size as u64),
            flags: VmoFlags::COW,
            pages: PageMap::new(page_count),
            parent: Mutex::new(Some(VmoParent {
                vmo: self.id,
                offset,
                is_cow: true,
            })),
            children: Mutex::new(Vec::new()),
            cache_policy: Mutex::new(self.cache_policy()),
            ref_count: AtomicUsize::new(1),
            mapped_aspaces: Mutex::new(alloc::collections::BTreeSet::new()),
        };

        // Add as child
        self.children.lock().push(vmo.id);

        Ok(vmo)
    }

    /// Increment reference count
    pub fn ref_inc(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference.
    pub fn ref_dec(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Release) == 1
    }

    /// Add a mapping to an address space
    ///
    /// Called when this VMO is mapped into an address space.
    /// This tracks which address spaces have this VMO mapped.
    ///
    /// # Arguments
    ///
    /// * `aspace_id` - Address space ID (or Process ID)
    pub fn add_mapping(&self, aspace_id: u64) {
        self.mapped_aspaces.lock().insert(aspace_id);
    }

    /// Remove a mapping from an address space
    ///
    /// Called when this VMO is unmapped from an address space.
    ///
    /// # Arguments
    ///
    /// * `aspace_id` - Address space ID (or Process ID)
    pub fn remove_mapping(&self, aspace_id: u64) {
        self.mapped_aspaces.lock().remove(&aspace_id);
    }

    /// Get share count
    ///
    /// Returns the number of unique address spaces this VMO is mapped into.
    /// A VMO is "shared" if this count is >= 2.
    pub fn share_count(&self) -> u32 {
        let count = self.mapped_aspaces.lock().len();
        if count < 2 {
            1  // Not shared (or mapped into single address space)
        } else {
            count as u32
        }
    }

    /// Check if this VMO is shared
    ///
    /// Returns true if this VMO is mapped into multiple address spaces.
    pub fn is_shared(&self) -> bool {
        self.share_count() >= 2
    }
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vmo_flags() {
        let flags = VmoFlags::RESIZABLE | VmoFlags::COW;
        assert!(flags.is_resizable());
        assert!(flags.is_cow());
    }

    #[test]
    fn test_vmo_create() {
        let vmo = Vmo::create(0x1000, VmoFlags::empty()).unwrap();
        assert_eq!(vmo.size(), 0x1000);
        assert!(!vmo.flags.is_resizable());
    }

    #[test]
    fn test_vmo_resize() {
        let vmo = Vmo::create(0x1000, VmoFlags::RESIZABLE).unwrap();
        assert!(vmo.resize(0x2000).is_ok());
        assert_eq!(vmo.size(), 0x2000);
    }

    #[test]
    fn test_cache_policy() {
        let policy = CachePolicy::Uncached;
        assert_eq!(CachePolicy::from_raw(1), policy);
        assert_eq!(policy.into_raw(), 1);
    }
}
