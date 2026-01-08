// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V early boot MMU setup
//!
//! This module provides early page table creation for RISC-V boot,
//! called from start.S while running in physical address space with
//! the MMU disabled. This code should be position independent.


use crate::arch::riscv64::mmu;
use crate::arch::riscv64::registers;
use crate::rustux::types::*;
use core::ptr;

// External boot allocator function (defined in start.S)
extern "C" {
    fn boot_alloc_page_phys() -> PAddr;
}

// Page size and shift
const PAGE_SIZE: usize = 4096;
const PAGE_SIZE_SHIFT: u32 = 12;

// Sv39 page table constants
const SV39_LEVELS: usize = 3;
const SV39_PAGE_TABLE_ENTRIES: usize = 512;

// Sv39 virtual address split (9 bits per level)
const SV39_VA_BITS_PER_LEVEL: u32 = 9;
const SV39_VA_BITS: u32 = 39;

// Kernel virtual address space
const KERNEL_BASE: usize = 0xFFFF_FFFF_C000_0000;
const KERNEL_LOAD_OFFSET: usize = 0x0010_0000; // 1MB

// PTE flags for boot mappings
const PTE_V: u64 = 1 << 0;   // Valid
const PTE_R: u64 = 1 << 1;   // Read
const PTE_W: u64 = 1 << 2;   // Write
const PTE_X: u64 = 1 << 3;   // Execute
const PTE_A: u64 = 1 << 6;   // Accessed
const PTE_D: u64 = 1 << 7;   // Dirty

// PTE flag combinations
const PTE_KERNEL_RW: u64 = PTE_V | PTE_R | PTE_W | PTE_A | PTE_D;
const PTE_KERNEL_RWX: u64 = PTE_V | PTE_R | PTE_W | PTE_X | PTE_A | PTE_D;

/// Sv39 page table entry type
pub type pte_t = u64;

/// Get VPN index at a specific level for Sv39
fn sv39_vpn_index(vaddr: usize, level: usize) -> usize {
    let shift = PAGE_SIZE_SHIFT + (SV39_VA_BITS_PER_LEVEL * (level as u32));
    (vaddr >> shift) & (SV39_PAGE_TABLE_ENTRIES - 1)
}

/// Called from start.S to allocate a page table from the boot allocator
///
/// # Safety
///
/// Must only be called during early boot when the boot allocator is available
#[no_mangle]
pub unsafe extern "C" fn boot_alloc_ptable() -> *mut pte_t {
    extern "C" {
        fn boot_alloc_page_phys() -> PAddr;
    }

    // Allocate a page from the boot allocator (returns physical address)
    let pa = boot_alloc_page_phys();

    // Convert to virtual address using direct mapping
    // For early boot, we use physical address = virtual address
    let ptr = pa as *mut pte_t;

    // Zero the page table entry
    // Avoid using memset since it may not be available
    for i in 0..SV39_PAGE_TABLE_ENTRIES {
        ptr.add(i).write_volatile(0);
    }

    ptr
}

/// Early boot mapping routine for Sv39
///
/// # Arguments
///
/// * `root_table` - Root page table (physical address)
/// * `vaddr` - Virtual address to map
/// * `paddr` - Physical address to map
/// * `len` - Length of mapping
/// * `flags` - PTE flags
///
/// # Returns
///
/// 0 on success, error code on failure
///
/// # Safety
///
/// Must be called with valid physical addresses and during early boot
unsafe fn riscv_boot_map(
    root_table: PAddr,
    vaddr: VAddr,
    paddr: PAddr,
    len: usize,
    flags: u64,
) -> i32 {
    let mut offset = 0;

    while offset < len {
        // At each level, we need to ensure the page table entry exists
        let mut table_pa = root_table;
        let mut table_va = table_pa as *mut pte_t;

        // Walk through all 3 levels of Sv39
        for level in (0..SV39_LEVELS).rev() {
            let index = sv39_vpn_index(vaddr + offset, level);
            let pte = table_va.add(index).read_volatile();

            // Check if PTE is valid
            if pte & PTE_V == 0 {
                // Need to allocate a new page table
                let new_table_pa = boot_alloc_page_phys();
                let new_table_va = new_table_pa as *mut pte_t;

                // Create the PTE entry pointing to the new table
                let new_pte = ((new_table_pa >> PAGE_SIZE_SHIFT) << 10) | PTE_V;
                table_va.add(index).write_volatile(new_pte);

                // Move to the new table for next iteration
                table_va = new_table_va;
            } else {
                // Valid entry, get the next level table
                let next_table_pa = (pte >> 10) << PAGE_SIZE_SHIFT;
                table_va = next_table_pa as *mut pte_t;
            }
        }

        // At level 0, we can create the final mapping
        let index = sv39_vpn_index(vaddr + offset, 0);
        let final_pte = ((paddr + offset as u64) & !((PAGE_SIZE - 1) as u64)) | flags;
        table_va.add(index).write_volatile(final_pte);

        offset += PAGE_SIZE;
    }

    0 // OK
}

