// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V Sv39/Sv48 Page Table Implementation
//!
//! This module provides a complete implementation of RISC-V Sv39 (48-bit virtual address)
//! and Sv48 (57-bit virtual address) page tables.
//!
//! # Design
//!
//! - **Sv39**: 3-level page table (512 GB address space)
//! - **Sv48**: 4-level page table (128 TB address space)
//! - **4KB pages**: Standard page size
//! - **Mega pages**: 2MB pages (optional, Sv39 only)
//! - **Giga pages**: 1GB pages (optional, Sv39 only)
//!
//! # Page Table Levels (Sv39)
//!
//! ```
//! Level 2 (PML4) → Level 1 (PDPT) → Level 0 (PT) → Page
//! 512 entries  → 512 entries   → 512 entries → 4KB
//! 9 bits      → 9 bits        → 9 bits     → 12 bits
//! = 39 bits virtual address (512 GB)
//! ```
//!
//! # Usage
//!
//! ```rust
//! let pt = PageTable::new_sv39()?;
//! pt.map_page(0x1000, 0x8000, Flags::READ | Flags::WRITE)?;
//! pt.unmap_page(0x1000)?;
//! ```

#![no_std]

use crate::kernel::pmm;
use crate::rustux::types::*;
use crate::kernel::sync::spin::SpinMutex;

/// ============================================================================
/// Constants
/// ============================================================================

/// Page size (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Page shift (log2 of page size)
pub const PAGE_SHIFT: usize = 12;

/// Number of entries per page table
pub const ENTRIES_PER_PAGE_TABLE: usize = 512;

/// Page table entry size (8 bytes)
pub const PAGE_TABLE_ENTRY_SIZE: usize = 8;

/// Sv39 virtual address bits
pub const SV39_VA_BITS: usize = 39;

/// Sv48 virtual address bits
pub const SV48_VA_BITS: usize = 48;

/// ============================================================================
/// Page Table Entry Flags
/// ============================================================================

/// Page table entry flags for RISC-V Sv39/Sv48
pub mod flags {
    /// Valid bit
    pub const VALID: u64 = 1 << 0;

    /// Readable bit
    pub const READ: u64 = 1 << 1;

    /// Writable bit
    pub const WRITE: u64 = 1 << 2;

    /// Executable bit
    pub const EXECUTE: u64 = 1 << 3;

    /// User-accessible bit
    pub const USER: u64 = 1 << 4;

    /// Global bit (ignored in Sv39/Sv48)
    pub const GLOBAL: u64 = 1 << 5;

    /// Accessed bit (hardware set)
    pub const ACCESSED: u64 = 1 << 6;

    /// Dirty bit (hardware set)
    pub const DIRTY: u64 = 1 << 7;

    /// Read/write permission (R + W)
    pub const RW: u64 = READ | WRITE;

    /// Read/execute permission (R + X)
    pub const RX: u64 = READ | EXECUTE;

    /// Read/write/execute permission
    pub const RWX: u64 = READ | WRITE | EXECUTE;

    /// User permissions
    pub const USER_READ: u64 = USER | READ;
    pub const USER_WRITE: u64 = USER | WRITE;
    pub const USER_EXECUTE: u64 = USER | EXECUTE;
    pub const USER_RW: u64 = USER | READ | WRITE;
    pub const USER_RX: u64 = USER | READ | EXECUTE;

    /// Kernel permissions
    pub const KERNEL_READ: u64 = READ;
    pub const KERNEL_WRITE: u64 = WRITE;
    pub const KERNEL_EXECUTE: u64 = EXECUTE;
    pub const KERNEL_RW: u64 = READ | WRITE;
    pub const KERNEL_RX: u64 = READ | EXECUTE;

    /// Combine user and kernel flags
    pub const PERM_USER: u64 = USER;
    pub const PERM_READ: u64 = READ;
    pub const PERM_WRITE: u64 = WRITE;
    pub const PERM_EXECUTE: u64 = EXECUTE;
}

/// ============================================================================
/// Page Table Entry
/// ============================================================================

/// RISC-V Sv39/Sv48 Page Table Entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry {
    /// Entry value
    pub entry: u64,
}

