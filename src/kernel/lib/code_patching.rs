// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Code Patching
//!
//! This module provides runtime code patching functionality for the kernel.
//! It allows applying patches to code sections during early boot.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::rustux::types::*;

/// Code patch information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CodePatchInfo {
    /// Destination address to patch
    pub dest_addr: *mut u8,
    /// Size of the region to patch
    pub dest_size: usize,
    /// Apply function
    pub apply_func: Option<extern "C" fn(&CodePatchInfo)>,
}

/// Code patch table (linker section)
extern "C" {
    /// Start of code patch table (linker symbol)
    #[link_name = "__start_code_patch_table"]
    static CODE_PATCH_TABLE_START: CodePatchInfo;

    /// End of code patch table (linker symbol)
    #[link_name = "__stop_code_patch_table"]
    static CODE_PATCH_TABLE_END: CodePatchInfo;
}

/// Code patching state
static CODE_PATCHING_APPLIED: AtomicBool = AtomicBool::new(false);

/// Get the code patch table as a slice
///
/// # Safety
///
/// This function assumes the linker symbols are properly defined
/// and form a valid array.
unsafe fn get_code_patch_table() -> &'static [CodePatchInfo] {
    let start = &CODE_PATCH_TABLE_START as *const _ as usize;
    let end = &CODE_PATCH_TABLE_END as *const _ as usize;

    if end <= start {
        return &[];
    }

    let count = (end - start) / core::mem::size_of::<CodePatchInfo>();
    core::slice::from_raw_parts(&CODE_PATCH_TABLE_START, count)
}

/// Apply a code patch
///
/// # Arguments
///
/// * `patch` - Code patch information
///
/// # Safety
///
/// The caller must ensure that the patch destination is valid
/// and writable.
pub unsafe fn apply_code_patch(patch: &CodePatchInfo) {
    if let Some(func) = patch.apply_func {
        println!(
            "CodePatch: Applying patch at {:#x}, size: {}",
            patch.dest_addr as usize, patch.dest_size
        );

        // Apply the patch
        func(patch);

        // Sync the cache range to ensure instructions are visible
        arch_sync_cache_range(patch.dest_addr as usize, patch.dest_size);
    }
}

/// Apply all startup code patches
///
/// This function is called during early boot to apply all code patches
/// that were registered in the code patch table.
///
/// # Safety
///
/// This function should only be called once during early boot initialization.
pub fn apply_startup_code_patches() {
    if CODE_PATCHING_APPLIED.swap(true, Ordering::AcqRel) {
        println!("CodePatch: Already applied, skipping");
        return;
    }

    println!("CodePatch: Applying startup code patches");

    // SAFETY: The code patch table is defined by the linker
    // and is valid during early boot.
    unsafe {
        let table = get_code_patch_table();
        println!("CodePatch: Found {} patches", table.len());

        for patch in table {
            apply_code_patch(patch);
        }
    }

    println!("CodePatch: Startup patches applied");
}

/// Architecture-specific cache synchronization
///
/// # Arguments
///
/// * `addr` - Start address of the range to sync
/// * `size` - Size of the range to sync
///
/// # Safety
///
/// The caller must ensure the address range is valid.
#[cfg(target_arch = "x86_64")]
unsafe fn arch_sync_cache_range(_addr: usize, _size: usize) {
    // x86_64 has coherent instruction cache, no need to do anything
    // However, we still need to ensure writes are visible
    core::arch::x86_64::_mfence();
}

#[cfg(target_arch = "aarch64")]
unsafe fn arch_sync_cache_range(addr: usize, size: usize) {
    // ARM requires explicit cache invalidation
    // Flush data cache to point of unification
    let mut current = addr & !(32 - 1);
    let end = (addr + size + 31) & !(32 - 1);

    while current < end {
        core::arch::asm!(
            "dc cvau, {0}",
            in(reg) current as u64,
            options(nostack, preserves_flags)
        );
        current += 32;
    }

    // Invalidate instruction cache to point of unification
    let mut current = addr & !(32 - 1);
    let end = (addr + size + 31) & !(32 - 1);

    while current < end {
        core::arch::asm!(
            "ic ivau, {0}",
            in(reg) current as u64,
            options(nostack, preserves_flags)
        );
        current += 32;
    }

    // Data synchronization barrier
    core::arch::asm!("dsb ish", options(nostack, preserves_flags));

    // Instruction synchronization barrier
    core::arch::asm!("isb", options(nostack, preserves_flags));
}

#[cfg(target_arch = "riscv64")]
unsafe fn arch_sync_cache_range(addr: usize, size: usize) {
    // RISC-V requires fence.i instruction
    let mut current = addr & !(64 - 1);
    let end = (addr + size + 63) & !(64 - 1);

    while current < end {
        core::arch::asm!(
            "fence.i",
            options(nostack, preserves_flags)
        );
        current += 64;
    }
}

/// Check if code patches have been applied
pub fn is_code_patching_applied() -> bool {
    CODE_PATCHING_APPLIED.load(Ordering::Acquire)
}

/// Initialize code patching system
///
/// This function applies startup code patches during early boot.
pub fn init() {
    apply_startup_code_patches();
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn test_apply_func(_patch: &CodePatchInfo) {
        // Test patch function
    }

    #[test]
    fn test_code_patch_info_size() {
        assert_eq!(core::mem::size_of::<CodePatchInfo>(), 24);
    }

    #[test]
    fn test_not_applied_initially() {
        // After this test, code patching will be marked as applied
        // but that's OK for tests
        assert!(!is_code_patching_applied());
    }
}
