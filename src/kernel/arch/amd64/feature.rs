// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 CPU Feature Detection
//!
//! This module provides CPU feature detection using CPUID.

#![no_std]

use core::arch::x86_64::{CpuidResult, __cpuid_count};

// Inline assembly cpuid wrapper for compatibility
#[inline]
unsafe fn cpuid_leaf(leaf: u32) -> CpuidResult {
    // Use the intrinsic which handles register constraints properly
    __cpuid_count(leaf, 0)
}

/// CPU feature flags
pub const X86_FEATURE_SMAP: u64 = 1 << 20;  // Supervisor Mode Access Prevention
pub const X86_FEATURE_SMEP: u64 = 1 << 7;   // Supervisor Mode Execution Protection
pub const X86_FEATURE_FSGSBASE: u64 = 1 << 0;  // FS/GS base instructions
pub const X86_FEATURE_SSE4_2: u64 = 1 << 20;  // SSE4.2
pub const X86_FEATURE_AVX: u64 = 1 << 28;    // AVX
pub const X86_FEATURE_AVX2: u64 = 1 << 5;    // AVX2 (in ebx)

/// CPU model information
#[derive(Debug, Clone, Copy)]
pub struct CpuModel {
    pub processor_type: u8,
    pub family: u8,
    pub model: u8,
    pub stepping: u8,
    pub display_family: u8,
    pub display_model: u8,
}

/// Get the CPU model information
pub fn x86_get_model() -> CpuModel {
    // TODO: Implement proper CPUID-based feature detection
    CpuModel {
        processor_type: 0,
        family: 6,
        model: 0,
        stepping: 1,
        display_family: 6,
        display_model: 0,
    }
}

/// Debug: print CPU features
pub fn x86_feature_debug() {
    // TODO: Implement feature detection and printing
}

/// Get all CPU features as a bitmask
pub unsafe fn x86_feature_get_all() -> u64 {
    // TODO: Implement CPU feature detection
    0
}

/// Test if a CPU feature is available
///
/// # Arguments
///
/// * `leaf` - CPUID leaf to query
/// * `subleaf` - CPUID subleaf to query
/// * `reg` - Register to check (0=EAX, 1=EBX, 2=ECX, 3=EDX)
/// * `bit` - Bit to test
///
/// # Returns
///
/// true if the feature bit is set
pub fn x86_feature_test(leaf: u32, subleaf: u32, reg: u32, bit: u32) -> bool {
    unsafe {
        let result = cpuid_with_subleaf(leaf, subleaf);
        let value = match reg {
            0 => result.eax,
            1 => result.ebx,
            2 => result.ecx,
            3 => result.edx,
            _ => return false,
        };
        (value & (1 << bit)) != 0
    }
}

/// Wrapper for CPUID with subleaf
///
/// # Arguments
///
/// * `leaf` - CPUID leaf
/// * `subleaf` - CPUID subleaf
///
/// # Returns
///
/// CPUID result
#[inline]
pub unsafe fn cpuid_with_subleaf(leaf: u32, subleaf: u32) -> CpuidResult {
    // In actual x86_64, this would be:
    // core::arch::x86_64::__cpuid_count(leaf, subleaf)
    // For now, return a stub
    CpuidResult {
        eax: 0,
        ebx: 0,
        ecx: 0,
        edx: 0,
    }
}

/// Perform CPUID operation
///
/// # Arguments
///
/// * `leaf` - CPUID leaf to query
///
/// # Returns
///
/// CPUID result
#[inline]
pub fn cpuid_query(leaf: u32) -> CpuidResult {
    unsafe { cpuid_leaf(leaf) }
}

/// Check if FSGSBASE instructions are available
///
/// # Returns
///
/// true if RDFSBASE/WRFSBASE/RDGSBASE/WRGSBASE instructions are available
pub fn g_x86_feature_fsgsbase() -> bool {
    // Check CPUID leaf 7, subleaf 0, EBX bit 0
    x86_feature_test(7, 0, 1, 0)
}

/// Check if SMAP is available
///
/// # Returns
///
/// true if Supervisor Mode Access Prevention is available
pub fn x86_feature_smap() -> bool {
    // Check CPUID leaf 7, subleaf 0, ECX bit 20
    x86_feature_test(7, 0, 2, 20)
}

/// Check if SMEP is available
///
/// # Returns
///
/// true if Supervisor Mode Execution Protection is available
pub fn x86_feature_smep() -> bool {
    // Check CPUID leaf 7, subleaf 0, EBX bit 7
    x86_feature_test(7, 0, 1, 7)
}

/// Check if CLFLUSH is available
///
/// # Returns
///
/// true if CLFLUSH instruction is available
pub fn x86_feature_clflush() -> bool {
    // Check CPUID leaf 1, EDX bit 19
    x86_feature_test(1, 0, 3, 19)
}

/// Check if CLFLUSHOPT is available
///
/// # Returns
///
/// true if CLFLUSHOPT instruction is available
pub fn x86_feature_clflushopt() -> bool {
    // Check CPUID leaf 7, subleaf 0, EBX bit 23
    x86_feature_test(7, 0, 1, 23)
}

/// X86 feature constants for convenience functions
pub const FEAT_CLFLUSH: u32 = 0;
pub const FEAT_CLFLUSHOPT: u32 = 1;
pub const FEAT_SMAP: u32 = 2;
pub const FEAT_SMEP: u32 = 3;

/// Test a feature by convenience constant
pub fn x86_has_feature(feature: u32) -> bool {
    match feature {
        FEAT_CLFLUSH => x86_feature_clflush(),
        FEAT_CLFLUSHOPT => x86_feature_clflushopt(),
        FEAT_SMAP => x86_feature_smap(),
        FEAT_SMEP => x86_feature_smep(),
        _ => false,
    }
}
