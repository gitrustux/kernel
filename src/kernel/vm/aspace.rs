// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Address Space Management
//!
//! This module provides cross-architecture address space management for the Rustux kernel.
//! Address spaces represent the virtual memory layout for processes or the kernel itself.
//!
//! # Design
//!
//! - Each address space has an ASID (Address Space ID) for hardware TLB tagging
//! - Address spaces are reference-counted and managed separately from processes
//! - Mappings are tracked in a sorted tree for efficient range operations
//! - Support for both user and kernel address spaces
//!
//! # Thread Safety
//!
//! Address spaces use interior mutability with mutexes to allow safe concurrent access.


use crate::kernel::vm::layout::*;
use crate::kernel::vm::page_table::*;
use crate::kernel::vm::{VmError, Result};
use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use crate::kernel::sync::spin::SpinMutex;
use crate::kernel::sync::Mutex;

/// ============================================================================
/// Address Space Flags
/// ============================================================================

/// Address space flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpaceFlags {
    /// No special flags
    None = 0,

    /// Kernel address space (not user-accessible)
    Kernel = 1 << 0,

    /// User address space
    User = 1 << 1,

    /// Guest address space (for virtualization)
    Guest = 1 << 2,

    /// Physical address space (for direct device access)
    Physical = 1 << 3,
}

impl AddressSpaceFlags {
    /// No flags
    pub const NONE: u32 = 0;

    /// Check if this is a kernel address space
    pub const fn is_kernel(self) -> bool {
        (self as u32 & Self::Kernel as u32) != 0
    }

    /// Check if this is a user address space
    pub const fn is_user(self) -> bool {
        (self as u32 & Self::User as u32) != 0
    }

    /// Check if this is a guest address space
    pub const fn is_guest(self) -> bool {
        (self as u32 & Self::Guest as u32) != 0
    }
}

/// ============================================================================
/// Mapping Descriptor
/// ============================================================================

/// A memory mapping within an address space
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VmMapping {
    /// Virtual address base
    pub base: VAddr,

    /// Size in bytes (must be page-aligned)
    pub size: usize,

    /// Physical address or VMO offset
    pub paddr_or_offset: PAddr,

    /// Memory protection flags
    pub prot: MemProt,

    /// Page table flags
    pub pt_flags: PageTableFlags,

    /// Mapping flags
    pub flags: MappingFlags,
}

impl VmMapping {
    /// Create a new mapping descriptor
    pub const fn new(
        base: VAddr,
        size: usize,
        paddr_or_offset: PAddr,
        prot: MemProt,
        flags: MappingFlags,
    ) -> Self {
        Self {
            base,
            size,
            paddr_or_offset,
            prot,
            pt_flags: PageTableFlags::from_bits(PageTableFlags::from_prot(prot)),
            flags,
        }
    }

    /// Get the end address (exclusive)
    pub const fn end(&self) -> VAddr {
        self.base + self.size
    }

    /// Check if this mapping contains a virtual address
    pub fn contains(&self, vaddr: VAddr) -> bool {
        vaddr >= self.base && vaddr < self.end()
    }

    /// Check if this mapping overlaps with another
    pub fn overlaps(&self, other: &VmMapping) -> bool {
        self.base < other.end() && self.end() > other.base
    }
}

/// Mapping flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingFlags {
    /// No special flags
    None = 0,

    /// Fixed mapping (don't auto-select address)
    Fixed = 1 << 0,

    /// Map at specific address only
    Exact = 1 << 1,

    /// Anonymous mapping (not file-backed)
    Anonymous = 1 << 2,

    /// Private mapping (COW)
    Private = 1 << 3,

    /// Shared mapping
    Shared = 1 << 4,

    /// Grow down (stack-like)
    GrowDown = 1 << 5,
}

/// ============================================================================
/// Address Space Structure
/// ============================================================================

/// Address space identifier allocator
static ASID_ALLOCATOR: AsidAllocator = AsidAllocator::new();

/// ASID allocator structure
struct AsidAllocator {
    next: AtomicU16,
    max: AtomicU16,
}

impl AsidAllocator {
    const fn new() -> Self {
        Self {
            next: AtomicU16::new(1),
            max: AtomicU16::new(0xFFFF),
        }
    }

    fn allocate(&self) -> Asid {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            // Handle wraparound - restart at 1
            self.next.store(1, Ordering::Relaxed);
            1
        } else {
            id
        }
    }

    fn free(&self, _asid: Asid) {
        // For now, ASIDs are not reclaimed
        // A proper implementation would track free ASIDs
    }
}

