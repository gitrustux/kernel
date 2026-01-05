// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Page Table Management
//!
//! This module provides page table structures for x86-64.

#![no_std]

use crate::rustux::types::*;

/// Page table entry type
pub type pt_entry_t = u64;

/// Different page table levels in the page table management hierarchy
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageTableLevel {
    /// Page Table level (4K pages)
    PT_L = 0,
    /// Page Directory level (2M pages)
    PD_L = 1,
    /// Page Directory Pointer Table level (1G pages)
    PDP_L = 2,
    /// Page Map Level 4 (top level)
    PML4_L = 3,
}

/// Page table role for unified address spaces
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableRole {
    /// Independent page table
    Independent = 0,
    /// Restricted page table (part of unified aspace)
    Restricted = 1,
    /// Shared page table (part of unified aspace)
    Shared = 2,
    /// Unified page table (combines restricted + shared)
    Unified = 3,
}

/// Type for flags used in the hardware page tables, for terminal entries.
pub type PtFlags = u64;

/// Type for flags used in the hardware page tables, for non-terminal entries.
pub type IntermediatePtFlags = u64;

/// Structure for tracking an upcoming TLB invalidation
#[repr(C)]
#[derive(Debug)]
pub struct PendingTlbInvalidation {
    /// If true, ignore |vaddr| and perform a full invalidation for this context.
    pub full_shootdown: bool,
    /// If true, at least one enqueued entry was for a global page.
    pub contains_global: bool,
    /// Number of valid elements in |items|
    pub count: u32,
    /// Reserved padding
    _pad: u32,
    /// List of addresses queued for invalidation.
    pub items: [PendingTlbItem; 32],
}

/// Item in a pending TLB invalidation
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PendingTlbItem {
    /// Raw encoded value
    pub raw: u64,
}

impl PendingTlbItem {
    /// Create a new pending TLB item
    pub fn new(vaddr: VAddr, level: PageTableLevel, is_global: bool, is_terminal: bool) -> Self {
        const PAGE_SHIFT: u64 = 12;

        let mut raw = 0u64;
        // Set page level (bits 2:0)
        raw |= (level as u64) & 0x7;
        // Set is_global bit (bit 3)
        if is_global {
            raw |= 1 << 3;
        }
        // Set is_terminal bit (bit 4)
        if is_terminal {
            raw |= 1 << 4;
        }
        // Set encoded address (bits 63:12)
        raw |= (vaddr as u64 >> PAGE_SHIFT) << 12;

        Self { raw }
    }

    /// Get the virtual address from this item
    pub fn addr(&self) -> VAddr {
        const PAGE_SHIFT: u64 = 12;
        ((self.raw >> 12) << PAGE_SHIFT) as VAddr
    }
}

impl PendingTlbInvalidation {
    /// Create a new empty pending TLB invalidation
    pub const fn new() -> Self {
        Self {
            full_shootdown: false,
            contains_global: false,
            count: 0,
            _pad: 0,
            items: [PendingTlbItem { raw: 0 }; 32],
        }
    }

    /// Add address |v|, translated at depth |level|, to the set of addresses to be invalidated.
    /// |is_terminal| should be true iff this invalidation is targeting the final step of the
    /// translation rather than a higher page table entry. |is_global_page| should be true iff this
    /// page was mapped with the global bit set.
    pub fn enqueue(&mut self, vaddr: VAddr, level: PageTableLevel, is_global_page: bool, is_terminal: bool) {
        if is_global_page {
            self.contains_global = true;
        }

        // We mark PML4_L entries as full shootdowns, since it's going to be expensive one way or another.
        if self.count as usize >= self.items.len() || level == PageTableLevel::PML4_L {
            self.full_shootdown = true;
            return;
        }

        self.items[self.count as usize] = PendingTlbItem::new(vaddr, level, is_global_page, is_terminal);
        self.count += 1;
    }

    /// Clear the list of pending invalidations
    pub fn clear(&mut self) {
        self.count = 0;
        self.full_shootdown = false;
        self.contains_global = false;
    }
}

