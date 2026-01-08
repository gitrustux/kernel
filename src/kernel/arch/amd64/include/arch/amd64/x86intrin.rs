// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 intrinsics for Rustux
//!
//! This module provides access to x86-specific intrinsics and CPU features.
//! It serves as a Rust wrapper around the functionality traditionally
//! provided by the <x86intrin.h> C header.

// We can leverage Rust's built-in intrinsics support through the core::arch module
// which provides architecture-specific intrinsics including x86/x86_64 ones.

/// Macro to check if a CPU feature is supported at runtime
///
/// This is a stub implementation that always returns false for now.
/// In a real implementation, this would use CPUID to check for feature support.
#[macro_export]
macro_rules! is_x86_feature_detected {
    ($feature:tt) => {
        false
    };
}

pub use core::arch::x86_64::*;

// Note: The original header contained workarounds for GCC bugs with certain
// intrinsic headers when using -mno-sse. Rust's approach is different and 
// doesn't have these specific issues, as it provides fine-grained control
// over which intrinsics are available based on the target features enabled.

/// Re-export specific AVX-512 intrinsics when target features are enabled
#[cfg(all(target_arch = "x86_64", target_feature = "avx512f"))]
pub mod avx512 {
    pub use core::arch::x86_64::{
        // AVX-512 foundation intrinsics
        _mm512_set1_epi32, _mm512_set1_epi64, _mm512_set1_ps, _mm512_set1_pd,
        _mm512_add_epi32, _mm512_add_epi64, _mm512_add_ps, _mm512_add_pd,
        _mm512_sub_epi32, _mm512_sub_epi64, _mm512_sub_ps, _mm512_sub_pd,
        _mm512_mul_epi32, _mm512_mul_epi64, _mm512_mul_ps, _mm512_mul_pd,
        // Add more as needed
    };
}

/// Re-export specific AVX intrinsics when target features are enabled
#[cfg(all(target_arch = "x86_64", target_feature = "avx"))]
pub mod avx {
    pub use core::arch::x86_64::{
        _mm256_set1_epi32, _mm256_set1_epi64x, _mm256_set1_ps, _mm256_set1_pd,
        _mm256_add_epi32, _mm256_add_ps, _mm256_add_pd,
        _mm256_sub_ps, _mm256_sub_pd,
        _mm256_mul_ps, _mm256_mul_pd,
        // Add more as needed
    };
}

/// Re-export specific SSE intrinsics when target features are enabled
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub mod sse {
    pub use core::arch::x86_64::{
        _mm_set1_epi32, _mm_set1_epi64x, _mm_set1_ps, _mm_set1_pd,
        _mm_add_epi32, _mm_add_ps, _mm_add_pd,
        _mm_sub_ps, _mm_sub_pd,
        _mm_mul_ps, _mm_mul_pd,
        // Add more as needed
    };
}

// Function to check if a CPU feature is available at runtime
// This is useful for code that needs to use different implementations
// based on available CPU features
#[inline]
pub fn has_cpu_feature(feature: &str) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        match feature {
            "sse" => is_x86_feature_detected!("sse"),
            "sse2" => is_x86_feature_detected!("sse2"),
            "sse3" => is_x86_feature_detected!("sse3"),
            "ssse3" => is_x86_feature_detected!("ssse3"),
            "sse4.1" => is_x86_feature_detected!("sse4.1"),
            "sse4.2" => is_x86_feature_detected!("sse4.2"),
            "avx" => is_x86_feature_detected!("avx"),
            "avx2" => is_x86_feature_detected!("avx2"),
            "fma" => is_x86_feature_detected!("fma"),
            "bmi1" => is_x86_feature_detected!("bmi1"),
            "bmi2" => is_x86_feature_detected!("bmi2"),
            "avx512f" => is_x86_feature_detected!("avx512f"),
            "avx512bw" => is_x86_feature_detected!("avx512bw"),
            "avx512dq" => is_x86_feature_detected!("avx512dq"),
            "avx512vl" => is_x86_feature_detected!("avx512vl"),
            _ => false,
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = feature;
        false
    }
}