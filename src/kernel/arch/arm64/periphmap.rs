// Copyright 2025 Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64::mmu::*;
use crate::vm::vm::*;
use crate::vm::*;

const PERIPH_RANGE_MAX: usize = 4;

struct PeriphRange {
    base_phys: u64,
    base_virt: u64,
    length: u64,
}

impl PeriphRange {
    const fn new() -> Self {
        PeriphRange {
            base_phys: 0,
            base_virt: 0,
            length: 0,
        }
    }
}

// Static array for peripheral memory mappings
static mut PERIPH_RANGES: [PeriphRange; PERIPH_RANGE_MAX] = [PeriphRange::new(); PERIPH_RANGE_MAX];

/// Add a new peripheral memory range mapping
///
/// # Arguments
///
/// * `base_phys` - Physical base address (must be page aligned)
/// * `length` - Length of the range (must be page aligned)
///
/// # Returns
///
/// * `RX_OK` on success
/// * Error code otherwise
pub fn add_periph_range(base_phys: paddr_t, length: size_t) -> rx_status_t {
    // peripheral ranges are allocated below the kernel image.
    let mut base_virt = unsafe { __code_start as u64 };

    debug_assert!(is_page_aligned(base_phys), "base_phys must be page aligned");
    debug_assert!(is_page_aligned(length as u64), "length must be page aligned");

    unsafe {
        for range in &mut PERIPH_RANGES {
            if range.length == 0 {
                base_virt -= length as u64;
                let status = arm64_boot_map_v(base_virt, base_phys, length, MMU_INITIAL_MAP_DEVICE);
                if status == RX_OK {
                    range.base_phys = base_phys;
                    range.base_virt = base_virt;
                    range.length = length as u64;
                }
                return status;
            } else {
                base_virt -= range.length;
            }
        }
    }
    
    RX_ERR_OUT_OF_RANGE
}

/// Reserve all peripheral ranges in the kernel address space
pub fn reserve_periph_ranges() {
    unsafe {
        for range in &PERIPH_RANGES {
            if range.length == 0 {
                break;
            }
            VmAspace::kernel_aspace().reserve_space(
                "periph", 
                range.length as usize, 
                range.base_virt
            );
        }
    }
}

/// Converts a physical address to its corresponding virtual address
/// in a peripheral memory mapping
///
/// # Arguments
///
/// * `paddr` - Physical address to convert
///
/// # Returns
///
/// * Virtual address if the physical address falls within a mapped peripheral range
/// * 0 otherwise
pub fn periph_paddr_to_vaddr(paddr: paddr_t) -> vaddr_t {
    unsafe {
        for range in &PERIPH_RANGES {
            if range.length == 0 {
                break;
            } else if paddr >= range.base_phys {
                let offset = paddr - range.base_phys;
                if offset < range.length {
                    return range.base_virt + offset;
                }
            }
        }
    }
    0
}

// External references
extern "C" {
    // Start of code section in kernel image
    static __code_start: u8;
}

// Type definitions
type paddr_t = u64;
type vaddr_t = u64;
type size_t = usize;
type rx_status_t = i32;

// Constants
const RX_OK: rx_status_t = 0;
const RX_ERR_OUT_OF_RANGE: rx_status_t = -33;

// Helper functions
fn is_page_aligned(addr: u64) -> bool {
    (addr & (PAGE_SIZE - 1)) == 0
}

const PAGE_SIZE: u64 = 4096;

// Forward declarations for external functions
extern "C" {
    fn arm64_boot_map_v(vaddr: vaddr_t, paddr: paddr_t, length: size_t, flags: u32) -> rx_status_t;
}

// MMU flags (would normally be defined in mmu.rs)
const MMU_INITIAL_MAP_DEVICE: u32 = 0; // This value should be defined in mmu.rs