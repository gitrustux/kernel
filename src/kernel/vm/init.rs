// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! VM Initialization
//!
//! This module provides VM system initialization functions.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::rustux::types::*;
use super::{Result, VmError};
use super::aspace;
use super::pmm;

/// Page size (4KB)
const PAGE_SIZE: usize = 4096;

/// Page alignment mask
const PAGE_MASK: usize = PAGE_SIZE - 1;

/// Round up to page boundary
#[inline]
pub const fn page_align(size: usize) -> usize {
    (size + PAGE_MASK) & !PAGE_MASK
}

/// Round down to page boundary
#[inline]
pub const fn page_rounddown(addr: usize) -> usize {
    addr & !PAGE_MASK
}

/// Zero page physical address
static ZERO_PAGE_PADDR: AtomicU64 = AtomicU64::new(0);

/// Kernel base physical address
static KERNEL_BASE_PHYS: AtomicU64 = AtomicU64::new(0);

/// VM initialization state
static VM_INIT_STATE: Mutex<VmInitState> = Mutex::new(VmInitState::Uninitialized);

/// VM initialization states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmInitState {
    Uninitialized,
    PreHeapInitialized,
    FullyInitialized,
}

/// Mark a range of physical pages as WIRED
///
/// # Arguments
///
/// * `pa` - Physical address of the range
/// * `len` - Length of the range in bytes
pub fn mark_pages_in_use_phys(pa: u64, len: usize) -> Result<()> {
    // Make sure we are inclusive of all of the pages in the address range
    let aligned_len = page_align(len + (pa as usize & PAGE_MASK));
    let aligned_pa = page_rounddown(pa as usize) as u64;

    // Allocate the range to mark it as in use
    // TODO: Implement pmm_alloc_range
    let _ = (aligned_pa, aligned_len);

    Ok(())
}

/// VM initialization before heap allocation
///
/// This function initializes the VM system early in the boot process,
/// before the heap is available. It sets up the zero page and marks
/// boot allocator pages as wired.
pub fn vm_init_preheap() -> Result<()> {
    let mut state = VM_INIT_STATE.lock();

    if *state != VmInitState::Uninitialized {
        return Err(VmError::InvalidArgs);
    }

    // Initialize kernel address space
    // TODO: Implement kernel_aspace_init_preheap in aspace module
    // aspace::kernel_aspace_init_preheap()?;

    // Mark boot allocator pages as wired
    // TODO: Get boot_alloc_start and boot_alloc_end from boot allocator
    // mark_pages_in_use_phys(boot_alloc_start, boot_alloc_end - boot_alloc_start)?;

    // Allocate and zero the zero page
    match pmm::pmm_alloc_page(0) {
        Ok((_page_ptr, pa)) => {
            ZERO_PAGE_PADDR.store(pa as u64, Ordering::Release);

            // Zero the page
            // TODO: Implement arch_zero_page or use paddr_to_physmap
            // let ptr = paddr_to_physmap(pa);
            // arch_zero_page(ptr);
        }
        Err(e) => {
            return Err(e);
        }
    }

    *state = VmInitState::PreHeapInitialized;
    Ok(())
}

/// Temporary region descriptor for kernel memory regions
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TempRegion {
    /// Region name
    pub name: &'static str,
    /// Base virtual address
    pub base: u64,
    /// Size in bytes
    pub size: usize,
    /// Architecture MMU flags
    pub arch_mmu_flags: u64,
}

