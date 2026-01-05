//! Copyright 2025 The Rustux Authors
//!
//! Use of this source code is governed by a MIT-style
//! license that can be found in the LICENSE file or at
//! https://opensource.org/licenses/MIT

use core::sync::atomic::compiler_fence;
use core::sync::atomic::Ordering;

/// Enable regular interrupts
#[inline(always)]
pub fn arch_enable_ints() {
    // SAFETY: This is a privileged MSR instruction for interrupt control
    unsafe {
        compiler_fence(Ordering::SeqCst);
        core::arch::asm!("msr daifclr, #2", options(nomem, nostack));
    }
}

/// Disable regular interrupts
#[inline(always)]
pub fn arch_disable_ints() {
    // SAFETY: This is a privileged MSR instruction for interrupt control
    unsafe {
        core::arch::asm!("msr daifset, #2", options(nomem, nostack));
        compiler_fence(Ordering::SeqCst);
    }
}

/// Check if regular interrupts are disabled
#[inline(always)]
pub fn arch_ints_disabled() -> bool {
    let state: u64;
    // SAFETY: This is reading the interrupt state register
    unsafe {
        core::arch::asm!(
            "mrs {}, daif",
            out(reg) state,
            options(nomem, nostack, preserves_flags)
        );
    }
    (state & (1 << 7)) != 0
}

/// Enable FIQ (fast interrupts)
#[inline(always)]
pub fn arch_enable_fiqs() {
    // SAFETY: This is a privileged MSR instruction for FIQ control
    unsafe {
        compiler_fence(Ordering::SeqCst);
        core::arch::asm!("msr daifclr, #1", options(nomem, nostack));
    }
}

/// Disable FIQ (fast interrupts)
#[inline(always)]
pub fn arch_disable_fiqs() {
    // SAFETY: This is a privileged MSR instruction for FIQ control
    unsafe {
        core::arch::asm!("msr daifset, #1", options(nomem, nostack));
        compiler_fence(Ordering::SeqCst);
    }
}

/// Check if FIQs (fast interrupts) are disabled
#[inline(always)]
pub fn arch_fiqs_disabled() -> bool {
    let state: u64;
    // SAFETY: This is reading the interrupt state register
    unsafe {
        core::arch::asm!(
            "mrs {}, daif",
            out(reg) state,
            options(nomem, nostack, preserves_flags)
        );
    }
    (state & (1 << 6)) != 0
}

/// Guard type that automatically enables/disables interrupts
pub struct InterruptGuard {
    was_enabled: bool,
}

impl InterruptGuard {
    /// Create a new guard that disables interrupts and restores them on drop
    #[inline]
    pub fn new() -> Self {
        let was_enabled = !arch_ints_disabled();
        if was_enabled {
            arch_disable_ints();
        }
        Self { was_enabled }
    }
}

impl Drop for InterruptGuard {
    #[inline]
    fn drop(&mut self) {
        if self.was_enabled {
            arch_enable_ints();
        }
    }
}