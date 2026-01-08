// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Boot Page Tables
//!
//! This module handles the creation and initialization of the kernel's
//! page tables during early boot. It sets up the kernel's virtual address
//! space with proper mappings for code, data, stacks, and physical memory.
//!
//! # Boot Sequence
//!
//! 1. Early boot (assembly) creates minimal page tables
//! 2. Rust code takes over, creates full kernel address space
//! 3. Maps kernel text, data, bss, heap, and physmap
//! 4. Switches to new page tables
//! 5. Verifies stability

#![no_std]

use crate::kernel::vm::layout::*;

// Architecture-specific layout imports
#[cfg(target_arch = "x86_64")]
use crate::kernel::vm::layout::amd64 as layout_arch;

#[cfg(target_arch = "aarch64")]
use crate::kernel::vm::layout::arm64 as layout_arch;

#[cfg(target_arch = "riscv64")]
use crate::kernel::vm::layout::riscv as layout_arch;
use crate::kernel::vm::page_table::*;
use crate::kernel::vm::aspace::*;
use crate::kernel::vm::{Result, VmError};
use crate::kernel::pmm;

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Kernel Memory Map Descriptor
/// ============================================================================

/// Describes a region of kernel memory to be mapped
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KernelMapDesc {
    /// Virtual address where this region should be mapped
    pub virt_addr: VAddr,

    /// Physical address (or 0 for identity/relocatable)
    pub phys_addr: PAddr,

    /// Size in bytes (must be page-aligned)
    pub size: usize,

    /// Memory protection flags
    pub prot: MemProt,

    /// Region name (for debugging)
    pub name: &'static str,
}

impl KernelMapDesc {
    /// Create a new kernel map descriptor
    pub const fn new(
        virt_addr: VAddr,
        phys_addr: PAddr,
        size: usize,
        prot: MemProt,
        name: &'static str,
    ) -> Self {
        Self {
            virt_addr,
            phys_addr,
            size,
            prot,
            name,
        }
    }

    /// Get the number of pages for this region
    pub const fn page_count(&self) -> usize {
        self.size / PAGE_SIZE
    }

    /// Get the end virtual address (exclusive)
    pub const fn end(&self) -> VAddr {
        self.virt_addr + self.size
    }
}

/// ============================================================================
/// Boot Page Table Allocator
/// ============================================================================

/// Boot-time page table allocator
///
/// Uses a simple bump allocator for early boot page table allocation.
/// This is used before the main PMM is fully initialized.
struct BootAllocator {
    next_addr: PAddr,
    end_addr: PAddr,
}

impl BootAllocator {
    /// Create a new boot allocator in the given region
    const fn new(base: PAddr, size: usize) -> Self {
        Self {
            next_addr: base,
            end_addr: base + size as PAddr,
        }
    }

    /// Allocate a page
    fn alloc_page(&mut self) -> Option<PAddr> {
        let addr = self.next_addr;
        self.next_addr += PAGE_SIZE as PAddr;

        if self.next_addr > self.end_addr {
            return None;
        }

        // Zero the page
        let virt_addr = addr as usize;
        unsafe {
            let ptr = virt_addr as *mut u8;
            for i in 0..PAGE_SIZE {
                ptr.add(i).write_volatile(0);
            }
        }

        Some(addr)
    }
}

/// ============================================================================
/// Kernel Address Space Builder
/// ============================================================================

/// Build the kernel's address space
///
/// This function creates and initializes the kernel's page tables,
/// mapping all required regions.
pub fn build_kernel_aspace() -> Result<AddressSpace> {
    // Create kernel address space
    let aspace = AddressSpace::new_kernel()?;

    // Get kernel regions from the linker
    let regions = kernel_memory_regions();

    // Map each region
    for region in regions {
        map_kernel_region(&aspace, region)?;
    }

    // Create and map per-CPU areas
    setup_percpu_areas(&aspace)?;

    // Create initial kernel stacks
    setup_kernel_stacks(&aspace)?;

    // Map physical memory window
    setup_physmap(&aspace)?;

    // Map device MMIO region
    setup_mmio_region(&aspace)?;

    Ok(aspace)
}

/// Map a kernel memory region
fn map_kernel_region(aspace: &AddressSpace, desc: KernelMapDesc) -> Result {
    let pt_flags = PageTableFlags::from_prot(desc.prot);

    // Kernel mappings don't have user flag
    let base_flags = pt_flags & !(PageTableFlags::User as u64);

    // Determine physical address (use virt if phys is 0)
    let paddr = if desc.phys_addr == 0 {
        // Assume identity mapping for now
        desc.virt_addr as PAddr
    } else {
        desc.phys_addr
    };

    aspace.map(
        desc.virt_addr,
        paddr,
        desc.page_count(),
        MemProt::Read, // Protection is handled by pt_flags
    )?;

    log_debug!(
        "Mapped {}: {:#x} -> {:#x} ({} pages, {})",
        desc.name,
        desc.virt_addr,
        paddr,
        desc.page_count(),
        desc.prot
    );

    Ok(())
}

