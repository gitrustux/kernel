// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! AMD64 (x86-64) timer support
//!
//! This module provides functions for accessing the x86-64 Time Stamp Counter (TSC)
//! and related timer functionality.


use crate::kernel::arch::amd64;

/// TSC frequency in Hz (cached)
///
/// This will be initialized during boot to contain the actual TSC frequency.
/// For now, we use a placeholder value that will be detected at runtime.
static mut TSC_FREQUENCY: u64 = 0;

/// Placeholder TSC frequency (will be detected at runtime)
const DEFAULT_TSC_MHZ: u64 = 2000; // 2 GHz placeholder

/// Read the Time Stamp Counter
///
/// The TSC is a 64-bit register that counts processor cycles.
/// It is incremented every CPU cycle and is synchronized across cores.
///
/// # Returns
///
/// The current TSC value as a 64-bit unsigned integer.
#[inline]
pub fn x86_rdtsc() -> u64 {
    unsafe {
        let (low, high): (u32, u32);
        core::arch::asm!("rdtsc", lateout("eax") low, lateout("edx") high, options(nomem, nostack));
        ((high as u64) << 32) | (low as u64)
    }
}

/// Read the Time Stamp Counter with serialization
///
/// This is a serialized version of RDTSC that ensures all prior instructions
/// have executed before reading the TSC.
///
/// # Returns
///
/// The current TSC value as a 64-bit unsigned integer.
#[inline]
pub unsafe fn x86_rdtsc_serialized() -> u64 {
    let (low, high): (u32, u32);
    core::arch::asm!(
        "cpuid",
        "rdtsc",
        out("eax") low,
        lateout("ecx") _,
        out("edx") high,
        options(nostack, nomem)
    );
    ((high as u64) << 32) | (low as u64)
}

/// Initialize the TSC frequency
///
/// This should be called during boot to determine the actual TSC frequency.
/// For now, this uses a placeholder implementation.
///
/// # Safety
///
/// This function modifies a global static variable.
pub unsafe fn x86_tsc_init() {
    // In a real implementation, this would:
    // 1. Use CPUID to check for invariant TSC
    // 2. Measure TSC frequency using a known time source (e.g., HPET, PIT)
    // 3. Cache the frequency for later use
    //
    // For now, we use a placeholder value
    TSC_FREQUENCY = DEFAULT_TSC_MHZ * 1_000_000;
}

/// Get the TSC frequency
///
/// Returns the cached TSC frequency in Hz.
///
/// # Returns
///
/// The TSC frequency in Hz, or a default value if not yet initialized.
#[inline]
pub fn x86_tsc_frequency() -> u64 {
    unsafe {
        if TSC_FREQUENCY == 0 {
            // If not initialized, return the default
            DEFAULT_TSC_MHZ * 1_000_000
        } else {
            TSC_FREQUENCY
        }
    }
}

/// Set the TSC frequency
///
/// This is called by platform code after detecting the actual frequency.
///
/// # Arguments
///
/// * `freq` - The TSC frequency in Hz
///
/// # Safety
///
/// This function modifies a global static variable.
pub unsafe fn x86_tsc_set_frequency(freq: u64) {
    TSC_FREQUENCY = freq;
}

/// Convert TSC ticks to nanoseconds
///
/// # Arguments
///
/// * `ticks` - Number of TSC ticks
///
/// # Returns
///
/// Equivalent time in nanoseconds
#[inline]
pub fn x86_tsc_to_ns(ticks: u64) -> u64 {
    let freq = x86_tsc_frequency();
    if freq == 0 {
        return 0;
    }
    (ticks * 1_000_000_000) / freq
}

/// Convert nanoseconds to TSC ticks
///
/// # Arguments
///
/// * `ns` - Time in nanoseconds
///
/// # Returns
///
/// Equivalent number of TSC ticks
#[inline]
pub fn x86_ns_to_tsc(ns: u64) -> u64 {
    let freq = x86_tsc_frequency();
    if freq == 0 {
        return 0;
    }
    (ns * freq) / 1_000_000_000
}

/// Get the current time in nanoseconds
///
/// # Returns
///
/// Current time in nanoseconds since boot
pub fn amd64_current_time() -> u64 {
    x86_tsc_to_ns(x86_rdtsc())
}