/// Create identity mapping for kernel code and data
///
/// # Arguments
///
/// * `root_table` - Root page table (physical address)
/// * `start` - Start of region to identity map
/// * `end` - End of region to identity map
///
/// # Safety
///
/// Must be called during early boot with valid addresses
#[no_mangle]
pub unsafe extern "C" fn riscv_boot_identity_map(
    root_table: PAddr,
    start: PAddr,
    end: PAddr,
) -> i32 {
    let len = end - start;
    riscv_boot_map(root_table, start as usize, start, len as usize, PTE_KERNEL_RWX)
}

/// Map kernel to its high virtual address
///
/// # Arguments
///
/// * `root_table` - Root page table (physical address)
/// * `phys_start` - Physical start of kernel
/// * `phys_end` - Physical end of kernel
///
/// # Safety
///
/// Must be called during early boot with valid addresses
#[no_mangle]
pub unsafe extern "C" fn riscv_boot_map_kernel(
    root_table: PAddr,
    phys_start: PAddr,
    phys_end: PAddr,
) -> i32 {
    let len = (phys_end - phys_start) as usize;
    let virt_start = KERNEL_BASE as usize + phys_start as usize;
    riscv_boot_map(root_table, virt_start, phys_start, len, PTE_KERNEL_RWX)
}

/// Enable MMU with Sv39 paging
///
/// # Arguments
///
/// * `root_table_ppn` - Root page table physical page number
/// * `asid` - Address space ID (0 for kernel)
///
/// # Safety
///
/// Must only be called once during early boot
pub unsafe fn riscv_enable_mmu_sv39(root_table_ppn: u64, asid: u16) {
    // Construct SATP value for Sv39
    // Mode = 8 (Sv39), ASID, PPN
    let satp = (8u64 << 60) | ((asid as u64) << 32) | (root_table_ppn & 0xFFF_FFFF_FFFF);

    // Write SATP to enable paging
    core::arch::asm!(
        "csrw satp, {satp}",
        satp = in(reg) satp,
        options(nostack)
    );

    // Flush TLB
    core::arch::asm!("sfence.vma", options(nostack));
}

/// Main early MMU initialization
///
/// Called from start.S to set up initial page tables.
///
/// # Arguments
///
/// * `hart_id` - Current hart ID
///
/// # Returns
///
/// Physical address of the root page table
///
/// # Safety
///
/// Must only be called from start.S during early boot
#[no_mangle]
pub unsafe extern "C" fn riscv_early_mmu_init(hart_id: usize) -> PAddr {
    let _ = hart_id; // May be used for per-hart page tables

    // Allocate the root page table
    let root_table_pa = boot_alloc_page_phys() as PAddr;

    // Create identity map for low memory (first 2MB for boot code/data)
    riscv_boot_identity_map(root_table_pa, 0, 0x200_000);

    // Create identity map for kernel image
    // Assuming kernel is loaded at 1MB physical
    let kernel_phys_start: PAddr = KERNEL_LOAD_OFFSET as PAddr;
    let kernel_phys_end: PAddr = kernel_phys_start + 0x400_000; // 4MB for now
    riscv_boot_identity_map(root_table_pa, kernel_phys_start, kernel_phys_end);

    // Map kernel to high virtual address
    riscv_boot_map_kernel(root_table_pa, kernel_phys_start, kernel_phys_end);

    // Map UART for early console (physical address 0x1000_0000 is common)
    riscv_boot_map(
        root_table_pa,
        0x1000_1000, // Virtual UART address
        0x1000_0000, // Physical UART address
        0x1000,      // 4KB
        PTE_KERNEL_RW,
    );

    // Enable MMU
    let root_table_ppn = (root_table_pa >> PAGE_SIZE_SHIFT) as u64;
    riscv_enable_mmu_sv39(root_table_ppn, 0);

    root_table_pa
}

/// Constants for page table sizes
pub const BOOT_PT_SIZE: usize = PAGE_SIZE;
pub const BOOT_PT_ALIGNMENT: usize = PAGE_SIZE;

/// Assert that page table entries are 64-bit (8 bytes)
const _: () = assert!(core::mem::size_of::<pte_t>() == 8);
