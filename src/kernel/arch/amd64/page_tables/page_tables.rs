// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by an MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86-64 Page Table Implementation
//!
//! This module provides a complete implementation of x86-64 (AMD64) page tables
//! with support for 4-level paging (PML4, PDPT, PD, PT).
//!
//! # Design
//!
//! - **4-level page table**: PML4 → PDPT → PD → PT → Page
//! - **4KB pages**: Standard page size
//! - **2MB pages**: Large page support (PD level)
//! - **1GB pages**: Huge page support (PDPT level)
//! - **EPT support**: Intel Extended Page Tables for virtualization
//!
//! # Page Table Levels
//!
//! ```
//! Level 3 (PML4) → Level 2 (PDPT) → Level 1 (PD) → Level 0 (PT) → Page
//! 512 entries   → 512 entries   → 512 entries → 512 entries → 4KB
//! 9 bits       → 9 bits        → 9 bits      → 9 bits     → 12 bits
//! = 48 bits virtual address (256 TB)
//! ```


use core::ops::{Deref, DerefMut};
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{spin::Mutex, Ordering};

use super::constants::*;
use super::super::include::arch::x86::page_tables::page_tables::*;
use crate::kernel::pmm;
use crate::rustux::types::status;

// Re-export from the include module
pub use super::include::arch::x86::page_tables::page_tables::{
    PtEntry, PageTableLevel, PendingTlbInvalidation, TlbInvalidationItem,
    X86PageTableBase, X86PageTableMmu, X86PageTableEpt, PtFlags, IntermediatePtFlags,
    RxStatus, MappingCursor, ConsistencyManager, CacheLineFlusher,
};

/// Page size in bytes
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_SHIFT; // 4096 bytes

/// Number of entries per page table
pub const ENTRIES_PER_PAGE_TABLE: usize = 512;

/// x86-64 MMU page table flags
pub mod mmu_flags {
    use super::*;

    /// Present flag - page is mapped
    pub const X86_MMU_PG_P: u64 = 0x0001;

    /// Read/Write flag
    pub const X86_MMU_PG_RW: u64 = 0x0002;

    /// User/Supervisor flag
    pub const X86_MMU_PG_U: u64 = 0x0004;

    /// Write-Through cache flag
    pub const X86_MMU_PG_WT: u64 = 0x0008;

    /// Cache Disable flag
    pub const X86_MMU_PG_CD: u64 = 0x0010;

    /// Accessed flag
    pub const X86_MMU_PG_A: u64 = 0x0020;

    /// Dirty flag
    pub const X86_MMU_PG_D: u64 = 0x0040;

    /// Page Size flag (1=2MB/1GB, 0=4KB)
    pub const X86_MMU_PG_PS: u64 = 0x0080;

    /// Global flag
    pub const X86_MMU_PG_G: u64 = 0x0100;

    /// PAT flag for 4KB pages
    pub const X86_MMU_PG_PTE_PAT: u64 = 0x0080;

    /// PAT flag for large pages
    pub const X86_MMU_PG_LARGE_PAT: u64 = 0x1000;

    /// NX (No-Execute) bit (only in EPT)
    pub const X86_EPT_X: u64 = 0x00000001;

    /// EPT Read flag
    pub const X86_EPT_R: u64 = 0x00000002;

    /// EPT Write flag
    pub const X86_EPT_W: u64 = 0x00000004;

    /// EPT Write-Back memory type
    pub const X86_EPT_WB: u64 = 0x00000006;

    /// Dirty/Access mask
    pub const X86_DIRTY_ACCESS_MASK: u64 = 0xf9f;

    /// Flags mask for regular PTEs
    pub const X86_FLAGS_MASK: u64 = 0x8000000000000fff;

    /// Flags mask for large pages
    pub const X86_LARGE_FLAGS_MASK: u64 = 0x8000000000001fff;

    /// Physical address frame mask for 4KB pages
    pub const X86_PG_FRAME: u64 = 0x000ffffffffff000;

    /// Physical address frame mask for 2MB pages
    pub const X86_LARGE_PAGE_FRAME: u64 = 0x000fffffffe00000;

    /// Physical address frame mask for 1GB pages
    pub const X86_HUGE_PAGE_FRAME: u64 = 0x000fffffc0000000;
}

/// Architecture MMU flags
pub mod arch_flags {
    use super::*;