/// Address space structure
///
/// Each address space represents a virtual memory context with its own
/// page tables and ASID.
pub struct AddressSpace {
    /// Page table for this address space
    page_table: Mutex<PageTable>,

    /// Address space flags
    flags: AddressSpaceFlags,

    /// Address space ID
    asid: Asid,

    /// Base virtual address
    base: VAddr,

    /// Size in bytes
    size: usize,

    /// Reference count
    ref_count: AtomicU64,
}

impl AddressSpace {
    /// Create a new address space
    pub fn new(flags: AddressSpaceFlags, base: VAddr, size: usize) -> Result<Self> {
        // Validate base and size
        if !is_canonical_vaddr(base) {
            return Err(VmError::InvalidAddress);
        }

        if base + size < base {
            return Err(VmError::InvalidArgs); // Overflow
        }

        // Create page table
        let page_table = if flags.is_kernel() {
            PageTable::new_kernel()?
        } else {
            PageTable::new()?
        };

        // Allocate ASID
        let asid = ASID_ALLOCATOR.allocate();

        let mut aspace = Self {
            page_table: Mutex::new(page_table),
            flags,
            asid,
            base,
            size,
            ref_count: AtomicU64::new(1),
        };

        // Initialize page table ASID
        aspace.page_table.lock().set_asid(asid);

        Ok(aspace)
    }

    /// Create a new user address space
    pub fn new_user() -> Result<Self> {
        #[cfg(target_arch = "aarch64")]
        let (base, size) = (arm64::USER_BASE, arm64::USER_MAX as usize);

        #[cfg(target_arch = "x86_64")]
        let (base, size) = (amd64::USER_BASE, amd64::USER_MAX as usize);

        #[cfg(target_arch = "riscv64")]
        let (base, size) = (riscv::USER_BASE, riscv::USER_MAX as usize);

        Self::new(AddressSpaceFlags::User, base, size)
    }

    /// Create a new kernel address space
    pub fn new_kernel() -> Result<Self> {
        #[cfg(target_arch = "aarch64")]
        let (base, size) = (arm64::KERNEL_BASE, arm64::KERNEL_SIZE);

        #[cfg(target_arch = "x86_64")]
        let (base, size) = (amd64::KERNEL_BASE, amd64::KERNEL_SIZE);

        #[cfg(target_arch = "riscv64")]
        let (base, size) = (riscv::KERNEL_BASE, riscv::KERNEL_SIZE);

        Self::new(AddressSpaceFlags::Kernel, base, size)
    }

    /// Get the ASID for this address space
    pub fn asid(&self) -> Asid {
        self.asid
    }

    /// Get the flags for this address space
    pub fn flags(&self) -> AddressSpaceFlags {
        self.flags
    }

    /// Get the base virtual address
    pub fn base(&self) -> VAddr {
        self.base
    }

    /// Get the size in bytes
    pub fn size(&self) -> usize {
        self.size
    }

    /// Check if this is a kernel address space
    pub fn is_kernel(&self) -> bool {
        self.flags.is_kernel()
    }

    /// Check if this is a user address space
    pub fn is_user(&self) -> bool {
        self.flags.is_user()
    }

    /// Check if a virtual address is valid for this address space
    pub fn is_valid_vaddr(&self, vaddr: VAddr) -> bool {
        vaddr >= self.base && vaddr < self.base + self.size
    }

    /// Map pages into this address space
    pub fn map(
        &self,
        vaddr: VAddr,
        paddr: PAddr,
        count: usize,
        prot: MemProt,
    ) -> Result {
        if !self.is_valid_vaddr(vaddr) {
            return Err(VmError::InvalidAddress);
        }

        // Validate the range doesn't overflow
        let end = vaddr.saturating_add(count * PAGE_SIZE);
        if end < vaddr || !self.is_valid_vaddr(end.saturating_sub(1)) {
            return Err(VmError::InvalidAddress);
        }

        let flags = PageTableFlags::from_prot(prot);

        // Set user flag if this is a user address space
        let flags = if self.is_user() {
            flags | PageTableFlags::User as u64
        } else {
            flags
        };

        self.page_table.lock().map_pages(vaddr, paddr, count, PageTableFlags::from_bits(flags))
    }