impl PageTableEntry {
    /// Create a new zero entry
    pub const fn new() -> Self {
        Self { entry: 0 }
    }

    /// Create an entry for a physical page
    pub const fn new_page(paddr: PAddr, flags: u64) -> Self {
        Self {
            entry: ((paddr >> 12) << 10) | flags | flags::VALID,
        }
    }

    /// Create an entry pointing to another page table
    pub const fn new_table(ppn: u64) -> Self {
        Self {
            entry: (ppn << 10) | flags::VALID,
        }
    }

    /// Check if entry is valid
    pub const fn is_valid(&self) -> bool {
        (self.entry & flags::VALID) != 0
    }

    /// Check if entry is a leaf (has R/W/X bits)
    pub const fn is_leaf(&self) -> bool {
        (self.entry & (flags::READ | flags::WRITE | flags::EXECUTE)) != 0
    }

    /// Check if entry is a table pointer
    pub const fn is_table(&self) -> bool {
        self.is_valid() && !self.is_leaf()
    }

    /// Get physical page number
    pub const fn ppn(&self) -> u64 {
        self.entry >> 10
    }

    /// Get physical address
    pub const fn paddr(&self) -> PAddr {
        (self.ppn() << 12) as PAddr
    }

    /// Get flags
    pub const fn flags(&self) -> u64 {
        self.entry & 0x3FF
    }

    /// Set flags
    pub fn set_flags(&mut self, flags: u64) {
        self.entry = (self.entry & !0x3FF) | (flags & 0x3FF);
    }

    /// Set accessed bit
    pub fn set_accessed(&mut self) {
        self.entry |= flags::ACCESSED;
    }

    /// Set dirty bit
    pub fn set_dirty(&mut self) {
        self.entry |= flags::DIRTY;
    }
}

/// ============================================================================
/// Page Table
/// ============================================================================

/// Page table level
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableLevel {
    /// Level 2 (root) - PML4 for Sv48
    L2 = 2,

    /// Level 1 - PDPT
    L1 = 1,

    /// Level 0 - PT
    L0 = 0,
}

impl PageTableLevel {
    /// Get shift for this level
    pub const fn shift(&self) -> usize {
        match self {
            Self::L2 => 30, // 1GB pages
            Self::L1 => 21, // 2MB pages
            Self::L0 => 12, // 4KB pages
        }
    }

    /// Get level number
    pub const fn level(&self) -> usize {
        *self as usize
    }
}

/// Page table
///
/// Represents a single level of the page table hierarchy.
pub struct PageTable {
    /// Physical address of this page table
    pub paddr: PAddr,

    /// Virtual address of this page table
    pub vaddr: VAddr,

    /// Entries in this page table
    pub entries: Mutex<[PageTableEntry; ENTRIES_PER_PAGE_TABLE]>,
}

impl PageTable {
    /// Allocate a new page table
    pub fn alloc() -> Result<Self> {
        // Allocate a physical page
        let paddr = pmm::alloc_page()?;
        let vaddr = pmm::paddr_to_vaddr(paddr);

        // Zero the page table
        unsafe {
            core::ptr::write_bytes(vaddr as *mut u8, 0, PAGE_SIZE);
        }

        Ok(Self {
            paddr,
            vaddr,
            entries: unsafe { Mutex::new(core::ptr::read_volatile(vaddr as *const _)) },
        })
    }

    /// Get entry at index
    pub fn get_entry(&self, index: usize) -> PageTableEntry {
        if index >= ENTRIES_PER_PAGE_TABLE {
            return PageTableEntry::new();
        }

        self.entries.lock()[index]
    }

    /// Set entry at index
    pub fn set_entry(&self, index: usize, entry: PageTableEntry) {
        if index >= ENTRIES_PER_PAGE_TABLE {
            return;
        }

        let mut entries = self.entries.lock();
        entries[index] = entry;

        // Write back to memory
        unsafe {
            let entry_ptr = (self.vaddr + index * core::mem::size_of::<PageTableEntry>())
                as *mut PageTableEntry;
            core::ptr::write_volatile(entry_ptr, entry);
        }
    }

    /// Clear entry at index
    pub fn clear_entry(&self, index: usize) {
        self.set_entry(index, PageTableEntry::new());
    }

