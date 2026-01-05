// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Memory Management Unit (MMU) definitions and operations
//!
//! This module provides constants, types and functions for managing
//! the x86 memory management unit, including page tables and memory types.

use crate::arch::amd64::page_tables::constants::*;
use crate::rustux::types::*;

// Extended Page Table (EPT) flags for virtualization
/// EPT Read permission
pub const X86_EPT_R: u64 = 1 << 0;
/// EPT Write permission
pub const X86_EPT_W: u64 = 1 << 1;
/// EPT Execute permission
pub const X86_EPT_X: u64 = 1 << 2;
/// EPT Accessed flag
pub const X86_EPT_A: u64 = 1 << 8;
/// EPT Dirty flag
pub const X86_EPT_D: u64 = 1 << 9;

// EPT Memory Type flags (from Volume 3, Section 28.2.6: EPT and Memory Typing)
/// EPT Memory Type mask
pub const X86_EPT_MEMORY_TYPE_MASK: u64 = 7 << 3;
/// EPT Uncached memory type
pub const X86_EPT_UC: u64 = 0 << 3;
/// EPT Write-combining memory type
pub const X86_EPT_WC: u64 = 1 << 3;
/// EPT Write-through memory type
pub const X86_EPT_WT: u64 = 4 << 3;
/// EPT Write-protected memory type
pub const X86_EPT_WP: u64 = 5 << 3;
/// EPT Write-back memory type
pub const X86_EPT_WB: u64 = 6 << 3;

// Page Attribute Table memory types, defined in Table 11-10 of Intel 3A
/// PAT Uncached memory type
pub const X86_PAT_UC: u8 = 0x00;
/// PAT Write-combining memory type
pub const X86_PAT_WC: u8 = 0x01;
/// PAT Write-through memory type
pub const X86_PAT_WT: u8 = 0x04;
/// PAT Write-protected memory type
pub const X86_PAT_WP: u8 = 0x05;
/// PAT Write-back memory type
pub const X86_PAT_WB: u8 = 0x06;
/// PAT Weakly Uncached memory type (can be overridden by a WC MTRR setting)
pub const X86_PAT_UC_: u8 = 0x07;

// PAT index configurations
/// PAT index 0 - write-back (default)
pub const X86_PAT_INDEX0: u8 = X86_PAT_WB;
/// PAT index 1 - write-through (default)
pub const X86_PAT_INDEX1: u8 = X86_PAT_WT;
/// PAT index 2 - weakly uncached (default)
pub const X86_PAT_INDEX2: u8 = X86_PAT_UC_;
/// PAT index 3 - uncached (default)
pub const X86_PAT_INDEX3: u8 = X86_PAT_UC;
/// PAT index 4 - write-back (default)
pub const X86_PAT_INDEX4: u8 = X86_PAT_WB;
/// PAT index 5 - write-through (default)
pub const X86_PAT_INDEX5: u8 = X86_PAT_WT;
/// PAT index 6 - weakly uncached (default)
pub const X86_PAT_INDEX6: u8 = X86_PAT_UC_;
/// PAT index 7 - write-combining (UC by default)
pub const X86_PAT_INDEX7: u8 = X86_PAT_WC;

// PTE PAT selectors
/// PTE PAT selector for write-back caching
pub const X86_MMU_PTE_PAT_WRITEBACK: u64 = X86_PAT_PTE_SELECTOR(0);
/// PTE PAT selector for write-through caching
pub const X86_MMU_PTE_PAT_WRITETHROUGH: u64 = X86_PAT_PTE_SELECTOR(1);
/// PTE PAT selector for uncachable memory
pub const X86_MMU_PTE_PAT_UNCACHABLE: u64 = X86_PAT_PTE_SELECTOR(3);
/// PTE PAT selector for write-combining memory
pub const X86_MMU_PTE_PAT_WRITE_COMBINING: u64 = X86_PAT_PTE_SELECTOR(7);

// Large page PAT selectors
/// Large page PAT selector for write-back caching
pub const X86_MMU_LARGE_PAT_WRITEBACK: u64 = X86_PAT_LARGE_SELECTOR(0);
/// Large page PAT selector for write-through caching
pub const X86_MMU_LARGE_PAT_WRITETHROUGH: u64 = X86_PAT_LARGE_SELECTOR(1);
/// Large page PAT selector for uncachable memory
pub const X86_MMU_LARGE_PAT_UNCACHABLE: u64 = X86_PAT_LARGE_SELECTOR(3);
/// Large page PAT selector for write-combining memory
pub const X86_MMU_LARGE_PAT_WRITE_COMBINING: u64 = X86_PAT_LARGE_SELECTOR(7);

/// Default flags for inner page directory entries
pub const X86_KERNEL_PD_FLAGS: u64 = X86_MMU_PG_RW | X86_MMU_PG_P;

