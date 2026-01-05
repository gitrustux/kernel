// Copyright 2025 The Rustux Authors
// Copyright (c) 2009 Corey Tabaka
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Cache operations
//!
//! This module provides cache manipulation functions for x86 processors.

#![no_std]

use crate::kernel::arch::amd64::feature;
use crate::rustux::types::*;

/// X86 feature flags
const X86_FEATURE_CLFLUSH: u64 = 1 << 19;
const X86_FEATURE_CLFLUSHOPT: u64 = 1 << 23;

/// Get data cache line size
///
/// # Returns
///
/// The size of the data cache line in bytes
pub fn arch_dcache_line_size() -> usize {
    unsafe { x86_get_clflush_line_size() as usize }
}

/// Get instruction cache line size
///
/// # Returns
///
/// The size of the instruction cache line in bytes
pub fn arch_icache_line_size() -> usize {
    unsafe { x86_get_clflush_line_size() as usize }
}

/// Synchronize the cache for the given range
///
/// Uses cpuid as a serializing instruction to ensure visibility
/// of instruction stream modifications (self/cross-modifying code).
///
/// # Arguments
///
/// * `start` - Starting virtual address
/// * `len` - Length of the range in bytes
pub fn arch_sync_cache_range(start: VAddr, len: usize) {
    let _ = start;
    let _ = len;

    // Invoke cpuid to act as a serializing instruction
    // This ensures we see modifications to the instruction stream
    // See Intel Volume 3, 8.1.3 "Handling Self- and Cross-Modifying Code"
    let mut v: u32;
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            let (_a, _b, _c, d): (u32, u32, u32, u32) = core::arch::x86_64::__cpuid(0);
            v = d;
            let _ = v; // Prevent unused warning
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            // For other architectures, use a compiler barrier
            core::hint::black_box(());
            v = 0;
        }
    }
}

/// Invalidate the cache for the given range
///
/// # Arguments
///
/// * `start` - Starting virtual address
/// * `len` - Length of the range in bytes
pub fn arch_invalidate_cache_range(_start: VAddr, _len: usize) {
    // No-op on x86 for instruction cache invalidation
}

/// Clean the cache for the given range
///
/// # Arguments
///
/// * `start` - Starting virtual address
/// * `len` - Length of the range in bytes
pub fn arch_clean_cache_range(start: VAddr, len: usize) {
    // TODO: consider wiring up clwb if present
    arch_clean_invalidate_cache_range(start, len);
}

/// Clean and invalidate the cache for the given range
///
/// # Arguments
///
/// * `start` - Starting virtual address
/// * `len` - Length of the range in bytes
pub fn arch_clean_invalidate_cache_range(start: VAddr, len: usize) {
    // Check if CLFLUSH is available
    if !unsafe { feature::x86_feature_test(X86_FEATURE_CLFLUSH) } {
        unsafe {
            // Fall back to WBINVD (write-back and invalidate)
            core::arch::asm!("wbinvd");
        }
        return;
    }

    // clflush/clflushopt is present
    let clsize = unsafe { x86_get_clflush_line_size() } as usize;
    let end = start + len;
    let mut ptr = start & !(clsize - 1); // ROUNDDOWN(start, clsize)

    // Check if CLFLUSHOPT is available
    let use_opt = unsafe { feature::x86_feature_test(X86_FEATURE_CLFLUSHOPT) };

    while ptr < end {
        unsafe {
            if use_opt {
                core::arch::asm!("clflushopt [{ptr}]", ptr = in(reg) ptr);
            } else {
                core::arch::asm!("clflush [{ptr}]", ptr = in(reg) ptr);
            }
        }
        ptr += clsize;
    }

    // Memory fence to ensure cache operations complete
    unsafe {
        core::arch::asm!("mfence");
    }
}

// External functions
extern "C" {
    /// Get the cache line size for CLFLUSH
    fn x86_get_clflush_line_size() -> u32;
}
