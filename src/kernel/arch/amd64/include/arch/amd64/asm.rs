// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Assembly-related definitions for x86 architecture
//!
//! This module contains constants and macros related to physical address
//! calculations used in assembly code throughout the kernel.

use core::ops::{Add, Sub};

/// Physical load address of the kernel
pub const PHYS_LOAD_ADDRESS: usize = crate::arch::KERNEL_LOAD_OFFSET as usize;

/// Difference between kernel virtual address and physical address
pub const PHYS_ADDR_DELTA: usize = crate::arch::KERNEL_BASE as usize - PHYS_LOAD_ADDRESS;

/// Convert a virtual address to a physical address
///
/// # Arguments
///
/// * `vaddr` - Virtual address to convert
///
/// # Returns
///
/// The corresponding physical address
#[inline]
pub fn phys<T>(vaddr: T) -> T
where
    T: Sub<usize, Output = T> + Copy
{
    vaddr - PHYS_ADDR_DELTA
}

/// Convert a physical address to a virtual address
///
/// # Arguments
///
/// * `paddr` - Physical address to convert
///
/// # Returns
///
/// The corresponding virtual address
#[inline]
pub fn virt<T>(paddr: T) -> T
where
    T: Add<usize, Output = T> + Copy
{
    paddr + PHYS_ADDR_DELTA
}

/// Includes from assembly code should use this macro
///
/// This makes the assembly code compatible with Rust's compilation model
#[macro_export]
macro_rules! asm_include {
    ($file:expr) => {
        core::include_str!(concat!(env!("RUSTUX_ASM_DIR"), "/", $file));
    };
}