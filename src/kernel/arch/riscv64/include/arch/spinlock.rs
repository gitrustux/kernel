// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture-agnostic spinlock interface

use crate::arch::spinlock;

/// Spinlock save state for IRQ-save operations
#[repr(C)]
pub struct SpinLockSaveIrqSave {
    state: u64,
}

impl SpinLockSaveIrqSave {
    pub const fn new() -> Self {
        Self { state: 0 }
    }
}

/// Acquire spinlock with interrupt state saved
#[inline(always)]
pub fn spin_lock_save(lock: &spinlock::SpinLock, state: &mut SpinLockSaveIrqSave) {
    state.state = unsafe { crate::arch::ops::arch_disable_ints() };
    lock.acquire();
}

/// Release spinlock and restore interrupt state
#[inline(always)]
pub fn spin_unlock_restore(lock: &spinlock::SpinLock, state: &SpinLockSaveIrqSave) {
    lock.release();
    unsafe { crate::arch::ops::arch_restore_ints(state.state) };
}