/// Default flags for 2MB/4MB/1GB page directory entries (large pages)
pub const X86_KERNEL_PD_LP_FLAGS: u64 = X86_MMU_PG_G | X86_MMU_PG_PS | X86_MMU_PG_RW | X86_MMU_PG_P;

/// No Execute (NX) flag for page table entries
pub const X86_MMU_PG_NX: u64 = 1u64 << 63;

/// Number of paging levels in the x86-64 page table hierarchy
pub const X86_PAGING_LEVELS: u8 = 4;

/// Shift for guest physical address space size
pub const MMU_GUEST_SIZE_SHIFT: u8 = 48;

// Page fault error code flags
/// Page fault: page present
pub const PFEX_P: u32 = 1 << 0;
/// Page fault: write access
pub const PFEX_W: u32 = 1 << 1;
/// Page fault: user mode access
pub const PFEX_U: u32 = 1 << 2;
/// Page fault: reserved bit set
pub const PFEX_RSV: u32 = 1 << 3;
/// Page fault: instruction fetch
pub const PFEX_I: u32 = 1 << 4;
/// Page fault: protection key violation
pub const PFEX_PK: u32 = 1 << 5;
/// Page fault: SGX violation
pub const PFEX_SGX: u32 = 1 << 15;

/// Structure representing a memory mapping range
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MapRange {
    /// Starting virtual address
    pub start_vaddr: VAddr,
    /// Starting physical address (32 bits wide in PAE mode)
    pub start_paddr: PAddr,
    /// Size of the mapping in bytes
    pub size: usize,
}

/// Check if a virtual address is canonical
///
/// In x86-64, valid addresses must sign-extend the 48th bit through the upper bits
///
/// # Arguments
///
/// * `vaddr` - Virtual address to check
///
/// # Returns
///
/// True if the address is canonical, false otherwise
pub fn x86_is_vaddr_canonical(vaddr: VAddr) -> bool {
    unsafe { sys_x86_is_vaddr_canonical(vaddr) }
}

/// Check if a physical address is valid
///
/// # Arguments
///
/// * `paddr` - Physical address to check
///
/// # Returns
///
/// True if the address is valid, false otherwise
pub fn x86_mmu_check_paddr(paddr: PAddr) -> bool {
    unsafe { sys_x86_mmu_check_paddr(paddr) }
}

/// Initialize MMU state for the current CPU
///
/// # Safety
///
/// This function is unsafe because it directly modifies CPU state
pub unsafe fn x86_mmu_percpu_init() {
    sys_x86_mmu_percpu_init();
}

/// Early initialization of the MMU subsystem
///
/// # Safety
///
/// This function is unsafe because it directly modifies system memory management state
pub unsafe fn x86_mmu_early_init() {
    sys_x86_mmu_early_init();
}

/// Complete initialization of the MMU subsystem
///
/// # Safety
///
/// This function is unsafe because it directly modifies system memory management state
pub unsafe fn x86_mmu_init() {
    sys_x86_mmu_init();
}

/// Get the physical address of the kernel's page table root (CR3)
///
/// # Returns
///
/// Physical address of the kernel's CR3 value
pub fn x86_kernel_cr3() -> PAddr {
    unsafe { sys_x86_kernel_cr3() }
}

extern "C" {
    fn sys_x86_is_vaddr_canonical(vaddr: VAddr) -> bool;
    fn sys_x86_mmu_check_paddr(paddr: PAddr) -> bool;
    fn sys_x86_mmu_percpu_init();
    fn sys_x86_mmu_early_init();
    fn sys_x86_mmu_init();
    fn sys_x86_kernel_cr3() -> PAddr;
}

/// Default virtual address width (will be updated at runtime)
pub mut G_VADDR_WIDTH: u8 = 48;
/// Default physical address width (will be updated at runtime)
pub mut G_PADDR_WIDTH: u8 = 32;
/// Whether the system supports 1GB huge pages
pub mut SUPPORTS_HUGE_PAGES: bool = false;

/// Check if a virtual address is canonical
///
/// In x86-64, valid addresses must sign-extend the 48th bit through the upper bits.
/// This is a pure Rust implementation of the check.
pub fn x86_is_vaddr_canonical_impl(vaddr: VAddr) -> bool {
    let max_vaddr_lohalf: u64 = (1u64 << (G_VADDR_WIDTH - 1)) - 1;
    let min_vaddr_hihalf: u64 = !max_vaddr_lohalf;

    // Check to see if the address is a canonical address
    !((vaddr as u64 > max_vaddr_lohalf) && (vaddr as u64 < min_vaddr_hihalf))
}

/// Check if a virtual address is aligned and canonical
pub fn x86_mmu_check_vaddr(vaddr: VAddr) -> bool {
    use crate::arch::amd64::page_tables::constants::PAGE_SIZE;

    // Check to see if the address is PAGE aligned
    if vaddr & (PAGE_SIZE - 1) != 0 {
        return false;
    }

    x86_is_vaddr_canonical_impl(vaddr)
}

