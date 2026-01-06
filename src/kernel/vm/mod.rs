// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Virtual Memory Subsystem
//!
//! This module provides architecture-agnostic virtual memory management for the Rustux kernel.
//! It implements uniform semantics across ARM64, AMD64, and RISC-V architectures.
//!
//! # Design Goals
//!
//! 1. **Uniform semantics** - Same rules for mapping, permissions, sharing, COW, paging
//! 2. **Object-based memory** - Memory represented as VM Objects (VMOs)
//! 3. **Deterministic behavior** - No implicit over-commit without policy choice
//! 4. **Explicit operations** - No hidden mappings or kernel-magic ownership changes
//!
//! # Organization
//!
//! - [`layout`] - Virtual address layout definitions
//! - [`page_table`] - Cross-architecture page table abstraction
//! - [`aspace`] - Address space management
//! - [`vmo`] - Virtual Memory Objects

#![no_std]

pub mod layout;
pub mod page_table;
pub mod aspace;
pub mod arch_vm_aspace;
pub mod boottables;
pub mod debug;
pub mod stacks;
pub mod pager;
pub mod stats;
pub mod fault;
pub mod walker;

// Re-exports for convenience
pub use layout::{
    VAddr,
    PAddr,
    PAGE_SIZE,
    PAGE_SIZE_SHIFT,
    PAGE_MASK,
    MemProt,
    MemRegion,
    RegionType,
    Asid,
    ASID_INVALID,
    is_kernel_vaddr,
    is_user_vaddr,
    is_canonical_vaddr,
    page_align_down,
    page_align_up,
    is_page_aligned,
};

pub use page_table::{
    PageTable,
    PageTableFlags,
    PageTableEntry,
    MappingType,
};

pub use aspace::{
    AddressSpace,
    AddressSpaceFlags,
};

/// Virtual memory errors
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmError {
    /// Success
    Ok = 0,

    /// Invalid argument
    InvalidArgs = 1,

    /// Out of memory
    NoMemory = 2,

    /// Invalid address
    InvalidAddress = 3,

    /// Access denied
    AccessDenied = 4,

    /// Already mapped
    AlreadyMapped = 5,

    /// Not mapped
    NotMapped = 6,

    /// Page fault
    PageFault = 7,

    /// Alignment error
    AlignmentError = 8,

    /// Not found
    NotFound = 9,

    /// Permission denied
    PermissionDenied = 10,

    /// Resource busy
    Busy = 11,

    /// Invalid state
    BadState = 12,
}

impl VmError {
    /// Check if operation succeeded
    pub const fn is_ok(self) -> bool {
        self as i32 == 0
    }

    /// Convert to raw status code
    pub const fn as_raw(self) -> i32 {
        self as i32
    }
}

// Convert VmError to Status for use with ? operator
impl From<VmError> for crate::rustux::types::Status {
    fn from(err: VmError) -> Self {
        err as i32
    }
}

/// Result type for VM operations
pub type Result<T = ()> = core::result::Result<T, VmError>;

/// VM module initialization
///
/// Must be called early in boot to set up the virtual memory subsystem.
pub fn init() {
    layout::validate_layout();
    page_table::init();
    aspace::init();
}

// ============================================================================
// Trait Definitions for Cross-Architecture Abstractions
// ============================================================================

/// Architecture-specific page table operations
///
/// This trait must be implemented for each supported architecture.
pub trait ArchPageTable: Sized {
    /// Page table entry type
    type Entry: PageTableEntry;

    /// Create a new page table
    fn new() -> Result<Self>;

    /// Map a page with the specified flags
    fn map(&mut self, vaddr: VAddr, paddr: PAddr, flags: PageTableFlags) -> Result;

    /// Unmap a page
    fn unmap(&mut self, vaddr: VAddr) -> Result;

    /// Resolve a virtual address to physical address
    fn resolve(&self, vaddr: VAddr) -> Option<PAddr>;

    /// Update page table entry flags
    fn protect(&mut self, vaddr: VAddr, flags: PageTableFlags) -> Result;

    /// Flush TLB entries for this page table
    fn flush_tlb(&self, vaddr: Option<VAddr>);

    /// Get the physical address of the root page table
    fn root_phys(&self) -> PAddr;
}