    /// Get physical address of this page table
    pub const fn paddr(&self) -> PAddr {
        self.paddr
    }

    /// Get physical page number of this page table
    pub const fn ppn(&self) -> u64 {
        (self.paddr >> 12) as u64
    }

    /// Invalidate TLB for this page table
    pub fn invalidate_tlb(&self) {
        unsafe {
            core::arch::asm!("sfence.vma");
        }
    }
}

/// ============================================================================
/// Address Space
/// ============================================================================

/// Virtual address space mode
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpaceMode {
    /// Sv39: 39-bit virtual addresses (512 GB)
    Sv39 = 8,

    /// Sv48: 48-bit virtual addresses (128 TB)
    Sv48 = 9,
}

impl AddressSpaceMode {
    /// Get SATP mode value
    pub const fn satp_mode(&self) -> u64 {
        *self as u64
    }

    /// Get virtual address bits
    pub const fn va_bits(&self) -> usize {
        match self {
            Self::Sv39 => 39,
            Self::Sv48 => 48,
        }
    }

    /// Get number of page table levels
    pub const fn levels(&self) -> usize {
        match self {
            Self::Sv39 => 3,
            Self::Sv48 => 4,
        }
    }
}

/// Virtual address space
///
/// Manages the root page table for a process or the kernel.
pub struct AddressSpace {
    /// Address space mode
    pub mode: AddressSpaceMode,

    /// Root page table
    pub root: Option<PageTable>,

    /// ASID (address space ID)
    pub asid: u16,
}

impl AddressSpace {
    /// Create a new address space
    pub fn new(mode: AddressSpaceMode, asid: u16) -> Result<Self> {
        let root = PageTable::alloc()?;

        Ok(Self {
            mode,
            root: Some(root),
            asid,
        })
    }

    /// Get the SATP value for this address space
    pub fn satp(&self) -> u64 {
        if let Some(ref root) = self.root {
            let mode_bits = self.mode.satp_mode() << 60;
            let asid_bits = (self.asid as u64) << 32;
            let ppn_bits = root.ppn();

            mode_bits | asid_bits | ppn_bits
        } else {
            0
        }
    }

    /// Activate this address space
    pub fn activate(&self) {
        let satp = self.satp();

        unsafe {
            core::arch::asm!("csrw satp, {}", in(reg) satp);
            core::arch::asm!("sfence.vma"); // Flush TLB
        }
    }

    /// Map a page
    ///
    /// # Arguments
    ///
    /// * `vaddr` - Virtual address to map
    /// * `paddr` - Physical address to map
    /// * `flags` - Page table entry flags
    /// * `alloc_tables` - Whether to allocate intermediate tables
    pub fn map_page(&mut self, vaddr: VAddr, paddr: PAddr, flags: u64, alloc_tables: bool) -> Result<()> {
        // Validate alignment
        if vaddr & (PAGE_SIZE - 1) != 0 || paddr & (PAGE_SIZE - 1) != 0 {
            return Err(RX_ERR_INVALID_ARGS);
        }

        let root = self.root.as_mut().ok_or(RX_ERR_BAD_STATE)?;

        // Walk/create page tables
        match self.mode {
            AddressSpaceMode::Sv39 => self.map_page_sv39(root, vaddr, paddr, flags, alloc_tables),
            AddressSpaceMode::Sv48 => self.map_page_sv48(root, vaddr, paddr, flags, alloc_tables),
        }
    }