/// Check if a physical address is valid and aligned
pub fn x86_mmu_check_paddr_impl(paddr: PAddr) -> bool {
    use crate::arch::amd64::page_tables::constants::PAGE_SIZE;

    // Check to see if the address is PAGE aligned
    if paddr & (PAGE_SIZE - 1) != 0 {
        return false;
    }

    let max_paddr: u64 = (1u64 << G_PADDR_WIDTH) - 1;
    paddr <= max_paddr as usize
}

/// Invalidate all TLB entries, including global entries
///
/// This implements the Intel-recommended method for global TLB invalidation
/// by temporarily disabling and re-enabling the PGE bit in CR4.
pub unsafe fn x86_tlb_global_invalidate() {
    use crate::arch::amd64::registers::{X86_CR4_PGE, x86_get_cr4, x86_set_cr4};

    let cr4 = x86_get_cr4();
    if cr4 & X86_CR4_PGE != 0 {
        x86_set_cr4(cr4 & !X86_CR4_PGE);
        x86_set_cr4(cr4);
    } else {
        // If PGE is not enabled, just reload CR3
        use crate::arch::amd64::registers::{x86_get_cr3, x86_set_cr3};
        let cr3 = x86_get_cr3();
        x86_set_cr3(cr3);
    }
}

/// Invalidate all TLB entries, excluding global entries
pub unsafe fn x86_tlb_nonglobal_invalidate() {
    use crate::arch::amd64::registers::{x86_get_cr3, x86_set_cr3};

    let cr3 = x86_get_cr3();
    x86_set_cr3(cr3);
}

/// Invalidate a single TLB entry for the given virtual address
///
/// # Safety
/// Caller must ensure the address is valid and page-aligned
pub unsafe fn x86_tlb_invalidate_page(vaddr: VAddr) {
    core::arch::asm!("invlpg [{}]", in(reg) vaddr, options(nostack, nomem));
}

/// Invalidate TLB entries on multiple CPUs
///
/// # Safety
/// Caller must ensure proper synchronization
pub unsafe fn x86_tlb_invalidate_page_on_cpus(target_mask: u64, vaddr: VAddr) {
    // TODO: Implement IPI-based TLB shootdown for SMP
    // For now, just invalidate locally
    x86_tlb_invalidate_page(vaddr);
}

/// Early MMU initialization
///
/// This is called very early in boot to set up basic MMU state
pub unsafe fn x86_mmu_early_init_impl() {
    x86_mmu_percpu_init();
    x86_mmu_mem_type_init();

    // TODO: Unmap the lower identity mapping
    // This requires access to the PML4 and TLB invalidation

    // Get the address width from the CPU
    // TODO: Call x86_linear_address_width() and x86_physical_address_width()
    // For now, use the defaults

    // Check for huge page support
    // TODO: Call x86_feature_test(X86_FEATURE_HUGE_PAGE)
    // For now, assume no huge pages
    SUPPORTS_HUGE_PAGES = false;
}

/// Complete MMU initialization
///
/// This is called after the VM subsystem is up
pub unsafe fn x86_mmu_init_impl() {
    // Currently empty in the original code
    // Placeholder for future initialization
}

/// Per-CPU MMU initialization
pub unsafe fn x86_mmu_percpu_init_impl() {
    // Initialize PAT (Page Attribute Table)
    // TODO: Set up PAT MSR for proper memory type handling
}

/// Initialize memory types (PAT/MTRR)
pub unsafe fn x86_mmu_mem_type_init() {
    // TODO: Implement PAT and MTRR initialization
    // For now, use BIOS defaults
}

/// Synchronize PAT settings across CPUs
///
/// # Safety
/// Caller must ensure proper CPU mask and synchronization
pub unsafe fn x86_pat_sync(cpu_mask: u64) {
    // TODO: Implement PAT synchronization for SMP
    // This ensures all CPUs have consistent memory type settings
}

/// Get the kernel's CR3 value (physical address of kernel page table)
pub fn x86_kernel_cr3_impl() -> PAddr {
    // The kernel page table physical address is computed as:
    // KERNEL_PT - __code_start + KERNEL_LOAD_OFFSET
    // For now, return a placeholder
    use crate::arch::amd64::page_tables::constants::KERNEL_BASE;
    use crate::arch::amd64::page_tables::constants::KERNEL_LOAD_OFFSET;

    // TODO: Get the actual value from the linker
    KERNEL_BASE - KERNEL_LOAD_OFFSET
}

/// Convert physical address to kernel virtual address
///
/// # Safety
/// Caller must ensure the physical address is valid and mapped
pub unsafe fn paddr_to_physmap(paddr: PAddr) -> VAddr {
    use crate::arch::amd64::page_tables::constants::PHYSMAP_BASE;

    PHYSMAP_BASE + paddr
}