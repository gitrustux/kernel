// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V peripheral memory mapping
//!
//! This module provides functions for mapping device memory into
//! the kernel address space for accessing peripherals.


use crate::arch::riscv64::mmu;
use crate::rustux::types::*;

/// Base address for peripheral mappings in kernel virtual address space
pub const PERIPH_BASE: VAddr = 0xFFFF_FFFF_F000_0000;

/// Size of the peripheral mapping region (256GB)
pub const PERIPH_SIZE: usize = 256 * 1024 * 1024 * 1024;

/// Typical RISC-V peripheral physical addresses
pub mod periph_pa {
    /// UART0 (typically 16550 compatible)
    pub const UART0: PAddr = 0x1000_0000;

    /// CLINT (Core-Local Interrupt Controller)
    pub const CLINT: PAddr = 0x0200_0000;

    /// PLIC (Platform-Local Interrupt Controller)
    pub const PLIC: PAddr = 0x0C00_0000;

    /// PCIe ECAM space
    pub const PCIE_ECAM: PAddr = 0x3000_0000;

    /// PCIe MMIO space
    pub const PCIE_MMIO: PAddr = 0x4000_0000;
}

/// Peripheral mapping entry
#[repr(C)]
pub struct PeriphMap {
    pub name: &'static str,
    pub phys: PAddr,
    pub size: usize,
    pub virt: VAddr,
    pub flags: u64,
    pub mapped: bool,
}

/// Peripheral mapping table
static mut PERIPH_MAPS: [PeriphMap; 16] = [
    PeriphMap {
        name: "uart0",
        phys: periph_pa::UART0,
        size: 0x1000,
        virt: 0,
        flags: mmu::pte_flags::VALID | mmu::pte_flags::READ | mmu::pte_flags::WRITE,
        mapped: false,
    },
    PeriphMap {
        name: "clint",
        phys: periph_pa::CLINT,
        size: 0x10000,
        virt: 0,
        flags: mmu::pte_flags::VALID | mmu::pte_flags::READ | mmu::pte_flags::WRITE,
        mapped: false,
    },
    PeriphMap {
        name: "plic",
        phys: periph_pa::PLIC,
        size: 0x400000,
        virt: 0,
        flags: mmu::pte_flags::VALID | mmu::pte_flags::READ | mmu::pte_flags::WRITE,
        mapped: false,
    },
    PeriphMap {
        name: "",
        phys: 0,
        size: 0,
        virt: 0,
        flags: 0,
        mapped: false,
    },
]; // Remaining entries are empty

/// Next available virtual address for peripheral mapping
static mut PERIPH_NEXT_VADDR: VAddr = PERIPH_BASE;

/// Initialize the peripheral mapping subsystem
pub fn riscv_periphmap_init() {
    unsafe {
        PERIPH_NEXT_VADDR = PERIPH_BASE;
    }
}

/// Map a peripheral into kernel virtual address space
///
/// # Arguments
///
/// * `phys` - Physical address of the peripheral
/// * `size` - Size of the mapping in bytes
/// * `flags` - PTE flags for the mapping
///
/// # Returns
///
/// Virtual address where the peripheral was mapped, or 0 on error
///
/// # Safety
///
/// Must be called during kernel initialization or from a context
/// where it's safe to modify page tables
pub unsafe fn riscv_periph_map(phys: PAddr, size: usize, flags: u64) -> VAddr {
    // Align size to page boundary
    let size_aligned = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

    // Allocate virtual address space
    let vaddr = PERIPH_NEXT_VADDR;
    PERIPH_NEXT_VADDR += size_aligned;

    // Check if we've overflowed the peripheral region
    if PERIPH_NEXT_VADDR > PERIPH_BASE + PERIPH_SIZE {
        return 0; // Out of peripheral address space
    }

    // TODO: Actually create the page table mappings
    // For now, just return the virtual address
    // In a full implementation, this would:
    // 1. Walk/allocate page tables
    // 2. Set up PTEs with device flags (uncached, etc.)
    // 3. Flush TLB

    vaddr
}

