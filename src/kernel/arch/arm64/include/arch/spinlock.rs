// Copyright 2025 The Rustux Authors
// Copyright (c) 2014 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64::interrupt;
use crate::arch::arm64::mp;
use crate::sys::types::*;
use crate::rustux::compiler::*;
use crate::rustux::thread_annotations::*;

/// ARM64 spinlock implementation
#[derive(Debug)]
#[repr(C)]
pub struct SpinLock {
    value: usize,
}

impl SpinLock {
    /// Create a new unlocked spinlock
    pub const fn new() -> Self {
        Self { value: 0 }
    }
    
    /// Acquire the spinlock, spinning until it becomes available
    #[inline]
    pub fn lock(&self) {
        unsafe { arch_spin_lock(self as *const _ as *mut _) }
    }
    
    /// Try to acquire the spinlock without spinning
    /// 
    /// Returns true if the lock was acquired, false otherwise
    #[inline]
    pub fn try_lock(&self) -> bool {
        unsafe { arch_spin_trylock(self as *const _ as *mut _) != 0 }
    }
    
    /// Release the spinlock
    #[inline]
    pub fn unlock(&self) {
        unsafe { arch_spin_unlock(self as *const _ as *mut _) }
    }
    
    /// Get the CPU ID of the current lock holder, or return an invalid value if not held
    #[inline]
    pub fn holder_cpu(&self) -> u32 {
        unsafe { arch_spin_lock_holder_cpu(self as *const _ as *mut _) }
    }
    
    /// Check if the current CPU holds this lock
    #[inline]
    pub fn held(&self) -> bool {
        unsafe { arch_spin_lock_held(self as *const _ as *mut _) }
    }
}

// Allow static initialization of spinlocks
impl Default for SpinLock {
    fn default() -> Self {
        Self::new()
    }
}

/// Type for storing the interrupt state when acquiring a spinlock
pub type SpinLockSavedState = u32;

/// Flags controlling how interrupts are managed when acquiring a spinlock
pub type SpinLockSaveFlags = u32;

// Spinlock flag definitions
pub mod flags {
    /* Possible future flags:
     * SPIN_LOCK_FLAG_PMR_MASK         = 0x000000ff,
     * SPIN_LOCK_FLAG_PREEMPTION       = 0x10000000,
     * SPIN_LOCK_FLAG_SET_PMR          = 0x20000000,
     */

    /* ARM specific flags */
    pub const IRQ: u32 = 0x40000000;
    pub const FIQ: u32 = 0x80000000; // Do not use unless IRQs are already disabled
    pub const IRQ_FIQ: u32 = IRQ | FIQ;

    /* default arm flag is to just disable plain irqs */
    pub const ARCH_DEFAULT_INTERRUPTS: u32 = IRQ;
}

// Internal state flags
mod state {
    pub const RESTORE_IRQ: u32 = 1;
    pub const RESTORE_FIQ: u32 = 2;
}

// These are the unsafe low-level functions that directly manipulate the lock
// They should only be called by the safe wrapper methods

extern "C" {
    pub fn arch_spin_lock(lock: *mut SpinLock);
    pub fn arch_spin_trylock(lock: *mut SpinLock) -> i32;
    pub fn arch_spin_unlock(lock: *mut SpinLock);
    pub fn arch_spin_lock_holder_cpu(lock: *mut SpinLock) -> u32;
    pub fn arch_spin_lock_held(lock: *mut SpinLock) -> bool;
}

/// Save the current interrupt state and disable interrupts according to flags
#[inline]
pub fn arch_interrupt_save(flags: SpinLockSaveFlags) -> SpinLockSavedState {
    let mut state: SpinLockSavedState = 0;
    
    if (flags & flags::IRQ != 0) && !interrupt::arch_ints_disabled() {
        state |= state::RESTORE_IRQ;
        interrupt::arch_disable_ints();
    }
    
    if (flags & flags::FIQ != 0) && !interrupt::arch_fiqs_disabled() {
        state |= state::RESTORE_FIQ;
        interrupt::arch_disable_fiqs();
    }
    
    state
}

/// Restore interrupts to their previous state
#[inline]
pub fn arch_interrupt_restore(old_state: SpinLockSavedState, flags: SpinLockSaveFlags) {
    if (flags & flags::FIQ != 0) && (old_state & state::RESTORE_FIQ != 0) {
        interrupt::arch_enable_fiqs();
    }
    
    if (flags & flags::IRQ != 0) && (old_state & state::RESTORE_IRQ != 0) {
        interrupt::arch_enable_ints();
    }
}

/// RAII guard that releases the spinlock when dropped
pub struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
    _not_send: core::marker::PhantomData<*mut ()>, // Not Send
}

impl<'a> SpinLockGuard<'a> {
    /// Create a new guard that holds the given lock
    pub fn new(lock: &'a SpinLock) -> Self {
        lock.lock();
        Self {
            lock,
            _not_send: core::marker::PhantomData,
        }
    }
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

/// RAII guard that manages both spinlock and interrupt state
pub struct SpinLockIrqSaveGuard<'a> {
    lock: &'a SpinLock,
    state: SpinLockSavedState,
    flags: SpinLockSaveFlags,
    _not_send: core::marker::PhantomData<*mut ()>, // Not Send
}

impl<'a> SpinLockIrqSaveGuard<'a> {
    /// Create a new guard that holds the given lock and manages interrupt state
    pub fn new(lock: &'a SpinLock, flags: SpinLockSaveFlags) -> Self {
        let state = arch_interrupt_save(flags);
        lock.lock();
        Self {
            lock,
            state,
            flags,
            _not_send: core::marker::PhantomData,
        }
    }
}

impl<'a> Drop for SpinLockIrqSaveGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
        arch_interrupt_restore(self.state, self.flags);
    }
}