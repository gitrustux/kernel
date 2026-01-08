// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit spinlock implementation
//!
//! Uses the LR/SC (load-reserved/store-conditional) pattern for atomic operations.

use crate::arch::riscv64;
use crate::arch::ops;
use core::sync::atomic::{AtomicU32, Ordering};

/// RISC-V spinlock using LR/SC
#[repr(C)]
pub struct SpinLock {
    lock: AtomicU32,
}

impl SpinLock {
    pub const fn new() -> Self {
        Self {
            lock: AtomicU32::new(0),
        }
    }

    /// Acquire the spinlock
    #[inline(always)]
    pub fn acquire(&self) {
        while self.try_acquire() == false {
            // Spin with pause hint
            ops::arch_spinloop_pause();
        }
    }

    /// Try to acquire the spinlock without blocking
    #[inline(always)]
    pub fn try_acquire(&self) -> bool {
        let result: i32;
        unsafe {
            core::arch::asm!(
                "1:",
                "lr.d w, ({0})",     // Load-reserved
                "bnez w, 2f",          // If locked, fail
                "li a0, 1",
                "sc.d a0, a0, ({0})", // Store-conditional
                "bnez a0, 1b",         // Retry if SC failed
                "li {1}, 1",      // Success
                "j 3f",
                "2:",
                "li {1}, 0",      // Failure
                "3:",
                in(reg) self.lock.as_ptr(),
                out(reg) result,
                options(nostack),
            );
        }
        result != 0
    }

    /// Release the spinlock
    #[inline(always)]
    pub fn release(&self) {
        self.lock.store(0, Ordering::Release);
        ops::smp_mb();
    }

    /// Check if the spinlock is held
    #[inline(always)]
    pub fn is_held(&self) -> bool {
        self.lock.load(Ordering::Acquire) != 0
    }
}

/// RISC-V spinlock save state - IRQ flags
#[repr(C)]
pub struct SpinLockSaveIrqSave {
    state: u64,
}

impl SpinLockSaveIrqSave {
    pub const fn new() -> Self {
        Self { state: 0 }
    }
}

/// Save interrupt state and acquire spinlock
#[inline(always)]
pub fn spin_lock_save(lock: &SpinLock, state: &mut SpinLockSaveIrqSave) {
    state.state = unsafe { ops::arch_disable_ints() };
    lock.acquire();
}

/// Restore interrupt state and release spinlock
#[inline(always)]
pub fn spin_unlock_restore(lock: &SpinLock, state: &SpinLockSaveIrqSave) {
    lock.release();
    unsafe { ops::arch_restore_ints(state.state) };
}
