//! Copyright 2017 The Rustux Authors
//!
//! Use of this source code is governed by a MIT-style
//! license that can be found in the LICENSE file or at
//! https://opensource.org/licenses/MIT

use crate::arch::arm64::arm64_cache_info_t;

/// Global feature flags for ARM64 CPU features
#[no_mangle]
pub static mut arm64_features: u32 = 0;

/// Block size of the dc zva instruction
#[no_mangle]
pub static mut arm64_zva_size: u32 = 0;

/// ICache line size
#[no_mangle]
pub static mut arm64_icache_size: u32 = 0;

/// DCache line size
#[no_mangle]
pub static mut arm64_dcache_size: u32 = 0;

/// Test if a specific ARM64 feature is present
#[inline]
pub fn arm64_feature_test(feature: u32) -> bool {
    // SAFETY: We're reading a global that's only modified during initialization
    unsafe { arm64_features & feature != 0 }
}

/// Initialize the feature detection for ARM64
/// Must be called on every CPU during initialization
pub fn arm64_feature_init() {
    // Implementation would go here
}

/// Dump the feature set for debugging
/// If full is true, dumps additional details
pub fn arm64_feature_debug(full: bool) {
    // Implementation would go here
}

/// Get cache information for the current CPU
pub fn arm64_get_cache_info(info: &mut arm64_cache_info_t) {
    // Implementation would go here
}

/// Dump cache information for a specific CPU
pub fn arm64_dump_cache_info(cpu: u32) {
    // Implementation would go here
}

/// Re-export common features from rustux-features
pub use rustux_features::{
    FEATURE_HW_BREAKPOINT_COMPAT,
    FEATURE_HW_WATCHPOINT_COMPAT,
    // Add other feature flags as needed
};