    /// Map a page in Sv39 mode
    fn map_page_sv39(
        &mut self,
        root: &mut PageTable,
        vaddr: VAddr,
        paddr: PAddr,
        flags: u64,
        alloc_tables: bool,
    ) -> Result<()> {
        // Extract page table indices from virtual address
        // Sv39: [39:30] L2, [29:21] L1, [20:12] L0, [11:0] offset
        let l2_idx = (vaddr >> 30) & 0x1FF;
        let l1_idx = (vaddr >> 21) & 0x1FF;
        let l0_idx = (vaddr >> 12) & 0x1FF;

        // Get or create L1 table
        let l1_entry = root.get_entry(l2_idx);
        let l1_vaddr = if l1_entry.is_valid() {
            l1_entry.paddr()
        } else {
            if !alloc_tables {
                return Err(RX_ERR_NOT_FOUND);
            }

            // Allocate new L1 table
            let l1_table = PageTable::alloc()?;
            root.set_entry(l2_idx, PageTableEntry::new_table(l1_table.ppn()));
            l1_table.vaddr
        };

        // Get or create L0 table
        let l1_table = unsafe { &*(l1_vaddr as *const PageTable) };
        let l0_entry = unsafe { &*(l1_vaddr as *const PageTable) }.get_entry(l1_idx);
        let l0_vaddr = if l0_entry.is_valid() {
            l0_entry.paddr()
        } else {
            if !alloc_tables {
                return Err(RX_ERR_NOT_FOUND);
            }

            // Allocate new L0 table
            let l0_table = PageTable::alloc()?;
            unsafe { &*(l1_vaddr as *const PageTable) }.set_entry(l1_idx, PageTableEntry::new_table(l0_table.ppn()));
            l0_table.vaddr
        };

        // Set page table entry
        let l0_table = unsafe { &*(l0_vaddr as *const PageTable) };
        l0_table.set_entry(l0_idx, PageTableEntry::new_page(paddr, flags));

        // Flush TLB
        root.invalidate_tlb();

        Ok(())
    }

    /// Unmap a page
    pub fn unmap_page(&mut self, vaddr: VAddr) -> Result<()> {
        // Validate alignment
        if vaddr & (PAGE_SIZE - 1) != 0 {
            return Err(RX_ERR_INVALID_ARGS);
        }

        let root = self.root.as_mut().ok_or(RX_ERR_BAD_STATE)?;

        match self.mode {
            AddressSpaceMode::Sv39 => self.unmap_page_sv39(root, vaddr),
            AddressSpaceMode::Sv48 => self.unmap_page_sv48(root, vaddr),
        }
    }

    /// Unmap a page in Sv39 mode
    fn unmap_page_sv39(&mut self, root: &mut PageTable, vaddr: VAddr) -> Result<()> {
        // Extract page table indices
        let l2_idx = (vaddr >> 30) & 0x1FF;
        let l1_idx = (vaddr >> 21) & 0x1FF;
        let l0_idx = (vaddr >> 12) & 0x1FF;

        // Get L1 table
        let l1_entry = root.get_entry(l2_idx);
        if !l1_entry.is_valid() {
            return Ok(()); // Already unmapped
        }

        let l1_vaddr = l1_entry.paddr();

        // Get L0 table
        let l0_entry = unsafe { &*(l1_vaddr as *const PageTable) }.get_entry(l1_idx);
        if !l0_entry.is_valid() {
            return Ok(()); // Already unmapped
        }

        let l0_vaddr = l0_entry.paddr();

        // Clear entry
        unsafe { &*(l0_vaddr as *mut PageTable) }.clear_entry(l0_idx);

        // Flush TLB
        root.invalidate_tlb();

        Ok(())
    }

    /// Translate virtual address to physical address
    pub fn translate(&self, vaddr: VAddr) -> Option<PAddr> {
        let root = self.root.as_ref()?;

        match self.mode {
            AddressSpaceMode::Sv39 => self.translate_sv39(root, vaddr),
            AddressSpaceMode::Sv48 => self.translate_sv48(root, vaddr),
        }
    }

    /// Translate virtual address in Sv39 mode
    fn translate_sv39(&self, root: &PageTable, vaddr: VAddr) -> Option<PAddr> {
        // Extract page table indices
        let l2_idx = (vaddr >> 30) & 0x1FF;
        let l1_idx = (vaddr >> 21) & 0x1FF;
        let l0_idx = (vaddr >> 12) & 0x1FF;

        // Walk page tables
        let l1_entry = root.get_entry(l2_idx);
        if !l1_entry.is_valid() {
            return None;
        }

        let l1_vaddr = l1_entry.paddr();
        let l0_entry = unsafe { &*(l1_vaddr as *const PageTable) }.get_entry(l1_idx);
        if !l0_entry.is_valid() {
            return None;
        }

        let l0_vaddr = l0_entry.paddr();
        let final_entry = unsafe { &*(l0_vaddr as *const PageTable) }.get_entry(l0_idx);
        if !final_entry.is_valid() || !final_entry.is_leaf() {
            return None;
        }

        // Calculate physical address
        let page_paddr = final_entry.paddr();
        let offset = vaddr & (PAGE_SIZE - 1);

        Some(page_paddr + offset)
    }