/// Memory mapping operations
pub trait Mapping {
    /// Get the virtual address base of this mapping
    fn base(&self) -> VAddr;

    /// Get the size of this mapping
    fn size(&self) -> usize;

    /// Get the memory protection flags
    fn prot(&self) -> MemProt;

    /// Check if this mapping contains a virtual address
    fn contains(&self, vaddr: VAddr) -> bool {
        let end = self.base().saturating_add(self.size());
        vaddr >= self.base() && vaddr < end
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Validate that a virtual address range is valid for the given address space
pub fn validate_vaddr_range(vaddr: VAddr, size: usize) -> Result {
    // Check for overflow
    if vaddr.saturating_add(size) < vaddr {
        return Err(VmError::InvalidArgs);
    }

    // Check alignment
    if size != 0 && !is_page_aligned(vaddr) {
        return Err(VmError::AlignmentError);
    }

    // Check if canonical
    if !is_canonical_vaddr(vaddr) {
        return Err(VmError::InvalidAddress);
    }

    Ok(())
}

/// Calculate number of pages needed for a given size
pub const fn bytes_to_pages(size: usize) -> usize {
    (size + PAGE_SIZE - 1) / PAGE_SIZE
}

/// Convert page count to bytes
pub const fn pages_to_bytes(pages: usize) -> usize {
    pages * PAGE_SIZE
}

/// Virtual address to physical address translation (kernel physmap only)
///
/// # Safety
///
/// This function assumes the physical mapping window is set up.
pub unsafe fn physmap_virt_to_phys(vaddr: VAddr) -> Option<PAddr> {
    #[cfg(target_arch = "aarch64")]
    {
        let base = layout::arm64::KERNEL_PHYSMAP_BASE;
        let size = layout::arm64::KERNEL_PHYSMAP_SIZE;
        if vaddr >= base && vaddr < base + size {
            Some(vaddr - base)
        } else {
            None
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        let base = layout::amd64::KERNEL_PHYSMAP_BASE;
        let size = layout::amd64::KERNEL_PHYSMAP_SIZE;
        if vaddr >= base && vaddr < base + size {
            Some(vaddr - base)
        } else {
            None
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        let base = layout::riscv64::KERNEL_PHYSMAP_BASE;
        let size = layout::riscv64::KERNEL_PHYSMAP_SIZE;
        if vaddr >= base && vaddr < base + size {
            Some(vaddr - base)
        } else {
            None
        }
    }
}

/// Physical address to virtual address (kernel physmap only)
pub fn phys_to_physmap(paddr: PAddr) -> VAddr {
    #[cfg(target_arch = "aarch64")]
    {
        layout::arm64::KERNEL_PHYSMAP_BASE + paddr
    }

    #[cfg(target_arch = "x86_64")]
    {
        layout::amd64::KERNEL_PHYSMAP_BASE + paddr
    }

    #[cfg(target_arch = "riscv64")]
    {
        layout::riscv64::KERNEL_PHYSMAP_BASE + paddr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_alignment() {
        assert_eq!(page_align_down(0x1000), 0x1000);
        assert_eq!(page_align_down(0x1FFF), 0x1000);
        assert_eq!(page_align_down(0x2000), 0x2000);

        assert_eq!(page_align_up(0x1000), 0x1000);
        assert_eq!(page_align_up(0x1001), 0x2000);
        assert_eq!(page_align_up(0x1FFF), 0x2000);

        assert!(is_page_aligned(0x1000));
        assert!(is_page_aligned(0x2000));
        assert!(!is_page_aligned(0x1001));
        assert!(!is_page_aligned(0x1FFF));
    }

    #[test]
    fn test_mem_prot() {
        assert!(MemProt::READ.can_read());
        assert!(!MemProt::READ.can_write());
        assert!(!MemProt::READ.can_execute());

        assert!(MemProt::WRITE.can_write());
        assert!(MemProt::WRITE.can_read()); // Write implies read

        assert!(MemProt::EXEC.can_execute());
        assert!(!MemProt::EXEC.can_write());

        // W^X validation
        assert!(MemProt::READ.is_valid_wxorx());
        assert!(MemProt::WRITE.is_valid_wxorx());
        assert!(MemProt::EXEC.is_valid_wxorx());
        assert!((MemProt::WRITE | MemProt::EXEC).is_valid_wxorx()); // Invalid!
    }
}
