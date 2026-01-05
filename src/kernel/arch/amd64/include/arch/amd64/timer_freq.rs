// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 timer frequency detection
//!
//! This module provides functions for detecting the frequency of the x86
//! CPU's core crystal clock and timestamp counter (TSC).

/// Returns the core crystal clock frequency if it can be determined from the CPU
/// alone (without calibration), or 0 if it cannot be determined.
///
/// # Returns
///
/// The core crystal clock frequency in Hz, or 0 if unknown
pub fn x86_lookup_core_crystal_freq() -> u64 {
    unsafe { sys_x86_lookup_core_crystal_freq() }
}

/// Returns the TSC (Time Stamp Counter) frequency if it can be determined from the CPU
/// alone (without calibration), or 0 if it cannot be determined.
///
/// # Returns
///
/// The TSC frequency in Hz, or 0 if unknown
pub fn x86_lookup_tsc_freq() -> u64 {
    unsafe { sys_x86_lookup_tsc_freq() }
}

// External function declarations
extern "C" {
    fn sys_x86_lookup_core_crystal_freq() -> u64;
    fn sys_x86_lookup_tsc_freq() -> u64;
}