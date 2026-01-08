// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Cross-Architecture Page Table Abstraction
//!
//! This module provides a unified interface for page table operations across
//! ARM64, AMD64, and RISC-V architectures, despite their different MMU designs.
//!
//! # Architecture Support
//!
//! | Architecture | Levels | VA Bits | Page Sizes |
//! //! |--------------|--------|---------|------------|
//! | ARM64 | 4 | 48 | 4KB, 16KB, 64KB |
//! | AMD64 | 4 | 48 | 4KB, 2MB, 1GB |
//! | RISC-V | 3 (Sv39) / 4 (Sv48) | 39 / 48 | 4KB, 2MB, 1GB |
//!
//! # Design
//!
//! The page table abstraction uses a traits-based approach where each architecture
//! implements the common `PageTableEntry` trait. The high-level `PageTable` type
//! provides architecture-agnostic operations.


use crate::kernel::vm::layout::*;
use crate::kernel::vm::{ArchPageTable, VmError, Result};
use core::fmt;

/// ============================================================================
/// Page Table Flags (Cross-Architecture)
/// ============================================================================

/// Page table entry flags that are consistent across architectures
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableFlags {
    /// Invalid / not present
    None = 0,

    /// Present / valid mapping
    Present = 1 << 0,

    /// Writable
    Write = 1 << 1,

    /// User-accessible (not kernel-only)
    User = 1 << 2,

    /// Write-through caching
    WriteThrough = 1 << 3,

    /// Cache disable
    CacheDisable = 1 << 4,

    /// Accessed flag (set by hardware)
    Accessed = 1 << 5,

    /// Dirty flag (set by hardware for writable mappings)
    Dirty = 1 << 6,

    /// Global mapping (not flushed on TLB shootdown)
    Global = 1 << 8,

    /// No-execute / execute-never
    NoExecute = 1 << 63,

    /// Device memory (uncacheable, device attributes)
    Device = 1 << 9,

    /// Normal memory (write-back cacheable)
    Normal = 1 << 10,
}

impl PageTableFlags {
    /// No flags set
    pub const NONE: u64 = 0;

    /// Present + writable (kernel-private, data)
    pub const KERNEL_DATA: u64 = (Self::Present as u64) | (Self::Write as u64);

    /// Present + read-only (kernel-private, code)
    pub const KERNEL_CODE: u64 = (Self::Present as u64);

    /// Present + user + writable (user data)
    pub const USER_DATA: u64 = (Self::Present as u64) | (Self::Write as u64) | (Self::User as u64);

    /// Present + user + read-only (user code)
    pub const USER_CODE: u64 = (Self::Present as u64) | (Self::User as u64);

    /// Create flags from raw bits
    pub const fn from_bits(bits: u64) -> Self {
        unsafe { core::mem::transmute(bits) }
    }

    /// Convert to raw bits
    pub const fn bits(self) -> u64 {
        self as u64
    }

    /// Check if present flag is set
    pub const fn is_present(self) -> bool {
        (self as u64 & Self::Present as u64) != 0
    }

    /// Check if writable flag is set
    pub const fn is_writable(self) -> bool {
        (self as u64 & Self::Write as u64) != 0
    }

    /// Check if user-accessible flag is set
    pub const fn is_user(self) -> bool {
        (self as u64 & Self::User as u64) != 0
    }

    /// Check if no-execute flag is set
    pub const fn is_no_execute(self) -> bool {
        (self as u64 & Self::NoExecute as u64) != 0
    }

    /// Check if this mapping allows execution
    pub const fn can_execute(self) -> bool {
        !self.is_no_execute()
    }

    /// Convert memory protection to page table flags
    pub const fn from_prot(prot: MemProt) -> u64 {
        let mut flags = Self::Present as u64;

        if prot.can_write() {
            flags |= Self::Write as u64;
        }

        // W^X: Execute flag is inverted (NoExecute)
        if !prot.can_execute() {
            flags |= Self::NoExecute as u64;
        }

        // For user mappings, set user bit
        // (kernel mappings don't have user bit set)
        // This will be handled by the caller based on address space type

        flags
    }

    /// Convert page table flags to memory protection
    pub const fn to_prot(self, is_user: bool) -> MemProt {
        let mut prot = MemProt::None;

        if self.is_present() {
            prot = MemProt::Read;
        }

        if self.is_writable() {
            prot = MemProt::Write; // Write implies read
        }

        if self.can_execute() {
            prot = MemProt::Read; // Combine with execute
            // Note: We can't represent R+X directly with MemProt enum
            // This would need a different representation
        }

        prot
    }

    /// Apply W^X policy to flags
    pub const fn enforce_wxorx(self) -> u64 {
        let bits = self as u64;
        let has_write = (bits & Self::Write as u64) != 0;
        let has_execute = (bits & Self::NoExecute as u64) == 0;

        if has_write && has_execute {
            // Clear execute if both write and execute are set
            bits | Self::NoExecute as u64
        } else {
            bits
        }
    }
}