/// Full VM initialization
///
/// This function completes VM initialization after the heap is available.
/// It reserves kernel memory regions (code, rodata, data, bss) and sets
/// up their protection flags.
pub fn vm_init() -> Result<()> {
    let mut state = VM_INIT_STATE.lock();

    if *state != VmInitState::PreHeapInitialized {
        return Err(VmError::InvalidArgs);
    }

    // Create or get kernel address space
    // TODO: Implement kernel_aspace() function
    // let aspace = aspace::kernel_aspace()?;

    // For now, create a new kernel address space
    let aspace = aspace::AddressSpace::new_kernel()?;

    // Define kernel memory regions
    // TODO: Get actual addresses from linker symbols
    let regions = [
        TempRegion {
            name: "kernel_code",
            base: 0xffffffff80100000u64, // __code_start
            size: page_align(0x100000), // __code_end - __code_start
            arch_mmu_flags: 0x1, // ARCH_MMU_FLAG_PERM_READ | ARCH_MMU_FLAG_PERM_EXECUTE
        },
        TempRegion {
            name: "kernel_rodata",
            base: 0xffffffff80200000u64, // __rodata_start
            size: page_align(0x100000), // __rodata_end - __rodata_start
            arch_mmu_flags: 0x1, // ARCH_MMU_FLAG_PERM_READ
        },
        TempRegion {
            name: "kernel_data",
            base: 0xffffffff80300000u64, // __data_start
            size: page_align(0x100000), // __data_end - __data_start
            arch_mmu_flags: 0x3, // ARCH_MMU_FLAG_PERM_READ | ARCH_MMU_FLAG_PERM_WRITE
        },
        TempRegion {
            name: "kernel_bss",
            base: 0xffffffff80400000u64, // __bss_start
            size: page_align(0x100000), // _end - __bss_start
            arch_mmu_flags: 0x3, // ARCH_MMU_FLAG_PERM_READ | ARCH_MMU_FLAG_PERM_WRITE
        },
    ];

    // Reserve and protect kernel regions
    for region in &regions {
        if (region.base as usize) & PAGE_MASK != 0 {
            return Err(VmError::AlignmentError);
        }

        println!(
            "VM: reserving kernel region [{:#x}, {:#x}] flags {:#x} name '{}'",
            region.base,
            region.base + region.size as u64,
            region.arch_mmu_flags,
            region.name
        );

        // Reserve the space (TODO: implement proper reservation)
        // For now, just log the reservation
        // aspace.map(region.base, region.base, region.size / PAGE_SIZE, MemProt::READ | MemProt::WRITE)?;

        // Set protection flags
        // TODO: Implement protect_region
        // protect_region(aspace, region.base, region.arch_mmu_flags)?;
    }

    // Reserve the physmap region
    // TODO: Get PHYSMAP_BASE and PHYSMAP_SIZE from constants
    // aspace.reserve_space("physmap", PHYSMAP_SIZE, PHYSMAP_BASE)?;

    // Add random padding for KASLR
    // TODO: Implement random padding

    *state = VmInitState::FullyInitialized;
    Ok(())
}

/// Convert virtual address to physical address
///
/// # Arguments
///
/// * `ptr` - Virtual address to convert
///
/// # Returns
///
/// Physical address, or 0 if the address is not mapped
pub fn vaddr_to_paddr(_ptr: *const u8) -> u64 {
    // TODO: Implement vaddr_to_paddr
    // This requires proper address space lookup
    0
}

/// Set the kernel base physical address
///
/// # Arguments
///
/// * `pa` - Physical address of the kernel base
pub fn set_kernel_base_phys(pa: u64) {
    KERNEL_BASE_PHYS.store(pa, Ordering::Release);
}

/// Get the kernel base physical address
pub fn get_kernel_base_phys() -> u64 {
    KERNEL_BASE_PHYS.load(Ordering::Acquire)
}

/// Get the zero page physical address
pub fn get_zero_page_paddr() -> u64 {
    ZERO_PAGE_PADDR.load(Ordering::Acquire)
}

/// Get VM initialization state
pub fn get_vm_init_state() -> VmInitState {
    *VM_INIT_STATE.lock()
}

/// Check if VM is fully initialized
pub fn is_vm_initialized() -> bool {
    *VM_INIT_STATE.lock() == VmInitState::FullyInitialized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_align() {
        assert_eq!(page_align(0), 0);
        assert_eq!(page_align(1), PAGE_SIZE);
        assert_eq!(page_align(4095), PAGE_SIZE);
        assert_eq!(page_align(4096), 4096);
        assert_eq!(page_align(4097), PAGE_SIZE * 2);
        assert_eq!(page_align(8192), 8192);
    }

    #[test]
    fn test_page_rounddown() {
        assert_eq!(page_rounddown(0), 0);
        assert_eq!(page_rounddown(1), 0);
        assert_eq!(page_rounddown(4095), 0);
        assert_eq!(page_rounddown(4096), 4096);
        assert_eq!(page_rounddown(4097), 4096);
        assert_eq!(page_rounddown(8192), 8192);
    }

    #[test]
    fn test_vm_init_state() {
        assert_eq!(get_vm_init_state(), VmInitState::Uninitialized);
        assert!(!is_vm_initialized());
    }
}