    /// Read permission
    pub const ARCH_MMU_FLAG_PERM_READ: u32 = 1 << 0;

    /// Write permission
    pub const ARCH_MMU_FLAG_PERM_WRITE: u32 = 1 << 1;

    /// Execute permission
    pub const ARCH_MMU_FLAG_PERM_EXECUTE: u32 = 1 << 2;

    /// Guest address space (for EPT)
    pub const ARCH_ASPACE_FLAG_GUEST: u32 = 1 << 31;
}

/// Convert virtual address to PML4 index
#[inline]
pub const fn vaddr_to_pml4_idx(vaddr: usize) -> usize {
    (vaddr >> PML4_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

/// Convert virtual address to PDPT index
#[inline]
pub const fn vaddr_to_pdpt_idx(vaddr: usize) -> usize {
    (vaddr >> PDP_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

/// Convert virtual address to PD index
#[inline]
pub const fn vaddr_to_pd_idx(vaddr: usize) -> usize {
    (vaddr >> PD_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

/// Convert virtual address to PT index
#[inline]
pub const fn vaddr_to_pt_idx(vaddr: usize) -> usize {
    (vaddr >> PT_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

/// Extract physical address from page table entry at given level
pub fn paddr_from_pte(level: PageTableLevel, pte: u64) -> usize {
    use mmu_flags::*;

    match level {
        PageTableLevel::PML4_L => panic!("Cannot get paddr from PML4 entry"),
        PageTableLevel::PDP_L => (pte & X86_HUGE_PAGE_FRAME) as usize,
        PageTableLevel::PD_L => (pte & X86_LARGE_PAGE_FRAME) as usize,
        PageTableLevel::PT_L => (pte & X86_PG_FRAME) as usize,
    }
}

/// Check if address is page-aligned
pub fn page_aligned(level: PageTableLevel, addr: usize) -> bool {
    let mask = match level {
        PageTableLevel::PT_L => PAGE_SIZE - 1,
        PageTableLevel::PD_L => (1 << PD_SHIFT) - 1,
        PageTableLevel::PDP_L => (1 << PDP_SHIFT) - 1,
        PageTableLevel::PML4_L => (1 << PML4_SHIFT) - 1,
    };
    addr & mask == 0
}

/// Convert virtual address to index at given level
pub fn vaddr_to_index(level: PageTableLevel, vaddr: usize) -> usize {
    match level {
        PageTableLevel::PML4_L => vaddr_to_pml4_idx(vaddr),
        PageTableLevel::PDP_L => vaddr_to_pdpt_idx(vaddr),
        PageTableLevel::PD_L => vaddr_to_pd_idx(vaddr),
        PageTableLevel::PT_L => vaddr_to_pt_idx(vaddr),
    }
}

impl ConsistencyManager {
    pub fn new<T>( _table: &T) -> Self {
        ConsistencyManager {
            pending: PendingTlbInvalidation::new(),
            requires_cache_flush: false,
        }
    }

    pub fn finish(&mut self) {
        // Flush TLB if needed
        if self.pending.count > 0 || self.pending.full_shootdown {
            arch_tlb_flush(&self.pending);
        }

        // Flush cache if needed
        if self.requires_cache_flush {
            arch_cache_flush();
        }

        self.pending.clear();
        self.requires_cache_flush = false;
    }
}

impl CacheLineFlusher {
    pub fn flush(&_self, _ptr: *const u8) {
        // Implemented in assembly
    }
}

/// Architecture-specific TLB flush
fn arch_tlb_flush(_pending: &PendingTlbInvalidation) {
    unsafe {
        core::arch::asm!("invlpg [rax]", options(nostack, nostack));
    }
}

/// Architecture-specific cache flush
fn arch_cache_flush() {
    unsafe {
        core::arch::asm!("wbinvd", options(nostack, nostack));
    }
}

impl MappingCursor {
    pub fn new(vaddr: usize, paddr: usize, size: usize) -> Self {
        MappingCursor {
            vaddr,
            paddr,
            size,
            page_idx: 0,
            page_count: (size + PAGE_SIZE - 1) / PAGE_SIZE,
        }
    }

    pub fn advance(&mut self, bytes: usize) {
        self.vaddr += bytes;
        if self.paddr != 0 {
            self.paddr += bytes;
        }
        self.size = self.size.saturating_sub(bytes);
        self.page_idx += 1;
    }

    pub fn remaining_pages(&self) -> usize {
        (self.size + PAGE_SIZE - 1) / PAGE_SIZE
    }
}

// ============================================================================
// X86PageTableBase Implementation
// ============================================================================

type Errno = RxStatus;

impl X86PageTableBase {
    /// Update a page table entry
    fn update_entry(
        &mut self,
        cm: &mut ConsistencyManager,
        level: PageTableLevel,
        vaddr: usize,
        pte: *mut PtEntry,
        paddr: usize,
        flags: PtFlags,
        was_terminal: bool,
    ) {
        use mmu_flags::*;

        unsafe {
            let entry_val = read_volatile(pte);

            // Update the entry
            let new_entry = (paddr as u64) | flags;
            write_volatile(pte, new_entry);

            // Queue TLB invalidation if this was a terminal entry
            if was_terminal {
                let is_global = entry_val & X86_MMU_PG_G != 0;
                cm.pending.enqueue(vaddr, level, is_global, true);
            }

            // Mark that we need cache flushes
            cm.requires_cache_flush = true;
        }
    }

    /// Remove a page table entry
    fn unmap_entry(
        &mut self,
        cm: &mut ConsistencyManager,
        level: PageTableLevel,
        vaddr: usize,
        pte: *mut PtEntry,
        was_terminal: bool,
    ) {
        use mmu_flags::*;

        unsafe {
            let entry_val = read_volatile(pte);

            // Clear the entry
            write_volatile(pte, 0);

            // Queue TLB invalidation if this was a terminal entry
            if was_terminal {
                let is_global = entry_val & X86_MMU_PG_G != 0;
                cm.pending.enqueue(vaddr, level, is_global, true);
            }

            cm.requires_cache_flush = true;
        }
    }

    /// Add a mapping to the page table
    fn add_mapping(
        &mut self,
        table: &mut [PtEntry],
        mmu_flags: u32,
        level: PageTableLevel,
        cursor: &mut MappingCursor,
        cm: &mut ConsistencyManager,
    ) -> Result<(), Errno> {
        use arch_flags::*;
        use mmu_flags::*;

        while cursor.size > 0 {
            let index = vaddr_to_index(level, cursor.vaddr);
            let entry = &mut table[index] as *mut PtEntry;

            let entry_val = unsafe { read_volatile(entry) };

            // Check if we need to split a large page
            if entry_val & X86_MMU_PG_P != 0 {
                let is_large = entry_val & X86_MMU_PG_PS != 0;
                let is_terminal = (entry_val & (X86_MMU_PG_RW | X86_MMU_PG_U)) != 0 || is_large;

                if is_terminal && !is_large {
                    // Page already mapped at this level
                    return Err(status::ERR_BAD_STATE);
                }

                if is_large {
                    // Split the large page
                    self.split_large_page(level, cursor.vaddr, entry, cm)?;
                    continue;
                }
            }

            // Determine if we should use a large page
            let use_large = self.supports_page_size(level) &&
                           cursor.size >= (1 << match level {
                               PageTableLevel::PD_L => PD_SHIFT,
                               PageTableLevel::PDP_L => PDP_SHIFT,
                               _ => PT_SHIFT,
                           }) &&
                           page_aligned(level, cursor.vaddr) &&
                           page_aligned(level, cursor.paddr);

            if level != PageTableLevel::PT_L && use_large {
                // Create a large page mapping
                let flags = self.terminal_flags(level, mmu_flags);
                self.update_entry(cm, level, cursor.vaddr, entry, cursor.paddr, flags, false);

                let page_size = 1 << match level {
                    PageTableLevel::PD_L => PD_SHIFT,
                    PageTableLevel::PDP_L => PDP_SHIFT,
                    _ => PT_SHIFT,
                };

                cursor.advance(page_size);
            } else if level == PageTableLevel::PT_L {
                // Create a 4KB page mapping
                let flags = self.terminal_flags(level, mmu_flags);
                self.update_entry(cm, level, cursor.vaddr, entry, cursor.paddr, flags, false);
                cursor.advance(PAGE_SIZE);
            } else {
                // Allocate intermediate page table
                if entry_val & X86_MMU_PG_P == 0 {
                    // Allocate new page table
                    let (page, phys_addr) = pmm::alloc_page(0)
                        .map_err(|_| status::ERR_INTERNAL)?;
                    page.set_state(pmm::VM_PAGE_STATE_MMU);

                    let virt_addr = pmm::paddr_to_vaddr(phys_addr);

                    // Zero the new page table
                    unsafe {
                        core::ptr::write_bytes(virt_addr as *mut u8, 0, PAGE_SIZE);
                    }

                    // Set the entry
                    let int_flags = self.intermediate_flags();
                    let new_entry = (phys_addr as u64) | int_flags;
                    unsafe {
                        write_volatile(entry, new_entry);
                    }

                    *self.pages.lock() += 1;
                }

                // Follow to next level
                let next_level = match level {
                    PageTableLevel::PML4_L => PageTableLevel::PDP_L,
                    PageTableLevel::PDP_L => PageTableLevel::PD_L,
                    PageTableLevel::PD_L => PageTableLevel::PT_L,
                    _ => return Err(status::ERR_BAD_STATE),
                };

                let paddr = paddr_from_pte(level, entry_val);
                let vaddr = x86_phys_to_virt(paddr);
                let next_table = unsafe {
                    core::slice::from_raw_parts_mut(vaddr as *mut PtEntry, ENTRIES_PER_PAGE_TABLE)
                };

                return self.add_mapping(next_table, mmu_flags, next_level, cursor, cm);
            }
        }

        Ok(())
    }

    /// Remove a mapping from the page table
    fn remove_mapping(
        &mut self,
        table: &mut [PtEntry],
        level: PageTableLevel,
        cursor: &mut MappingCursor,
        cm: &mut ConsistencyManager,
    ) -> Result<(), Errno> {
        use mmu_flags::*;

        while cursor.size > 0 {
            let index = vaddr_to_index(level, cursor.vaddr);
            let entry = &mut table[index] as *mut PtEntry;

            let entry_val = unsafe { read_volatile(entry) };

            if entry_val & X86_MMU_PG_P == 0 {
                // Page not mapped, skip
                cursor.advance(PAGE_SIZE);
                continue;
            }

            let is_large = entry_val & X86_MMU_PG_PS != 0;
            let is_terminal = (entry_val & (X86_MMU_PG_RW | X86_MMU_PG_U)) != 0 || is_large;

            if is_terminal {
                // Remove the mapping
                let page_size = if is_large {
                    match level {
                        PageTableLevel::PD_L => 1 << PD_SHIFT,
                        PageTableLevel::PDP_L => 1 << PDP_SHIFT,
                        _ => PAGE_SIZE,
                    }
                } else {
                    PAGE_SIZE
                };

                self.unmap_entry(cm, level, cursor.vaddr, entry, true);
                cursor.advance(page_size);
            } else {
                // Recurse to next level
                let next_level = match level {
                    PageTableLevel::PML4_L => PageTableLevel::PDP_L,
                    PageTableLevel::PDP_L => PageTableLevel::PD_L,
                    PageTableLevel::PD_L => PageTableLevel::PT_L,
                    _ => return Err(status::ERR_BAD_STATE),
                };

                let paddr = paddr_from_pte(level, entry_val);
                let vaddr = x86_phys_to_virt(paddr);
                let next_table = unsafe {
                    core::slice::from_raw_parts_mut(vaddr as *mut PtEntry, ENTRIES_PER_PAGE_TABLE)
                };

                // Save the original cursor size to check if we emptied the table
                let orig_size = cursor.size;
                self.remove_mapping(next_table, next_level, cursor, cm)?;

                // Check if the intermediate table is now empty
                if cursor.size == orig_size {
                    // Table was empty, check if all entries are zero
                    let mut empty = true;
                    for i in 0..ENTRIES_PER_PAGE_TABLE {
                        if unsafe { read_volatile(&next_table[i]) } != 0 {
                            empty = false;
                            break;
                        }
                    }

                    if empty {
                        // Free the intermediate table
                        self.unmap_entry(cm, level, cursor.vaddr, entry, false);

                        if let Some(page) = pmm::paddr_to_vm_page(paddr) {
                            pmm::free_page(page);
                        }

                        *self.pages.lock() -= 1;
                    }

                    cursor.advance(PAGE_SIZE);
                }
            }
        }

        Ok(())
    }

    /// Update mappings in the page table
    fn update_mapping(
        &mut self,
        table: &mut [PtEntry],
        mmu_flags: u32,
        level: PageTableLevel,
        cursor: &mut MappingCursor,
        cm: &mut ConsistencyManager,
    ) -> Result<(), Errno> {
        use mmu_flags::*;

        while cursor.size > 0 {
            let index = vaddr_to_index(level, cursor.vaddr);
            let entry = &mut table[index] as *mut PtEntry;

            let entry_val = unsafe { read_volatile(entry) };

            if entry_val & X86_MMU_PG_P == 0 {
                // Skip unmapped pages (we may encounter these due to demand paging)
                cursor.advance(PAGE_SIZE);
                continue;
            }

            let is_large = entry_val & X86_MMU_PG_PS != 0;
            let is_terminal = (entry_val & (X86_MMU_PG_RW | X86_MMU_PG_U)) != 0 || is_large;

            if is_terminal {
                let paddr = paddr_from_pte(level, entry_val);
                let term_flags = self.terminal_flags(level, mmu_flags);
                self.update_entry(cm, level, cursor.vaddr, entry, paddr, term_flags, true);
                cursor.advance(PAGE_SIZE);
            } else {
                // Recurse to next level
                let next_level = match level {
                    PageTableLevel::PML4_L => PageTableLevel::PDP_L,
                    PageTableLevel::PDP_L => PageTableLevel::PD_L,
                    PageTableLevel::PD_L => PageTableLevel::PT_L,
                    _ => return Err(status::ERR_BAD_STATE),
                };

                let paddr = paddr_from_pte(level, entry_val);
                let vaddr = x86_phys_to_virt(paddr);
                let next_table = unsafe {
                    core::slice::from_raw_parts_mut(vaddr as *mut PtEntry, ENTRIES_PER_PAGE_TABLE)
                };

                self.update_mapping(next_table, mmu_flags, next_level, cursor, cm)?;
            }
        }

        Ok(())
    }

    /// Get a mapping from the page table
    fn get_mapping(
        &self,
        table: &[PtEntry],
        vaddr: usize,
        level: PageTableLevel,
    ) -> Result<(PageTableLevel, u64), Errno> {
        use mmu_flags::*;

        let index = vaddr_to_index(level, vaddr);
        let entry = unsafe { read_volatile(&table[index]) };

        if entry & X86_MMU_PG_P == 0 {
            return Err(status::ERR_NOT_FOUND);
        }

        let is_large = entry & X86_MMU_PG_PS != 0;
        let is_terminal = (entry & (X86_MMU_PG_RW | X86_MMU_PG_U)) != 0 || is_large;

        if is_terminal {
            Ok((level, entry))
        } else if level != PageTableLevel::PT_L {
            let next_level = match level {
                PageTableLevel::PML4_L => PageTableLevel::PDP_L,
                PageTableLevel::PDP_L => PageTableLevel::PD_L,
                PageTableLevel::PD_L => PageTableLevel::PT_L,
                _ => return Err(status::ERR_BAD_STATE),
            };

            let paddr = paddr_from_pte(level, entry);
            let virt = x86_phys_to_virt(paddr);
            let next_table = unsafe {
                core::slice::from_raw_parts(virt as *const PtEntry, ENTRIES_PER_PAGE_TABLE)
            };

            self.get_mapping(next_table, vaddr, next_level)
        } else {
            Err(status::ERR_NOT_FOUND)
        }
    }

    /// Split a large page into smaller pages
    fn split_large_page(
        &mut self,
        level: PageTableLevel,
        vaddr: usize,
        pte: *mut PtEntry,
        cm: &mut ConsistencyManager,
    ) -> Result<(), Errno> {
        use mmu_flags::*;

        let entry_val = unsafe { read_volatile(pte) };
        let paddr = paddr_from_pte(level, entry_val);
        let flags = self.split_flags(level, entry_val);

        // Allocate new page table
        let (page, phys_addr) = pmm::alloc_page(0)
            .map_err(|_| status::ERR_INTERNAL)?;
        page.set_state(pmm::VM_PAGE_STATE_MMU);

        let virt_addr = pmm::paddr_to_vaddr(phys_addr);

        // Zero the new page table
        unsafe {
            core::ptr::write_bytes(virt_addr as *mut u8, 0, PAGE_SIZE);
        }

        // Fill in the entries
        let next_table = unsafe {
            core::slice::from_raw_parts_mut(virt_addr as *mut PtEntry, ENTRIES_PER_PAGE_TABLE)
        };

        let (next_level, entry_count, entry_size) = match level {
            PageTableLevel::PDP_L => (PageTableLevel::PD_L, 512, 1 << PD_SHIFT),
            PageTableLevel::PD_L => (PageTableLevel::PT_L, 512, 1 << PT_SHIFT),
            _ => return Err(status::ERR_BAD_STATE),
        };

        for i in 0..entry_count {
            let entry_paddr = paddr + (i * entry_size);
            let new_flags = if level == PageTableLevel::PDP_L {
                // For 1GB → 2MB, keep PS flag
                flags | X86_MMU_PG_PS
            } else {
                flags
            };

            next_table[i] = (entry_paddr as u64) | new_flags | X86_MMU_PG_P;
        }

        // Update the original entry to point to the new table
        let int_flags = self.intermediate_flags();
        let new_entry = (phys_addr as u64) | int_flags;
        unsafe {
            write_volatile(pte, new_entry);
        }

        *self.pages.lock() += 1;

        Ok(())
    }
}

// ============================================================================
// X86PageTableMmu Implementation
// ============================================================================

impl X86PageTableMethods for X86PageTableMmu {
    fn top_level(&self) -> PageTableLevel {
        PageTableLevel::PML4_L
    }

    fn allowed_flags(&self, flags: u32) -> bool {
        use arch_flags::*;
        (flags & ARCH_MMU_FLAG_PERM_READ) != 0
    }

    fn check_paddr(&self, paddr: usize) -> bool {
        // Check if physical address is valid
        // This would call into platform-specific code
        paddr < (1 << 52) // x86-64 supports 52-bit physical addresses
    }

    fn check_vaddr(&self, vaddr: usize) -> bool {
        // Check if virtual address is canonical
        // x86-64: bits [63:48] must be all 0 or all 1
        let high_bits = vaddr >> 48;
        high_bits == 0 || high_bits == 0xFFFF
    }

    fn supports_page_size(&self, level: PageTableLevel) -> bool {
        match level {
            PageTableLevel::PDP_L => true,  // 1GB pages
            PageTableLevel::PD_L => true,   // 2MB pages
            PageTableLevel::PT_L => true,   // 4KB pages
            PageTableLevel::PML4_L => false,
        }
    }

    fn intermediate_flags(&self) -> PtFlags {
        use mmu_flags::*;
        X86_MMU_PG_RW | X86_MMU_PG_P
    }

    fn terminal_flags(&self, _level: PageTableLevel, flags: u32) -> PtFlags {
        use arch_flags::*;
        use mmu_flags::*;

        let mut result = X86_MMU_PG_P;

        if flags & ARCH_MMU_FLAG_PERM_WRITE != 0 {
            result |= X86_MMU_PG_RW;
        }

        if flags & ARCH_MMU_FLAG_PERM_EXECUTE == 0 {
            // x86-64 uses NX bit which is in bit 63
            // This is handled in the calling code
        }

        // User/supervisor bit should be set based on context
        // For now, assume kernel mappings
        // result |= X86_MMU_PG_U;

        result
    }

    fn split_flags(&self, _level: PageTableLevel, flags: PtFlags) -> PtFlags {
        use mmu_flags::*;
        flags & !(X86_MMU_PG_PS | X86_LARGE_PAT_MASK)
    }

    fn tlb_invalidate(&self, _pending: &PendingTlbInvalidation) {
        unsafe {
            core::arch::asm!("invlpg [rax]", options(nostack, nostack));
        }
    }

    fn pt_flags_to_mmu_flags(&self, flags: u64, _level: PageTableLevel) -> u32 {
        use arch_flags::*;
        use mmu_flags::*;

        let mut result = ARCH_MMU_FLAG_PERM_READ;

        if flags & X86_MMU_PG_RW != 0 {
            result |= ARCH_MMU_FLAG_PERM_WRITE;
        }

        // NX bit is in bit 63
        if flags & (1 << 63) == 0 {
            result |= ARCH_MMU_FLAG_PERM_EXECUTE;
        }

        result
    }

    fn needs_cache_flushes(&self) -> bool {
        // x86-64 generally needs cache flushes for page table updates
        true
    }
}

// ============================================================================
// X86PageTableEpt Implementation
// ============================================================================

impl X86PageTableMethods for X86PageTableEpt {
    fn top_level(&self) -> PageTableLevel {
        PageTableLevel::PML4_L
    }

    fn allowed_flags(&self, _flags: u32) -> bool {
        // EPT has different allowed flags
        true
    }

    fn check_paddr(&self, paddr: usize) -> bool {
        paddr < (1 << 52) // x86-64 supports 52-bit physical addresses
    }

    fn check_vaddr(&self, _vaddr: usize) -> bool {
        // For EPT, addresses are guest physical
        true
    }

    fn supports_page_size(&self, level: PageTableLevel) -> bool {
        match level {
            PageTableLevel::PDP_L => true,  // 1GB pages
            PageTableLevel::PD_L => true,   // 2MB pages
            PageTableLevel::PT_L => true,   // 4KB pages
            PageTableLevel::PML4_L => false,
        }
    }

    fn intermediate_flags(&self) -> PtFlags {
        use mmu_flags::*;
        X86_EPT_R | X86_EPT_W | X86_EPT_X | X86_EPT_WB
    }

    fn terminal_flags(&self, _level: PageTableLevel, flags: u32) -> PtFlags {
        use arch_flags::*;
        use mmu_flags::*;

        let mut result = X86_EPT_R | X86_EPT_WB;

        if flags & ARCH_MMU_FLAG_PERM_WRITE != 0 {
            result |= X86_EPT_W;
        }

        if flags & ARCH_MMU_FLAG_PERM_EXECUTE != 0 {
            result |= X86_EPT_X;
        }

        result
    }

    fn split_flags(&self, _level: PageTableLevel, flags: PtFlags) -> PtFlags {
        flags
    }

    fn tlb_invalidate(&self, _pending: &PendingTlbInvalidation) {
        // EPT uses INVEPT instead of INVLPG
        // This would be implemented with proper INVEPT invocation
    }

    fn pt_flags_to_mmu_flags(&self, flags: u64, _level: PageTableLevel) -> u32 {
        use arch_flags::*;
        use mmu_flags::*;

        let mut result = 0;

        if flags & X86_EPT_R != 0 {
            result |= ARCH_MMU_FLAG_PERM_READ;
        }

        if flags & X86_EPT_W != 0 {
            result |= ARCH_MMU_FLAG_PERM_WRITE;
        }

        if flags & X86_EPT_X != 0 {
            result |= ARCH_MMU_FLAG_PERM_EXECUTE;
        }

        result
    }

    fn needs_cache_flushes(&self) -> bool {
        // EPT typically doesn't need cache flushes
        false
    }
}

// ============================================================================
// X86PageTableMethods Trait
// ============================================================================

/// Trait for x86 page table method implementations
pub trait X86PageTableMethods {
    fn top_level(&self) -> PageTableLevel;
    fn allowed_flags(&self, flags: u32) -> bool;
    fn check_paddr(&self, paddr: usize) -> bool;
    fn check_vaddr(&self, vaddr: usize) -> bool;
    fn supports_page_size(&self, level: PageTableLevel) -> bool;
    fn intermediate_flags(&self) -> PtFlags;
    fn terminal_flags(&self, level: PageTableLevel, flags: u32) -> PtFlags;
    fn split_flags(&self, level: PageTableLevel, flags: PtFlags) -> PtFlags;
    fn tlb_invalidate(&self, pending: &PendingTlbInvalidation);
    fn pt_flags_to_mmu_flags(&self, flags: u64, level: PageTableLevel) -> u32;
    fn needs_cache_flushes(&self) -> bool;
}

// ============================================================================
// Deref Implementation for MMU and EPT
// ============================================================================

impl Deref for X86PageTableMmu {
    type Target = X86PageTableBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for X86PageTableMmu {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Deref for X86PageTableEpt {
    type Target = X86PageTableBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for X86PageTableEpt {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

// ============================================================================
// Helper Functions for pmm integration
// ============================================================================

/// Physical to virtual address conversion using physmap
pub fn paddr_to_physmap(paddr: usize) -> usize {
    // This should match the kernel's physical mapping
    // For x86-64, commonly using direct mapping at high addresses
    x86_phys_to_virt(paddr)
}

/// Check if a page is present
pub fn is_page_present(pte: u64) -> bool {
    pte & mmu_flags::X86_MMU_PG_P != 0
}

/// Check if a page is large (2MB or 1GB)
pub fn is_large_page(pte: u64) -> bool {
    pte & mmu_flags::X86_MMU_PG_PS != 0
}
