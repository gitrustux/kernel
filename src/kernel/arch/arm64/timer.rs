// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 Generic Timer support
//!
//! This module provides functions for accessing the ARMv8 Generic Timer,
//! which provides a system counter and per-CPU timers.

#![no_std]

use crate::arch::arm64;

// External assembly functions for system register access
extern "C" {
    /// Read a 64-bit system register
    fn __arm_rsr64(reg: &str) -> u64;

    /// Write a 64-bit system register
    fn __arm_wsr64(reg: &str, val: u64);

    /// Instruction synchronization barrier
    fn __isb(value: u32);
}

/// Memory barrier types
const ARM_MB_SY: u32 = 0xF;

/// Get the current system counter value
///
/// This reads the CNTVCT_EL0 register (Counter-timer Virtual Count register).
/// The virtual counter is unaffected by changes to the virtual offset.
///
/// # Returns
///
/// The current system counter value as a 64-bit unsigned integer.
#[inline]
pub fn arm64_current_time() -> u64 {
    unsafe {
        let cntvct: u64;
        core::arch::asm!("mrs {0}, cntvct_el0", out(reg) cntvct);
        cntvct
    }
}

/// Get the timer frequency
///
/// This reads the CNTFRQ_EL0 register (Counter-timer Frequency register).
/// This value is fixed at boot and indicates the frequency of the system counter in Hz.
///
/// # Returns
///
/// The timer frequency in Hz.
#[inline]
pub fn arm64_timer_get_frequency() -> u64 {
    unsafe {
        let cntfrq: u64;
        core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) cntfrq);
        cntfrq
    }
}

/// Set the physical timer to fire at a specified deadline
///
/// This programs the CNTP_CVAL_EL0 register (Counter-timer Physical Timer CompareValue register).
/// The timer will fire when the system counter reaches or exceeds this value.
///
/// # Arguments
///
/// * `deadline` - The absolute deadline in system counter units
///
/// # Safety
///
/// This function modifies hardware timer state. The caller must ensure proper
/// interrupt handling is set up.
#[inline]
pub unsafe fn arm64_timer_set(deadline: u64) {
    // Set the compare value
    core::arch::asm!("msr cntp_cval_el0, {}", in(reg) deadline);

    // Enable the timer, unmask it
    // Timer control register: bit 0 = enable, bit 1 = mask
    const TIMER_ENABLE: u64 = 1 << 0;
    core::arch::asm!("msr cntp_ctl_el0, {}", in(reg) TIMER_ENABLE);

    // Ensure the timer configuration takes effect
    core::arch::asm!("isb");
}

/// Set the virtual timer to fire at a specified deadline
///
/// This programs the CNTV_CVAL_EL0 register (Counter-timer Virtual Timer CompareValue register).
///
/// # Arguments
///
/// * `deadline` - The absolute deadline in system counter units
///
/// # Safety
///
/// This function modifies hardware timer state. The caller must ensure proper
/// interrupt handling is set up.
#[inline]
pub unsafe fn arm64_timer_set_virtual(deadline: u64) {
    // Set the compare value
    core::arch::asm!("msr cntv_cval_el0, {}", in(reg) deadline);

    // Enable the timer, unmask it
    const TIMER_ENABLE: u64 = 1 << 0;
    core::arch::asm!("msr cntv_ctl_el0, {}", in(reg) TIMER_ENABLE);

    // Ensure the timer configuration takes effect
    core::arch::asm!("isb");
}

/// Cancel the physical timer
///
/// This disables the physical timer by masking it.
///
/// # Safety
///
/// This function modifies hardware timer state.
#[inline]
pub unsafe fn arm64_timer_cancel() {
    // Mask the timer (bit 1 = mask)
    const TIMER_MASK: u64 = 1 << 1;
    core::arch::asm!("msr cntp_ctl_el0, {}", in(reg) TIMER_MASK);

    // Ensure the timer configuration takes effect
    core::arch::asm!("isb");
}

/// Cancel the virtual timer
///
/// This disables the virtual timer by masking it.
///
/// # Safety
///
/// This function modifies hardware timer state.
#[inline]
pub unsafe fn arm64_timer_cancel_virtual() {
    // Mask the timer (bit 1 = mask)
    const TIMER_MASK: u64 = 1 << 1;
    core::arch::asm!("msr cntv_ctl_el0, {}", in(reg) TIMER_MASK);

    // Ensure the timer configuration takes effect
    core::arch::asm!("isb");
}

/// Get the current physical timer compare value
///
/// # Returns
///
/// The current compare value for the physical timer.
#[inline]
pub fn arm64_timer_get_compare() -> u64 {
    unsafe {
        let cval: u64;
        core::arch::asm!("mrs {0}, cntp_cval_el0", out(reg) cval);
        cval
    }
}

/// Check if the physical timer is enabled
///
/// # Returns
///
/// `true` if the timer is enabled, `false` otherwise.
#[inline]
pub fn arm64_timer_enabled() -> bool {
    unsafe {
        let ctl: u64;
        core::arch::asm!("mrs {0}, cntp_ctl_el0", out(reg) ctl);
        (ctl & 1) != 0
    }
}

/// Re-arm the timer with a new deadline
///
/// This is a convenience function that cancels any existing timer
/// and sets a new deadline.
///
/// # Arguments
///
/// * `deadline` - The new absolute deadline in system counter units
///
/// # Safety
///
/// This function modifies hardware timer state.
#[inline]
pub unsafe fn arm64_timer_rearm(deadline: u64) {
    arm64_timer_cancel();
    arm64_timer_set(deadline);
}