    // ============================================================================
    // Sv48 Implementation
    // ============================================================================

    /// Map a page in Sv48 mode
    ///
    /// Sv48 has 4 levels: [47:39] L3, [38:30] L2, [29:21] L1, [20:12] L0, [11:0] offset
    fn map_page_sv48(
        &mut self,
        root: &mut PageTable,
        vaddr: VAddr,
        paddr: PAddr,
        flags: u64,
        alloc_tables: bool,
    ) -> Result<()> {
        // Extract page table indices from virtual address
        // Sv48: [47:39] L3, [38:30] L2, [29:21] L1, [20:12] L0, [11:0] offset
        let l3_idx = (vaddr >> 39) & 0x1FF;
        let l2_idx = (vaddr >> 30) & 0x1FF;
        let l1_idx = (vaddr >> 21) & 0x1FF;
        let l0_idx = (vaddr >> 12) & 0x1FF;

        // Get or create L2 table
        let l2_entry = root.get_entry(l3_idx);
        let l2_vaddr = if l2_entry.is_valid() {
            l2_entry.paddr()
        } else {
            if !alloc_tables {
                return Err(RX_ERR_NOT_FOUND);
            }

            // Allocate new L2 table
            let l2_table = PageTable::alloc()?;
            root.set_entry(l3_idx, PageTableEntry::new_table(l2_table.ppn()));
            l2_table.vaddr
        };

        // Get or create L1 table
        let l2_table = unsafe { &*(l2_vaddr as *const PageTable) };
        let l1_entry = l2_table.get_entry(l2_idx);
        let l1_vaddr = if l1_entry.is_valid() {
            l1_entry.paddr()
        } else {
            if !alloc_tables {
                return Err(RX_ERR_NOT_FOUND);
            }

            // Allocate new L1 table
            let l1_table = PageTable::alloc()?;
            unsafe { &*(l2_vaddr as *const PageTable) }.set_entry(l2_idx, PageTableEntry::new_table(l1_table.ppn()));
            l1_table.vaddr
        };

        // Get or create L0 table
        let l1_table = unsafe { &*(l1_vaddr as *const PageTable) };
        let l0_entry = l1_table.get_entry(l1_idx);
        let l0_vaddr = if l0_entry.is_valid() {
            l0_entry.paddr()
        } else {
            if !alloc_tables {
                return Err(RX_ERR_NOT_FOUND);
            }

            // Allocate new L0 table
            let l0_table = PageTable::alloc()?;
            unsafe { &*(l1_vaddr as *const PageTable) }.set_entry(l1_idx, PageTableEntry::new_table(l0_table.ppn()));
            l0_table.vaddr
        };

        // Set page table entry
        let l0_table = unsafe { &*(l0_vaddr as *const PageTable) };
        l0_table.set_entry(l0_idx, PageTableEntry::new_page(paddr, flags));

        // Flush TLB
        root.invalidate_tlb();

        Ok(())
    }

    /// Unmap a page in Sv48 mode
    fn unmap_page_sv48(&mut self, root: &mut PageTable, vaddr: VAddr) -> Result<()> {
        // Extract page table indices
        let l3_idx = (vaddr >> 39) & 0x1FF;
        let l2_idx = (vaddr >> 30) & 0x1FF;
        let l1_idx = (vaddr >> 21) & 0x1FF;
        let l0_idx = (vaddr >> 12) & 0x1FF;

        // Get L2 table
        let l2_entry = root.get_entry(l3_idx);
        if !l2_entry.is_valid() {
            return Ok(()); // Already unmapped
        }

        let l2_vaddr = l2_entry.paddr();

        // Get L1 table
        let l1_entry = unsafe { &*(l2_vaddr as *const PageTable) }.get_entry(l2_idx);
        if !l1_entry.is_valid() {
            return Ok(()); // Already unmapped
        }

        let l1_vaddr = l1_entry.paddr();

        // Get L0 table
        let l0_entry = unsafe { &*(l1_vaddr as *const PageTable) }.get_entry(l1_idx);
        if !l0_entry.is_valid() {
            return Ok(()); // Already unmapped
        }

        let l0_vaddr = l0_entry.paddr();

        // Clear entry
        unsafe { &*(l0_vaddr as *mut PageTable) }.clear_entry(l0_idx);

        // Flush TLB
        root.invalidate_tlb();

        Ok(())
    }

