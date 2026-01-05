// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Time Stamp Counter (TSC) management for x86
//!
//! This module provides functions for adjusting and storing the Time Stamp Counter
//! settings on x86 processors.

/// Adjust the Time Stamp Counter (TSC)
///
/// This function makes adjustments to the TSC to ensure proper timekeeping.
///
/// # Safety
///
/// This function is unsafe because it directly modifies CPU state related to timing.
pub unsafe fn x86_tsc_adjust() {
    sys_x86_tsc_adjust();
}

/// Store the current TSC adjustment value
///
/// This function saves the current TSC adjustment value for later use.
///
/// # Safety
///
/// This function is unsafe because it accesses CPU state related to timing.
pub unsafe fn x86_tsc_store_adjustment() {
    sys_x86_tsc_store_adjustment();
}

// External function declarations for the system implementations
extern "C" {
    fn sys_x86_tsc_adjust();
    fn sys_x86_tsc_store_adjustment();
}