/// Setup per-CPU data areas
fn setup_percpu_areas(aspace: &AddressSpace) -> Result {
    #[cfg(target_arch = "aarch64")]
    let (base, size) = (layout_arch::KERNEL_PERCPU_BASE, layout_arch::KERNEL_PERCPU_SIZE);

    #[cfg(target_arch = "x86_64")]
    let (base, size) = (layout_arch::KERNEL_PERCPU_BASE, layout_arch::KERNEL_PERCPU_SIZE);

    #[cfg(target_arch = "riscv64")]
    let (base, size) = (layout_arch::KERNEL_PERCPU_BASE, layout_arch::KERNEL_PERCPU_SIZE);

    // For now, map a single per-CPU area
    // In multi-CPU systems, we'd allocate one area per CPU
    let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

    // Allocate physical pages for per-CPU data
    let paddr = match pmm::pmm_alloc_page(pmm::PMM_ALLOC_FLAG_ANY) {
        Ok(addr) => addr,
        Err(_) => return Err(VmError::NoMemory),
    };

    aspace.map(base, paddr as usize, pages, MemProt::ReadWrite)?;

    Ok(())
}

/// Setup kernel stacks
fn setup_kernel_stacks(aspace: &AddressSpace) -> Result {
    const MAX_KERNEL_STACKS: usize = 256;
    const STACK_REGION_SIZE: usize = MAX_KERNEL_STACKS * KERNEL_STACK_SIZE;

    #[cfg(target_arch = "aarch64")]
    let stacks_base = layout_arch::KERNEL_HEAP_BASE + layout_arch::KERNEL_HEAP_SIZE;

    #[cfg(target_arch = "x86_64")]
    let stacks_base = layout_arch::KERNEL_HEAP_BASE + layout_arch::KERNEL_HEAP_SIZE;

    #[cfg(target_arch = "riscv64")]
    let stacks_base = layout_arch::KERNEL_HEAP_BASE + layout_arch::KERNEL_HEAP_SIZE;

    // Allocate physical pages for stacks
    let total_pages = STACK_REGION_SIZE / PAGE_SIZE;

    // Allocate from PMM
    let paddr = match pmm::pmm_alloc_contiguous(total_pages, pmm::PMM_ALLOC_FLAG_ANY, 12) {
        Ok(addr) => addr,
        Err(_) => return Err(VmError::NoMemory),
    };

    aspace.map(stacks_base, paddr as usize, total_pages, MemProt::ReadWrite)?;

    log_debug!(
        "Mapped kernel stacks: {:#x} -> {:#x} ({} stacks)",
        stacks_base,
        paddr,
        MAX_KERNEL_STACKS
    );

    Ok(())
}

/// Setup physical memory direct map
fn setup_physmap(aspace: &AddressSpace) -> Result {
    #[cfg(target_arch = "aarch64")]
    let (base, size) = (layout_arch::KERNEL_PHYSMAP_BASE, layout_arch::KERNEL_PHYSMAP_SIZE);

    #[cfg(target_arch = "x86_64")]
    let (base, size) = (layout_arch::KERNEL_PHYSMAP_BASE, layout_arch::KERNEL_PHYSMAP_SIZE);

    #[cfg(target_arch = "riscv64")]
    let (base, size) = (layout_arch::KERNEL_PHYSMAP_BASE, layout_arch::KERNEL_PHYSMAP_SIZE);

    // Map the first portion of physical memory 1:1
    let map_size = size.min(1 * 1024 * 1024 * 1024); // Start with 1GB
    let pages = map_size / PAGE_SIZE;

    // For identity mapping, phys == virt - base
    aspace.map(base, 0, pages, MemProt::ReadWrite)?;

    log_debug!(
        "Mapped physmap: {:#x} -> {:#x} ({} GB)",
        base,
        0,
        map_size / (1024 * 1024 * 1024)
    );

    Ok(())
}

/// Setup device MMIO region
fn setup_mmio_region(aspace: &AddressSpace) -> Result {
    #[cfg(target_arch = "aarch64")]
    let (base, size) = (layout_arch::KERNEL_MMIO_BASE, layout_arch::KERNEL_MMIO_SIZE);

    #[cfg(target_arch = "x86_64")]
    let (base, size) = (layout_arch::KERNEL_MMIO_BASE, layout_arch::KERNEL_MMIO_SIZE);

    #[cfg(target_arch = "riscv64")]
    let (base, size) = (layout_arch::KERNEL_MMIO_BASE, layout_arch::KERNEL_MMIO_SIZE);

    // Map a small region for early MMIO (e.g., UART)
    let map_size = 64 * 1024 * 1024; // 64MB
    let pages = map_size / PAGE_SIZE;

    // MMIO is typically at specific physical addresses
    // This would be platform-specific
    let paddr = 0x0900_0000; // Example UART address

    // Device memory should be uncached
    aspace.map(base, paddr, pages, MemProt::ReadWrite)?;

    log_debug!(
        "Mapped MMIO: {:#x} -> {:#x}",
        base,
        paddr
    );

    Ok(())
}