/// ============================================================================
/// Page Table Entry Trait
/// ============================================================================

/// Trait for architecture-specific page table entry implementations
pub trait PageTableEntry: Sized + Copy {
    /// Create a new entry (invalid/not present)
    fn new() -> Self;

    /// Create an entry for a mapped page
    fn new_entry(paddr: PAddr, flags: PageTableFlags) -> Self;

    /// Get the physical address from this entry
    fn paddr(&self) -> PAddr;

    /// Get the flags from this entry
    fn flags(&self) -> PageTableFlags;

    /// Check if this entry is present/valid
    fn is_present(&self) -> bool;

    /// Check if this entry is a block/large page (not a table pointer)
    fn is_block(&self) -> bool;

    /// Update the flags for this entry
    fn set_flags(&mut self, flags: PageTableFlags);

    /// Convert to raw bits
    fn as_bits(&self) -> u64;

    /// Create from raw bits
    fn from_bits(bits: u64) -> Self;
}

/// ============================================================================
/// Page Table Types
/// ============================================================================

/// Mapping type (size of mapped pages)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingType {
    /// 4KB page
    Page4K = 0,

    /// 2MB page (large page)
    Page2M = 1,

    /// 1GB page (huge page)
    Page1G = 2,
}

impl MappingType {
    /// Get the size in bytes for this mapping type
    pub const fn size(&self) -> usize {
        match self {
            Self::Page4K => 4 * 1024,
            Self::Page2M => 2 * 1024 * 1024,
            Self::Page1G => 1024 * 1024 * 1024,
        }
    }

    /// Get the page shift for this mapping type
    pub const fn shift(&self) -> u8 {
        match self {
            Self::Page4K => 12,
            Self::Page2M => 21,
            Self::Page1G => 30,
        }
    }
}

/// ============================================================================
/// Cross-Architecture Page Table Interface
/// ============================================================================

/// Architecture-specific page table implementation selector
#[cfg(target_arch = "aarch64")]
pub type ArchPageTableImpl = crate::kernel::arch::arm64::mmu::ArmPageTable;

/// Architecture-specific page table implementation selector
#[cfg(target_arch = "x86_64")]
pub type ArchPageTableImpl = crate::kernel::arch::amd64::include::arch::aspace::X86PageTableMmu;

/// Architecture-specific page table implementation selector
#[cfg(target_arch = "riscv64")]
pub type ArchPageTableImpl = crate::kernel::arch::riscv64::mmu::RiscvPageTable;

/// High-level page table interface
///
/// This wraps the architecture-specific implementation and provides
/// a uniform API across all architectures.
pub struct PageTable {
    inner: ArchPageTableImpl,
    asid: Asid,
}

impl PageTable {
    /// Create a new page table
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: ArchPageTableImpl::new(),
            asid: ASID_INVALID,
        })
    }

    /// Create a new kernel page table
    pub fn new_kernel() -> Result<Self> {
        Ok(Self {
            inner: ArchPageTableImpl::new_kernel()?,
            asid: ASID_INVALID,
        })
    }

    /// Get the ASID for this page table
    pub fn asid(&self) -> Asid {
        self.asid
    }

    /// Set the ASID for this page table
    pub fn set_asid(&mut self, asid: Asid) {
        self.asid = asid;
    }

    /// Map a page with the specified flags
    pub fn map(&mut self, vaddr: VAddr, paddr: PAddr, flags: PageTableFlags) -> Result {
        // Validate alignment
        if !is_page_aligned(vaddr) || !is_page_aligned(paddr) {
            return Err(VmError::AlignmentError);
        }

        // Enforce W^X
        let flags_bits = flags.enforce_wxorx();

        self.inner.map(vaddr, paddr, flags_bits)
    }

    /// Map multiple pages
    pub fn map_pages(
        &mut self,
        vaddr: VAddr,
        paddr: PAddr,
        count: usize,
        flags: PageTableFlags,
    ) -> Result {
        // Validate alignment
        if !is_page_aligned(vaddr) || !is_page_aligned(paddr) {
            return Err(VmError::AlignmentError);
        }

        // Enforce W^X
        let flags_bits = flags.enforce_wxorx();

        for i in 0..count {
            let va = vaddr + (i * PAGE_SIZE);
            let pa = paddr + (i * PAGE_SIZE);
            self.inner.map(va, pa, flags_bits)?;
        }

        Ok(())
    }

    /// Unmap a page
    pub fn unmap(&mut self, vaddr: VAddr) -> Result {
        if !is_page_aligned(vaddr) {
            return Err(VmError::AlignmentError);
        }

        self.inner.unmap(vaddr)
    }

    /// Unmap multiple pages
    pub fn unmap_pages(&mut self, vaddr: VAddr, count: usize) -> Result {
        if !is_page_aligned(vaddr) {
            return Err(VmError::AlignmentError);
        }

        for i in 0..count {
            let va = vaddr + (i * PAGE_SIZE);
            self.inner.unmap(va)?;
        }

        Ok(())
    }

    /// Resolve a virtual address to physical address
    pub fn resolve(&self, vaddr: VAddr) -> Option<PAddr> {
        self.inner.resolve(vaddr)
    }

    /// Update protection flags for a mapping
    pub fn protect(&mut self, vaddr: VAddr, flags: PageTableFlags) -> Result {
        if !is_page_aligned(vaddr) {
            return Err(VmError::AlignmentError);
        }

        // Enforce W^X
        let flags_bits = flags.enforce_wxorx();

        self.inner.protect(vaddr, flags_bits)
    }

    /// Update protection flags for multiple pages
    pub fn protect_pages(
        &mut self,
        vaddr: VAddr,
        count: usize,
        flags: PageTableFlags,
    ) -> Result {
        if !is_page_aligned(vaddr) {
            return Err(VmError::AlignmentError);
        }

        // Enforce W^X
        let flags_bits = flags.enforce_wxorx();

        for i in 0..count {
            let va = vaddr + (i * PAGE_SIZE);
            self.inner.protect(va, flags_bits)?;
        }

        Ok(())
    }

    /// Flush TLB entries for this page table
    pub fn flush_tlb(&mut self) {
        self.inner.flush_tlb(None);
    }

    /// Flush TLB for a specific virtual address
    pub fn flush_tlb_va(&mut self, vaddr: VAddr) {
        self.inner.flush_tlb(Some(vaddr));
    }

    /// Get the physical address of the root page table
    pub fn root_phys(&self) -> PAddr {
        self.inner.root_phys()
    }
}