    /// Unmap pages from this address space
    pub fn unmap(&self, vaddr: VAddr, count: usize) -> Result {
        if !self.is_valid_vaddr(vaddr) {
            return Err(VmError::InvalidAddress);
        }

        self.page_table.lock().unmap_pages(vaddr, count)
    }

    /// Change protection for pages in this address space
    pub fn protect(&self, vaddr: VAddr, count: usize, prot: MemProt) -> Result {
        if !self.is_valid_vaddr(vaddr) {
            return Err(VmError::InvalidAddress);
        }

        let flags = PageTableFlags::from_prot(prot);

        // Set user flag if this is a user address space
        let flags = if self.is_user() {
            flags | PageTableFlags::User as u64
        } else {
            flags
        };

        self.page_table.lock().protect_pages(vaddr, count, PageTableFlags::from_bits(flags))
    }

    /// Resolve a virtual address to physical address
    pub fn resolve(&self, vaddr: VAddr) -> Option<PAddr> {
        if !self.is_valid_vaddr(vaddr) {
            return None;
        }

        self.page_table.lock().resolve(vaddr)
    }

    /// Flush TLB entries for this address space
    pub fn flush_tlb(&self) {
        self.page_table.lock().flush_tlb();
    }

    /// Flush TLB for a specific virtual address
    pub fn flush_tlb_va(&self, vaddr: VAddr) {
        self.page_table.lock().flush_tlb_va(vaddr);
    }

    /// Get the physical address of the root page table
    pub fn root_phys(&self) -> PAddr {
        self.page_table.lock().root_phys()
    }

    /// Increment reference count
    pub fn add_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference
    pub fn unref(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Relaxed) == 1
    }

    /// Get the current reference count
    pub fn ref_count(&self) -> u64 {
        self.ref_count.load(Ordering::Relaxed)
    }
}

// ============================================================================
// Address Space Context Switch
// ============================================================================

/// Switch to a different address space
///
/// This updates the active page table (CR3/TTBR/SATP) and performs
/// necessary TLB flushes.
///
/// # Safety
///
/// The caller must ensure the address space remains valid during use.
pub unsafe fn context_switch(from: Option<&AddressSpace>, to: Option<&AddressSpace>) {
    // Architecture-specific implementation
    #[cfg(target_arch = "aarch64")]
    {
        // TODO: Implement ARM64 context switch
    }

    #[cfg(target_arch = "x86_64")]
    {
        // x86_64 context switch - load CR3 with new page table
        if let Some(to_aspace) = to {
            let cr3_value = to_aspace.root_phys() as u64;
            unsafe {
                crate::kernel::arch::amd64::mmu::write_cr3(cr3_value);
            }
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        // TODO: Implement RISC-V context switch
    }
}

/// ============================================================================
// Initialization
// ============================================================================

/// Initialize the address space subsystem
pub fn init() {
    // ASID allocator is already initialized statically
    // Any additional initialization would go here
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_space_flags() {
        assert!(AddressSpaceFlags::Kernel.is_kernel());
        assert!(!AddressSpaceFlags::Kernel.is_user());

        assert!(AddressSpaceFlags::User.is_user());
        assert!(!AddressSpaceFlags::User.is_kernel());
    }

    #[test]
    fn test_vm_mapping() {
        let mapping = VmMapping::new(0x1000, 0x2000, 0x8000, MemProt::READ, MappingFlags::Anonymous);

        assert_eq!(mapping.base, 0x1000);
        assert_eq!(mapping.size, 0x2000);
        assert_eq!(mapping.end(), 0x3000);
        assert!(mapping.contains(0x1000));
        assert!(mapping.contains(0x2000));
        assert!(!mapping.contains(0x3000));
        assert!(!mapping.contains(0x0FFF));
    }

    #[test]
    fn test_mapping_overlaps() {
        let mapping1 = VmMapping::new(0x1000, 0x2000, 0, MemProt::READ, MappingFlags::None);
        let mapping2 = VmMapping::new(0x2000, 0x2000, 0, MemProt::READ, MappingFlags::None);
        let mapping3 = VmMapping::new(0x4000, 0x1000, 0, MemProt::READ, MappingFlags::None);

        // Adjacent mappings don't overlap
        assert!(!mapping1.overlaps(&mapping2));

        // Overlapping mapping
        let mapping4 = VmMapping::new(0x1500, 0x1000, 0, MemProt::READ, MappingFlags::None);
        assert!(mapping1.overlaps(&mapping4));

        // Non-overlapping
        assert!(!mapping1.overlaps(&mapping3));
    }
}