/// Unmap a peripheral mapping
///
/// # Arguments
///
/// * `vaddr` - Virtual address to unmap
/// * `size` - Size of the mapping
///
/// # Safety
///
/// Must be called from a context where it's safe to modify page tables
pub unsafe fn riscv_periph_unmap(vaddr: VAddr, size: usize) {
    // TODO: Unmap page table entries
    // In a full implementation, this would:
    // 1. Walk page tables
    // 2. Clear PTEs
    // 3. Flush TLB
    // 4. Free page table pages if possible

    let _ = vaddr;
    let _ = size;
}

/// Get or create a peripheral mapping by name
///
/// # Arguments
///
/// * `name` - Name of the peripheral to map
///
/// # Returns
///
/// Virtual address where the peripheral is mapped, or 0 if not found
pub unsafe fn riscv_periph_get_by_name(name: &str) -> VAddr {
    for map in PERIPH_MAPS.iter() {
        if map.name.is_empty() {
            break;
        }

        if map.name == name {
            if !map.mapped {
                // Map it now
                let vaddr = riscv_periph_map(map.phys, map.size, map.flags);

                // Update the mapping entry
                let map_ptr = map as *const PeriphMap as *mut PeriphMap;
                (*map_ptr).virt = vaddr;
                (*map_ptr).mapped = true;

                return vaddr;
            } else {
                return map.virt;
            }
        }
    }

    0 // Not found
}

/// Map UART for kernel console
///
/// # Returns
///
/// Virtual address of UART, or 0 on error
pub fn riscv_map_uart() -> VAddr {
    unsafe { riscv_periph_get_by_name("uart0") }
}

/// Map CLINT (timer and IPI)
///
/// # Returns
///
/// Virtual address of CLINT, or 0 on error
pub fn riscv_map_clint() -> VAddr {
    unsafe { riscv_periph_get_by_name("clint") }
}

/// Map PLIC (external interrupts)
///
/// # Returns
///
/// Virtual address of PLIC, or 0 on error
pub fn riscv_map_plic() -> VAddr {
    unsafe { riscv_periph_get_by_name("plic") }
}

/// Create a memory-mapped I/O access window
///
/// This is used by device drivers to access device registers.
///
/// # Arguments
///
/// * `phys` - Physical address of the device registers
/// * `size` - Size of the register space
///
/// # Returns
///
/// Virtual address for accessing the device, or 0 on error
pub fn riscv_mmiomap(phys: PAddr, size: usize) -> VAddr {
    // Flags for device memory: valid, readable, writable, not executable
    let flags = mmu::pte_flags::VALID
        | mmu::pte_flags::READ
        | mmu::pte_flags::WRITE
        | mmu::pte_flags::GLOBAL;

    unsafe { riscv_periph_map(phys, size, flags) }
}

/// Release a memory-mapped I/O window
///
/// # Arguments
///
/// * `vaddr` - Virtual address returned by riscv_mmiomap
/// * `size` - Size of the mapping
pub fn riscv_mmiounmap(vaddr: VAddr, size: usize) {
    unsafe { riscv_periph_unmap(vaddr, size) }
}

/// Physical to virtual address translation for peripherals
///
/// # Arguments
///
/// * `phys` - Physical address
///
/// # Returns
///
/// Virtual address if in peripheral range, otherwise 0
pub fn riscv_periph_phys_to_virt(phys: PAddr) -> VAddr {
    // Check if this is a known peripheral
    for map in unsafe { PERIPH_MAPS.iter() } {
        if map.name.is_empty() {
            break;
        }

        if phys >= map.phys && phys < (map.phys + map.size) {
            let offset = phys - map.phys;
            if map.mapped {
                return map.virt + offset;
            } else {
                // Map it first
                let vaddr = unsafe { riscv_periph_get_by_name(map.name) };
                if vaddr != 0 {
                    return vaddr + offset;
                }
            }
        }
    }

    0 // Not found
}

/// Assert PeriphMap is the expected size
const _: () = assert!(core::mem::size_of::<PeriphMap>() == 40);
