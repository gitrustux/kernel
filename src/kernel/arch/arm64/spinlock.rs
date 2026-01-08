// Copyright 2025 Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::ops::arch_curr_cpu_num;
use core::arch::asm;

// We need to use unsafe code in this file since we're implementing
// the locks themselves. The public interface will have proper safety
// guarantees, but internally we need to work with raw memory.

/// Acquire a spinlock
///
/// This function will spin until the lock is acquired.
/// 
/// # Safety
///
/// This function is unsafe because it doesn't check if the lock is already held
/// by the current thread, which could lead to deadlocks.
#[no_mangle]
pub unsafe extern "C" fn arch_spin_lock(lock: *mut spin_lock_t) {
    let val = arch_curr_cpu_num() + 1;
    let mut temp: u64;

    // This is a direct translation of the assembly in the C++ version.
    // It uses inline assembly to implement a spinlock with proper memory barriers.
    asm!(
        "sevl",
        "1: wfe",
        "ldaxr {temp}, [{lock}]",
        "cbnz {temp}, 1b",
        "stxr w{temp}, {val}, [{lock}]",
        "cbnz w{temp}, 1b",
        temp = out(reg) temp,
        lock = in(reg) &(*lock).value,
        val = in(reg) val,
        options(nostack)
    );
}

/// Try to acquire a spinlock without blocking
///
/// # Returns
///
/// * 0 if the lock was acquired
/// * Non-zero if the lock was already held
/// 
/// # Safety
///
/// This function is unsafe because it doesn't provide any thread safety guarantees.
#[no_mangle]
pub unsafe extern "C" fn arch_spin_trylock(lock: *mut spin_lock_t) -> i32 {
    let val = arch_curr_cpu_num() + 1;
    let mut out: u64;

    // Direct translation of the assembly in the C++ version
    asm!(
        "ldaxr {out}, [{lock}]",
        "cbnz {out}, 1f",
        "stxr w{out}, {val}, [{lock}]",
        "1:",
        out = out(reg) out,
        lock = in(reg) &(*lock).value,
        val = in(reg) val,
        options(nostack)
    );

    out as i32
}

/// Release a previously acquired spinlock
///
/// # Safety
///
/// This function is unsafe because it doesn't check if the lock is actually held
/// by the current thread.
#[no_mangle]
pub unsafe extern "C" fn arch_spin_unlock(lock: *mut spin_lock_t) {
    // We use atomic store with SeqCst ordering to provide
    // the same guarantees as the C++ __atomic_store_n with __ATOMIC_SEQ_CST
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    (*lock).value = 0;
}

/// Spinlock structure definition
#[repr(C)]
pub struct spin_lock_t {
    pub value: u64,
}

impl spin_lock_t {
    /// Create a new unlocked spinlock
    pub const fn new() -> Self {
        spin_lock_t { value: 0 }
    }
}

// A safer Rust wrapper around the raw lock functions
pub struct SpinLock<T> {
    lock: spin_lock_t,
    data: core::cell::UnsafeCell<T>,
}

// SpinLock can be safely shared between threads
unsafe impl<T> Sync for SpinLock<T> {}
unsafe impl<T> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Create a new spinlock protecting the given data
    pub const fn new(data: T) -> Self {
        SpinLock {
            lock: spin_lock_t::new(),
            data: core::cell::UnsafeCell::new(data),
        }
    }

    /// Lock the spinlock, returning a guard that provides access to the data
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        unsafe {
            arch_spin_lock(&self.lock as *const _ as *mut _);
            SpinLockGuard { lock: self }
        }
    }

    /// Try to lock the spinlock without blocking
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        unsafe {
            if arch_spin_trylock(&self.lock as *const _ as *mut _) == 0 {
                Some(SpinLockGuard { lock: self })
            } else {
                None
            }
        }
    }
    
    /// Get a mutable reference to the underlying data
    ///
    /// # Safety
    ///
    /// This function is unsafe because it bypasses the lock mechanism.
    /// The caller must ensure that they have exclusive access to the data.
    pub unsafe fn get_mut_unchecked(&self) -> &mut T {
        &mut *self.data.get()
    }
}

/// A RAII guard that releases the lock when dropped
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            arch_spin_unlock(&self.lock.lock as *const _ as *mut _);
        }
    }
}

impl<'a, T> core::ops::Deref for SpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}