/// Base class for x86 page tables
///
/// This provides the common interface for page table operations.
pub struct X86PageTableBase {
    /// Physical address of the page table
    pub phys: PAddr,
    /// Virtual address of the page table
    pub virt: *mut pt_entry_t,
    /// Number of pages allocated for this page table
    pub pages: usize,
    /// Context pointer (for TLB invalidation)
    pub ctx: *mut u8,
    /// Role of this page table (for unified address spaces)
    pub role: PageTableRole,
    /// Number of references to this page table (for unified address spaces)
    pub num_references: u32,
}

impl X86PageTableBase {
    /// Create a new empty page table base
    pub const fn new() -> Self {
        Self {
            phys: 0,
            virt: core::ptr::null_mut(),
            pages: 0,
            ctx: core::ptr::null_mut(),
            role: PageTableRole::Independent,
            num_references: 0,
        }
    }

    /// Get the physical address of this page table
    pub fn phys(&self) -> PAddr {
        self.phys
    }

    /// Get the virtual address of this page table
    pub fn virt(&self) -> *mut pt_entry_t {
        self.virt
    }

    /// Get the number of pages allocated for this page table
    pub fn pages(&self) -> usize {
        self.pages
    }

    /// Get the context pointer
    pub fn ctx(&self) -> *mut u8 {
        self.ctx
    }

    /// Check if this page table is restricted
    pub fn is_restricted(&self) -> bool {
        self.role == PageTableRole::Restricted
    }

    /// Check if this page table is shared
    pub fn is_shared(&self) -> bool {
        self.role == PageTableRole::Shared
    }

    /// Check if this page table is unified
    pub fn is_unified(&self) -> bool {
        self.role == PageTableRole::Unified
    }

    /// Get the lock order for this page table
    /// Returns 1 for unified page tables and 0 for all other page tables.
    pub fn lock_order(&self) -> u32 {
        if self.is_unified() { 1 } else { 0 }
    }

    /// Initialize the page table with a context pointer
    ///
    /// # Arguments
    ///
    /// * `ctx` - Context pointer for TLB invalidation
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    pub fn init(&mut self, ctx: *mut core::ffi::c_void) -> crate::rustux::types::RxStatus {
        self.ctx = ctx as *mut u8;
        0 // OK
    }

    /// Destroy the page table
    ///
    /// # Returns
    ///
    /// Status code indicating success or failure
    pub fn destroy(&mut self) -> crate::rustux::types::RxStatus {
        // TODO: Implement page table cleanup
        self.virt = core::ptr::null_mut();
        self.phys = 0;
        self.pages = 0;
        0 // OK
    }
}

pub mod page_tables {
    use super::*;

    /// Page table entry
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct PageTableEntry {
        pub value: u64,
    }

    impl PageTableEntry {
        pub const fn new() -> Self {
            Self { value: 0 }
        }

        /// Create a new page table entry from a physical address and flags
        pub const fn from_parts(paddr: PAddr, flags: PtFlags) -> Self {
            Self {
                value: (paddr as u64) | flags,
            }
        }

        /// Get the physical address from this entry
        pub fn paddr(&self) -> PAddr {
            (self.value & 0x000F_FFFF_FFFF_F000) as PAddr
        }

        /// Get the flags from this entry
        pub fn flags(&self) -> PtFlags {
            self.value & 0xFFF_F000_0000_0FFF
        }

        /// Check if this entry is present
        pub fn is_present(&self) -> bool {
            (self.value & 1) != 0
        }

        /// Check if this entry is writable
        pub fn is_writable(&self) -> bool {
            (self.value & 2) != 0
        }

        /// Check if this entry is user-accessible
        pub fn is_user(&self) -> bool {
            (self.value & 4) != 0
        }

        /// Check if this is a large page
        pub fn is_large_page(&self) -> bool {
            (self.value & 0x80) != 0
        }