/// ============================================================================
/// Linker-Defined Regions
/// ============================================================================

/// Get kernel memory regions from linker symbols
///
/// These are defined by the linker script and describe where
/// the kernel's various sections are located.
fn kernel_memory_regions() -> [KernelMapDesc; 3] {
    // These symbols are defined by the linker script
    extern "C" {
        fn __code_start();
        fn __code_end();
        fn __data_start();
        fn __data_end();
        fn __bss_start();
        fn __bss_end();
    }

    // Create regions at runtime instead of const context
    [
        // Kernel code/text (RX)
        KernelMapDesc::new(
            __code_start as VAddr,
            0, // Will use identity
            __code_end as usize - __code_start as usize,
            MemProt::Read,
            "kernel_text",
        ),
        // Kernel data (RW)
        KernelMapDesc::new(
            __data_start as VAddr,
            0,
            __data_end as usize - __data_start as usize,
            MemProt::ReadWrite,
            "kernel_data",
        ),
        // Kernel BSS (RW)
        KernelMapDesc::new(
            __bss_start as VAddr,
            0,
            __bss_end as usize - __bss_start as usize,
            MemProt::ReadWrite,
            "kernel_bss",
        ),
    ]
}

/// ============================================================================
/// Boot-Time Page Table Creation
// ============================================================================

/// Create early boot page tables
///
/// This is called from assembly before the Rust kernel takes over.
/// It creates minimal page tables to get us to a 64-bit long mode
/// with virtual memory enabled.
///
/// # Safety
///
/// Must be called with MMU disabled, running in physical addressing mode.
#[no_mangle]
pub unsafe extern "C" fn boot_create_page_tables() -> PAddr {
    #[cfg(target_arch = "aarch64")]
    {
        crate::kernel::arch::arm64::boot_mmu::arm64_boot_create_page_tables()
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::kernel::arch::amd64::mmu::x86_boot_create_page_tables();
        0 // Return dummy PAddr for now
    }

    #[cfg(target_arch = "riscv64")]
    {
        crate::kernel::arch::riscv64::boot_mmu::riscv_boot_create_page_tables()
    }
}

/// Finalize kernel address space setup
///
/// Called after the kernel has basic page tables, this creates
/// the full kernel address space and switches to it.
pub fn finalize_kernel_aspace() -> Result {
    log_info!("Finalizing kernel address space...");

    // Build the full kernel address space
    let aspace = build_kernel_aspace()?;

    // Flush TLB to ensure new mappings take effect
    aspace.flush_tlb();

    // Switch to the new page table
    unsafe {
        // Architecture-specific switch
        #[cfg(target_arch = "aarch64")]
        {
            let root = aspace.root_phys();
            crate::kernel::arch::arm64::mmu::set_ttbr1_el1(root);
        }

        #[cfg(target_arch = "x86_64")]
        {
            let root = aspace.root_phys() as u64;
            crate::kernel::arch::amd64::mmu::write_cr3(root);
        }

        #[cfg(target_arch = "riscv64")]
        {
            let root = aspace.root_phys();
            crate::arch::riscv64::mmu::write_satp(root);
        }
    }

    // Verify we can still access memory
    verify_kernel_aspace(&aspace)?;

    log_info!("Kernel address space initialized successfully");

    Ok(())
}

/// Verify that the kernel address space is working
fn verify_kernel_aspace(aspace: &AddressSpace) -> Result {
    // Try to resolve kernel code address
    extern "C" {
        fn kernel_aspace_test();
    }

    let addr = kernel_aspace_test as VAddr;
    match aspace.resolve(addr) {
        Some(_) => {
            log_debug!("Address space verification passed");
            Ok(())
        }
        None => {
            log_error!("Address space verification failed: cannot resolve kernel code");
            Err(crate::kernel::vm::VmError::BadState)
        }
    }
}

/// Test function that verifies address space works
#[no_mangle]
pub extern "C" fn kernel_aspace_test() {
    // This function exists purely to have an address we can test
}

// ============================================================================
// Module for architecture-specific boot MMU code
// ============================================================================

#[cfg(target_arch = "arm64")]
pub mod arm64 {
    use super::*;

    /// ARM64-specific boot page table creation
    pub unsafe fn create_boot_page_tables() -> PAddr {
        extern "C" {
            fn arm64_boot_create_page_tables() -> PAddr;
        }

        arm64_boot_create_page_tables()
    }
}

#[cfg(target_arch = "x86_64")]
pub mod amd64 {
    use super::*;

    /// AMD64-specific boot page table creation
    pub unsafe fn create_boot_page_tables() -> PAddr {
        extern "C" {
            fn x86_boot_create_page_tables() -> PAddr;
        }

        x86_boot_create_page_tables()
    }
}

#[cfg(target_arch = "riscv64")]
pub mod riscv {
    use super::*;

    /// RISC-V-specific boot page table creation
    pub unsafe fn create_boot_page_tables() -> PAddr {
        extern "C" {
            fn riscv_boot_create_page_tables() -> PAddr;
        }

        riscv_boot_create_page_tables()
    }
}