// ============================================================================
// Page Table Initialization
// ============================================================================

/// Initialize the page table subsystem
pub fn init() {
    // Architecture-specific initialization
    #[cfg(target_arch = "aarch64")]
    {
        // TODO: Implement ARM64 page table init
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::kernel::arch::amd64::mmu::x86_mmu_init();
    }

    #[cfg(target_arch = "riscv64")]
    {
        // TODO: Implement RISC-V page table init
    }
}

// ============================================================================
// Page Table Entry Implementations per Architecture
// ============================================================================

/// Generic 64-bit page table entry (works for all architectures)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GenericEntry(u64);

impl PageTableEntry for GenericEntry {
    fn new() -> Self {
        Self(0)
    }

    fn new_entry(paddr: PAddr, flags: PageTableFlags) -> Self {
        // Physical address should be page-aligned
        debug_assert!(is_page_aligned(paddr));

        // Combine address and flags
        // Note: The exact bit layout varies by architecture
        Self((paddr as u64) | flags.bits())
    }

    fn paddr(&self) -> PAddr {
        // Mask out the flags to get physical address
        // This is architecture-specific
        (self.0 & 0x0000_FFFF_FFFF_F000) as PAddr
    }

    fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits(self.0 & 0xFF0_0000_0000_0FFF)
    }

    fn is_present(&self) -> bool {
        (self.0 & 1) != 0
    }

    fn is_block(&self) -> bool {
        // Architecture-specific
        #[cfg(target_arch = "aarch64")]
        {
            // ARM64: bit 1 = 0 indicates block/page (not table)
            (self.0 & 0x2) == 0
        }

        #[cfg(target_arch = "x86_64")]
        {
            // x86-64: bit 7 = 1 indicates page (PS = huge page)
            (self.0 & 0x80) != 0
        }

        #[cfg(target_arch = "riscv64")]
        {
            // RISC-V: bits 1-4 indicate mapping type
            (self.0 & 0xE) != 0
        }
    }

    fn set_flags(&mut self, flags: PageTableFlags) {
        // Preserve address, update flags
        let paddr = self.paddr();
        self.0 = (paddr as u64) | flags.bits();
    }

    fn as_bits(&self) -> u64 {
        self.0
    }

    fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flags() {
        let flags = PageTableFlags::KERNEL_DATA;
        assert!(flags.is_present());
        assert!(flags.is_writable());
        assert!(!flags.is_user());

        let flags = PageTableFlags::from_bits(PageTableFlags::KERNEL_DATA);
        assert_eq!(flags.bits(), PageTableFlags::KERNEL_DATA);
    }

    #[test]
    fn test_wxorx() {
        // W + X should be rejected
        let bad_flags = PageTableFlags::Write | PageTableFlags::Present;
        let enforced = bad_flags.enforce_wxorx();
        // Should have NoExecute set
        assert!(enforced & PageTableFlags::NoExecute as u64 != 0);
    }

    #[test]
    fn test_mapping_type() {
        assert_eq!(MappingType::Page4K.size(), 4096);
        assert_eq!(MappingType::Page2M.size(), 2 * 1024 * 1024);
        assert_eq!(MappingType::Page1G.size(), 1024 * 1024 * 1024);
    }
}