        /// Check if this entry has the no-execute flag set
        pub fn is_no_execute(&self) -> bool {
            (self.value & (1 << 63)) != 0
        }
    }
}

/// x86 MMU flags
pub mod mmu_flags {
    use super::*;

    /// Present flag
    pub const X86_MMU_PG_P: PtFlags = 1 << 0;
    /// Writable flag
    pub const X86_MMU_PG_W: PtFlags = 1 << 1;
    /// User flag
    pub const X86_MMU_PG_U: PtFlags = 1 << 2;
    /// Write-through flag
    pub const X86_MMU_PG_PWT: PtFlags = 1 << 3;
    /// Cache disable flag
    pub const X86_MMU_PG_PCD: PtFlags = 1 << 4;
    /// Accessed flag
    pub const X86_MMU_PG_A: PtFlags = 1 << 5;
    /// Dirty flag
    pub const X86_MMU_PG_D: PtFlags = 1 << 6;
    /// Page size flag (large pages)
    pub const X86_MMU_PG_PS: PtFlags = 1 << 7;
    /// Global flag
    pub const X86_MMU_PG_G: PtFlags = 1 << 8;
    /// No-execute flag
    pub const X86_MMU_PG_NX: PtFlags = 1 << 63;

    /// Frame mask (physical address bits)
    pub const X86_PG_FRAME: PtFlags = 0x000F_FFFF_FFFF_F000;
    /// Large page frame mask (for PDP_L 1GB pages)
    pub const X86_HUGE_PAGE_FRAME: PtFlags = 0x000FFF_FFFF_FC000;
    /// Large page frame mask (for PD_L 2MB pages)
    pub const X86_LARGE_PAGE_FRAME: PtFlags = 0x000FFFE0_0000_0FFF;

    /// Check if a page table entry is present
    #[inline]
    pub const fn is_page_present(entry: pt_entry_t) -> bool {
        (entry & X86_MMU_PG_P) != 0
    }

    /// Check if a page table entry is a large page
    #[inline]
    pub const fn is_large_page(entry: pt_entry_t) -> bool {
        (entry & X86_MMU_PG_PS) != 0
    }

    /// MMU flags for device memory (uncached, present, writable)
    pub const MMU_FLAGS_PERM_DEVICE: PtFlags = X86_MMU_PG_P | X86_MMU_PG_W | X86_MMU_PG_PWT | X86_MMU_PG_PCD;
    /// MMU flags for uncached memory
    pub const MMU_FLAGS_UNCACHED: PtFlags = X86_MMU_PG_PCD;

    /// Architecture MMU flags - Read permission
    pub const ARCH_MMU_FLAG_PERM_READ: u32 = 1 << 0;
    /// Architecture MMU flags - Write permission
    pub const ARCH_MMU_FLAG_PERM_WRITE: u32 = 1 << 1;
    /// Architecture MMU flags - Execute permission
    pub const ARCH_MMU_FLAG_PERM_EXECUTE: u32 = 1 << 2;

    /// Address space flags - Guest (for EPT)
    pub const ARCH_ASPACE_FLAG_GUEST: u32 = 1u32 << 31;
}

/// Page table constants
pub mod constants {
    /// Number of entries per page table
    pub const NO_OF_PT_ENTRIES: usize = 512;

    /// Page shift (4KB = 2^12)
    pub const PT_SHIFT: u64 = 12;
    /// Page directory shift (2MB = 2^21)
    pub const PD_SHIFT: u64 = 21;
    /// Page directory pointer shift (1GB = 2^30)
    pub const PDP_SHIFT: u64 = 30;
    /// PML4 shift (512GB = 2^39)
    pub const PML4_SHIFT: u64 = 39;

    /// Page size
    pub const PAGE_SIZE: usize = 1 << PT_SHIFT;
    /// Large page size (2MB)
    pub const LARGE_PAGE_SIZE: usize = 1 << PD_SHIFT;
    /// Huge page size (1GB)
    pub const HUGE_PAGE_SIZE: usize = 1 << PDP_SHIFT;
}
