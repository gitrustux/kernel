// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by an MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};
use core::ptr::{null_mut};
use spin::mutex::{Mutex, MutexGuard}; // Using spin::mutex for #![no_std] compatibility
use crate::rustux::types::{Status};
use crate::rustux::types::status;

pub type PtEntry = u64; // Page table entries are 64-bit wide
pub const PAGE_SIZE_SHIFT: usize = 12; // Assuming a 4KB page size
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_SHIFT; // 4096 bytes
pub const MAX_PENDING_INVALIDATIONS: usize = 32;

// Status codes matching zx_status_t from Zircon
pub type RxStatus = Status;

impl From<RxStatus> for Result<(), RxStatus> {
    fn from(status: RxStatus) -> Self {
        if status == status::OK {
            Ok(())
        } else {
            Err(status)
        }
    }
}

/// FFI bindings for C code
mod ffi {
    use super::*;

    /// Export the page table base interface for C code
    #[no_mangle]
    pub extern "C" fn rx_page_table_base_phys(table: &X86PageTableBase) -> usize {
        table.phys()
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_base_virt(table: &X86PageTableBase) -> *mut PtEntry {
        table.virt()
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_base_pages(table: &X86PageTableBase) -> usize {
        table.pages()
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_base_ctx(table: &X86PageTableBase) -> *mut core::ffi::c_void {
        table.ctx()
    }

    /// Export core page table operations
    #[no_mangle]
    pub extern "C" fn rx_page_table_map_pages(
        table: &mut X86PageTableBase,
        vaddr: usize,
        phys: *const usize,
        count: usize,
        flags: u32,
        mapped: *mut usize,
    ) -> RxStatus {
        let phys_slice = if !phys.is_null() && count > 0 {
            unsafe { core::slice::from_raw_parts(phys, count) }
        } else {
            &[]
        };
        
        match table.map_pages(vaddr, phys_slice, flags) {
            Ok(count) => {
                if !mapped.is_null() {
                    unsafe { *mapped = count; }
                }
                status::OK
            },
            Err(e) => e,
        }
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_map_pages_contiguous(
        table: &mut X86PageTableBase,
        vaddr: usize,
        paddr: usize,
        count: usize,
        flags: u32,
        mapped: *mut usize,
    ) -> RxStatus {
        match table.map_pages_contiguous(vaddr, paddr, count, flags) {
            Ok(count) => {
                if !mapped.is_null() {
                    unsafe { *mapped = count; }
                }
                status::OK
            },
            Err(e) => e,
        }
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_unmap_pages(
        table: &mut X86PageTableBase,
        vaddr: usize,
        count: usize,
        unmapped: *mut usize,
    ) -> RxStatus {
        match table.unmap_pages(vaddr, count) {
            Ok(count) => {
                if !unmapped.is_null() {
                    unsafe { *unmapped = count; }
                }
                status::OK
            },
            Err(e) => e,
        }
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_protect_pages(
        table: &mut X86PageTableBase,
        vaddr: usize,
        count: usize,
        flags: u32,
    ) -> RxStatus {
        match table.protect_pages(vaddr, count, flags) {
            Ok(()) => status::OK,
            Err(e) => e,
        }
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_query_vaddr(
        table: &X86PageTableBase,
        vaddr: usize,
        paddr: *mut usize,
        mmu_flags: *mut u32,
    ) -> RxStatus {
        match table.query_vaddr(vaddr) {
            Ok((addr, flags)) => {
                if !paddr.is_null() {
                    unsafe { *paddr = addr; }
                }
                if !mmu_flags.is_null() {
                    unsafe { *mmu_flags = flags; }
                }
                status::OK
            },
            Err(e) => e,
        }
    }

    /// Export initialization and destruction functions
    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_init(table: &mut X86PageTableMmu, ctx: *mut core::ffi::c_void) -> RxStatus {
        match table.init(ctx) {
            Ok(()) => status::OK,
            Err(e) => e,
        }
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_destroy(table: &mut X86PageTableMmu, base: usize, size: usize) {
        table.destroy(base, size);
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_init(table: &mut X86PageTableEpt, ctx: *mut core::ffi::c_void) -> RxStatus {
        match table.init(ctx) {
            Ok(()) => status::OK,
            Err(e) => e,
        }
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_destroy(table: &mut X86PageTableEpt, base: usize, size: usize) {
        table.destroy(base, size);
    }

    /// Export virtual method implementations for MMU and EPT
    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_top_level(_: &X86PageTableMmu) -> PageTableLevel {
        PageTableLevel::PML4_L // Always PML4_L for x86_64
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_top_level(_: &X86PageTableEpt) -> PageTableLevel {
        PageTableLevel::PML4_L // Always PML4_L for x86_64
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_allowed_flags(table: &X86PageTableMmu, flags: u32) -> bool {
        table.allowed_flags(flags)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_allowed_flags(table: &X86PageTableEpt, flags: u32) -> bool {
        table.allowed_flags(flags)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_check_paddr(table: &X86PageTableMmu, paddr: usize) -> bool {
        table.check_paddr(paddr)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_check_paddr(table: &X86PageTableEpt, paddr: usize) -> bool {
        table.check_paddr(paddr)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_check_vaddr(table: &X86PageTableMmu, vaddr: usize) -> bool {
        table.check_vaddr(vaddr)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_check_vaddr(table: &X86PageTableEpt, vaddr: usize) -> bool {
        table.check_vaddr(vaddr)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_supports_page_size(table: &X86PageTableMmu, level: PageTableLevel) -> bool {
        table.supports_page_size(level)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_supports_page_size(table: &X86PageTableEpt, level: PageTableLevel) -> bool {
        table.supports_page_size(level)
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_mmu_needs_cache_flushes(table: &X86PageTableMmu) -> bool {
        table.needs_cache_flushes()
    }

    #[no_mangle]
    pub extern "C" fn rx_page_table_ept_needs_cache_flushes(table: &X86PageTableEpt) -> bool {
        table.needs_cache_flushes()
    }

    /// Export pending TLB invalidation functions
    #[no_mangle]
    pub extern "C" fn rx_pending_tlb_invalidation_new() -> Box<PendingTlbInvalidation> {
        Box::new(PendingTlbInvalidation::new())
    }

    #[no_mangle]
    pub extern "C" fn rx_pending_tlb_invalidation_enqueue(
        pending: &mut PendingTlbInvalidation, 
        vaddr: usize,
        level: PageTableLevel,
        is_global_page: bool,
        is_terminal: bool,
    ) {
        pending.enqueue(vaddr, level, is_global_page, is_terminal);
    }

    #[no_mangle]
    pub extern "C" fn rx_pending_tlb_invalidation_clear(pending: &mut PendingTlbInvalidation) {
        pending.clear();
    }

    #[no_mangle]
    pub extern "C" fn rx_pending_tlb_invalidation_free(_: Box<PendingTlbInvalidation>) {
        // Box automatically deallocates when dropped
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PageTableLevel {
    PT_L,
    PD_L,
    PDP_L,
    PML4_L,
}

#[repr(C)]
pub struct PendingTlbInvalidation {
    full_shootdown: bool,
    contains_global: bool,
    count: usize,
    items: [TlbInvalidationItem; MAX_PENDING_INVALIDATIONS],
}

#[repr(C)]
pub struct TlbInvalidationItem {
    raw: u64,
}

impl TlbInvalidationItem {
    fn new(vaddr: usize, level: PageTableLevel, is_global: bool, is_terminal: bool) -> Self {
        let encoded_addr = (vaddr >> PAGE_SIZE_SHIFT) as u64;
        let level_bits = level as u64 & 0b11;
        let global_bit = (is_global as u64) << 3;
        let terminal_bit = (is_terminal as u64) << 4;
        let raw = level_bits | global_bit | terminal_bit | (encoded_addr << 12);
        
        TlbInvalidationItem { raw }
    }

    fn page_level(&self) -> usize {
        (self.raw & 0b111) as usize // Extracting page level bits
    }

    fn is_global(&self) -> bool {
        (self.raw >> 3) & 0b1 != 0 // Extracting global bit
    }

    fn is_terminal(&self) -> bool {
        (self.raw >> 4) & 0b1 != 0 // Extracting terminal bit
    }

    fn encoded_addr(&self) -> u64 {
        (self.raw >> 12) // Extracting encoded address
    }

    pub fn addr(&self) -> usize {
        (self.encoded_addr() as usize) << PAGE_SIZE_SHIFT
    }
}

impl PendingTlbInvalidation {
    pub fn new() -> Self {
        PendingTlbInvalidation {
            full_shootdown: false,
            contains_global: false,
            count: 0,
            items: [TlbInvalidationItem { raw: 0 }; MAX_PENDING_INVALIDATIONS],
        }
    }

    pub fn enqueue(&mut self, vaddr: usize, level: PageTableLevel, is_global_page: bool, is_terminal: bool) {
        if self.count < MAX_PENDING_INVALIDATIONS {
            self.items[self.count] = TlbInvalidationItem::new(vaddr, level, is_global_page, is_terminal);
            self.count += 1;
            if is_global_page {
                self.contains_global = true;
            }
        } else {
            // If we run out of space, just do a full shootdown
            self.full_shootdown = true;
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.contains_global = false;
        self.full_shootdown = false;
    }
}

// Helper structures needed for implementation
pub struct CacheLineFlusher;
pub struct ConsistencyManager {
    pub pending: PendingTlbInvalidation,
    pub requires_cache_flush: bool,
}

impl ConsistencyManager {
    pub fn new<T>(_table: &T) -> Self {
        ConsistencyManager {
            pending: PendingTlbInvalidation::new(),
            requires_cache_flush: false,
        }
    }

    pub fn finish(&mut self) {
        // Flush TLB if needed
        if self.pending.count > 0 || self.pending.full_shootdown {
            // TLB flush would happen here
        }

        // Flush cache if needed
        if self.requires_cache_flush {
            // Cache flush would happen here
        }

        self.pending.clear();
        self.requires_cache_flush = false;
    }
}

pub struct MappingCursor {
    pub vaddr: usize,
    pub paddr: usize,
    pub size: usize,
    pub page_idx: usize,
    pub page_count: usize,
}

impl MappingCursor {
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

// Base structure providing common functionality for x86 page table management.
pub struct X86PageTableBase {
    phys: usize,
    virt: *mut PtEntry,
    pages: Mutex<usize>,
    ctx: *mut core::ffi::c_void,
    canary: u64, // Used as a magic number for debugging
}

// Type aliases for flags to match C++ types
pub type PtFlags = u64;
pub type IntermediatePtFlags = u64;

impl X86PageTableBase {
    const CANARY_MAGIC: u64 = 0x5836500000000000; // "X86P" as a magic number

    pub fn new() -> Self {
        X86PageTableBase {
            phys: 0,
            virt: null_mut(),
            pages: Mutex::new(0),
            ctx: null_mut(),
            canary: Self::CANARY_MAGIC,
        }
    }

    pub fn phys(&self) -> usize {
        self.phys
    }

    pub fn virt(&self) -> *mut PtEntry {
        self.virt
    }

    pub fn pages(&self) -> usize {
        *self.pages.lock()
    }

    pub fn ctx(&self) -> *mut core::ffi::c_void {
        self.ctx
    }

    // Core functionality methods that implement the C++ class methods
    pub fn init(&mut self, ctx: *mut core::ffi::c_void) -> Result<(), RxStatus> {
        // Verify canary
        if self.canary != Self::CANARY_MAGIC {
            return Err(status::ERR_BAD_STATE);
        }
        
        self.ctx = ctx;
        Ok(())
    }

    pub fn destroy(&mut self, base: usize, size: usize) {
        // In a full implementation, this would free the page tables
        // and potentially check for leaks
        let _ = (base, size); // Suppress unused variable warnings
    }

    pub fn map_pages(&mut self, vaddr: usize, phys: &[usize], flags: u32) -> Result<usize, RxStatus> {
        // Check vaddr validity
        if !self.check_vaddr(vaddr) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // Check flags validity
        if !self.allowed_flags(flags) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // In a full implementation, this would map pages in the page table
        // This is a placeholder
        Ok(phys.len())
    }

    pub fn map_pages_contiguous(&mut self, vaddr: usize, paddr: usize, count: usize, flags: u32) -> Result<usize, RxStatus> {
        // Check vaddr validity
        if !self.check_vaddr(vaddr) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // Check paddr validity
        if !self.check_paddr(paddr) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // Check flags validity
        if !self.allowed_flags(flags) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // In a full implementation, this would map contiguous pages in the page table
        // This is a placeholder
        Ok(count)
    }

    pub fn unmap_pages(&mut self, vaddr: usize, count: usize) -> Result<usize, RxStatus> {
        // Check vaddr validity
        if !self.check_vaddr(vaddr) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // In a full implementation, this would unmap pages from the page table
        // This is a placeholder
        Ok(count)
    }

    pub fn protect_pages(&mut self, vaddr: usize, count: usize, flags: u32) -> Result<(), RxStatus> {
        // Check vaddr validity
        if !self.check_vaddr(vaddr) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // Check flags validity
        if !self.allowed_flags(flags) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // In a full implementation, this would change page protection flags
        // This is a placeholder
        Ok(())
    }

    pub fn query_vaddr(&self, vaddr: usize) -> Result<(usize, u32), RxStatus> {
        // Check vaddr validity
        if !self.check_vaddr(vaddr) {
            return Err(status::ERR_INVALID_ARGS);
        }

        // In a full implementation, this would look up the physical address and flags
        // This is a placeholder
        Err(status::ERR_NOT_SUPPORTED)
    }

    // Virtual methods that must be implemented by derived classes
    pub fn top_level(&self) -> PageTableLevel {
        unimplemented!("top_level must be implemented by derived classes")
    }

    pub fn allowed_flags(&self, _flags: u32) -> bool {
        unimplemented!("allowed_flags must be implemented by derived classes")
    }

    pub fn check_paddr(&self, _paddr: usize) -> bool {
        unimplemented!("check_paddr must be implemented by derived classes")
    }

    pub fn check_vaddr(&self, _vaddr: usize) -> bool {
        unimplemented!("check_vaddr must be implemented by derived classes")
    }

    pub fn supports_page_size(&self, _level: PageTableLevel) -> bool {
        unimplemented!("supports_page_size must be implemented by derived classes")
    }

    pub fn intermediate_flags(&self) -> IntermediatePtFlags {
        unimplemented!("intermediate_flags must be implemented by derived classes")
    }

    pub fn terminal_flags(&self, _level: PageTableLevel, _flags: u32) -> PtFlags {
        unimplemented!("terminal_flags must be implemented by derived classes")
    }

    pub fn split_flags(&self, _level: PageTableLevel, _flags: PtFlags) -> PtFlags {
        unimplemented!("split_flags must be implemented by derived classes")
    }

    pub fn tlb_invalidate(&self, _pending: &mut PendingTlbInvalidation) {
        unimplemented!("tlb_invalidate must be implemented by derived classes")
    }

    pub fn pt_flags_to_mmu_flags(&self, _flags: PtFlags, _level: PageTableLevel) -> u32 {
        unimplemented!("pt_flags_to_mmu_flags must be implemented by derived classes")
    }

    pub fn needs_cache_flushes(&self) -> bool {
        unimplemented!("needs_cache_flushes must be implemented by derived classes")
    }

    // Helper methods for implementation
    fn update_entry(&mut self, _cm: &mut ConsistencyManager, _level: PageTableLevel, _vaddr: usize,
                   _pte: *mut PtEntry, _paddr: usize, _flags: PtFlags, _was_terminal: bool) {
        // Implementation would go here
    }

    fn unmap_entry(&mut self, _cm: &mut ConsistencyManager, _level: PageTableLevel, _vaddr: usize,
                  _pte: *mut PtEntry, _was_terminal: bool) {
        // Implementation would go here
    }

    fn split_large_page(&mut self, _level: PageTableLevel, _vaddr: usize, 
                       _pte: *mut PtEntry, _cm: &mut ConsistencyManager) -> Result<(), RxStatus> {
        // Implementation would go here
        Ok(())
    }
}

// Implementation of derived classes

pub struct X86PageTableMmu {
    base: X86PageTableBase,
}

impl X86PageTableMmu {
    pub fn new() -> Self {
        X86PageTableMmu {
            base: X86PageTableBase::new(),
        }
    }

    pub fn init(&mut self, ctx: *mut core::ffi::c_void) -> Result<(), RxStatus> {
        self.base.init(ctx)
    }

    pub fn destroy(&mut self, base: usize, size: usize) {
        self.base.destroy(base, size)
    }

    pub fn top_level(&self) -> PageTableLevel {
        PageTableLevel::PML4_L
    }

    pub fn allowed_flags(&self, _flags: u32) -> bool {
        // Implementation would check if flags are valid for MMU
        true
    }

    pub fn check_paddr(&self, _paddr: usize) -> bool {
        // Implementation would check if physical address is valid
        true
    }

    pub fn check_vaddr(&self, _vaddr: usize) -> bool {
        // Implementation would check if virtual address is valid
        true
    }

    pub fn supports_page_size(&self, level: PageTableLevel) -> bool {
        // Implementation would check if this page size is supported
        match level {
            PageTableLevel::PT_L => true,
            PageTableLevel::PD_L => true,
            PageTableLevel::PDP_L => true,
            _ => false,
        }
    }

    pub fn needs_cache_flushes(&self) -> bool {
        // Implementation would determine if cache flushes are needed
        true
    }
}

pub struct X86PageTableEpt {
    base: X86PageTableBase,
}

impl X86PageTableEpt {
    pub fn new() -> Self {
        X86PageTableEpt {
            base: X86PageTableBase::new(),
        }
    }

    pub fn init(&mut self, ctx: *mut core::ffi::c_void) -> Result<(), RxStatus> {
        self.base.init(ctx)
    }

    pub fn destroy(&mut self, base: usize, size: usize) {
        self.base.destroy(base, size)
    }

    pub fn top_level(&self) -> PageTableLevel {
        PageTableLevel::PML4_L
    }

    pub fn allowed_flags(&self, _flags: u32) -> bool {
        // Implementation would check if flags are valid for EPT
        true
    }

    pub fn check_paddr(&self, _paddr: usize) -> bool {
        // Implementation would check if physical address is valid
        true
    }

    pub fn check_vaddr(&self, _vaddr: usize) -> bool {
        // Implementation would check if virtual address is valid
        true
    }

    pub fn supports_page_size(&self, level: PageTableLevel) -> bool {
        // Implementation would check if this page size is supported
        match level {
            PageTableLevel::PT_L => true,
            PageTableLevel::PD_L => true,
            PageTableLevel::PDP_L => true,
            _ => false,
        }
    }

    pub fn needs_cache_flushes(&self) -> bool {
        // Implementation would determine if cache flushes are needed
        false // EPT typically doesn't need cache flushes
    }
}