    /// Translate virtual address in Sv48 mode
    fn translate_sv48(&self, root: &PageTable, vaddr: VAddr) -> Option<PAddr> {
        // Extract page table indices
        let l3_idx = (vaddr >> 39) & 0x1FF;
        let l2_idx = (vaddr >> 30) & 0x1FF;
        let l1_idx = (vaddr >> 21) & 0x1FF;
        let l0_idx = (vaddr >> 12) & 0x1FF;

        // Walk page tables (4 levels for Sv48)
        let l2_entry = root.get_entry(l3_idx);
        if !l2_entry.is_valid() {
            return None;
        }

        let l2_vaddr = l2_entry.paddr();
        let l1_entry = unsafe { &*(l2_vaddr as *const PageTable) }.get_entry(l2_idx);
        if !l1_entry.is_valid() {
            return None;
        }

        let l1_vaddr = l1_entry.paddr();
        let l0_entry = unsafe { &*(l1_vaddr as *const PageTable) }.get_entry(l1_idx);
        if !l0_entry.is_valid() {
            return None;
        }

        let l0_vaddr = l0_entry.paddr();
        let final_entry = unsafe { &*(l0_vaddr as *const PageTable) }.get_entry(l0_idx);
        if !final_entry.is_valid() || !final_entry.is_leaf() {
            return None;
        }

        // Calculate physical address
        let page_paddr = final_entry.paddr();
        let offset = vaddr & (PAGE_SIZE - 1);

        Some(page_paddr + offset)
    }
}

/// ============================================================================
/// Kernel Address Space
/// ============================================================================

/// Global kernel address space
static mut KERNEL_AS: Option<AddressSpace> = None;

/// Initialize kernel address space
pub fn init_kernel_as() {
    unsafe {
        KERNEL_AS = Some(AddressSpace::new(AddressSpaceMode::Sv39, 0).unwrap());
    }

    log_info!("Kernel address space initialized");
}

/// Get kernel address space
pub fn kernel_as() -> &'static mut AddressSpace {
    unsafe {
        KERNEL_AS.as_mut().expect("Kernel address space not initialized")
    }
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pte_new() {
        let pte = PageTableEntry::new();
        assert!(!pte.is_valid());
        assert!(!pte.is_leaf());
    }

    #[test]
    fn test_pte_page() {
        let pte = PageTableEntry::new_page(0x8000, flags::READ | flags::WRITE);
        assert!(pte.is_valid());
        assert!(pte.is_leaf());
        assert_eq!(pte.paddr(), 0x8000);
    }

    #[test]
    fn test_pte_table() {
        let pte = PageTableEntry::new_table(0x1000);
        assert!(pte.is_valid());
        assert!(pte.is_table());
        assert_eq!(pte.ppn(), 0x1000);
    }

    #[test]
    fn test_flags() {
        assert_eq!(flags::VALID, 1);
        assert_eq!(flags::READ, 2);
        assert_eq!(flags::WRITE, 4);
        assert_eq!(flags::EXECUTE, 8);
        assert_eq!(flags::USER, 16);
    }

    #[test]
    fn test_address_space_mode() {
        let mode = AddressSpaceMode::Sv39;
        assert_eq!(mode.va_bits(), 39);
        assert_eq!(mode.levels(), 3);
        assert_eq!(mode.satp_mode(), 8);
    }

    #[test]
    fn test_extract_indices() {
        let vaddr: VAddr = 0x1234_5678; // Some address
        let l2_idx = (vaddr >> 30) & 0x1FF;
        let l1_idx = (vaddr >> 21) & 0x1FF;
        let l0_idx = (vaddr >> 12) & 0x1FF;

        // Verify indices are in valid range
        assert!(l2_idx < 512);
        assert!(l1_idx < 512);
        assert!(l0_idx < 512);
    }
}
