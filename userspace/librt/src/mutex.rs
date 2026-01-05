// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Mutex implementation
//!
//! This module provides a mutex based on futex for userspace synchronization.

#![no_std]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering};
use libsys::{Result, Error, Status, syscall::SyscallNumber};

/// Mutex state values
const MUTEX_UNLOCKED: u32 = 0;
const MUTEX_LOCKED: u32 = 1;
const MUTEX_CONTENDED: u32 = 2;

/// A mutual exclusion primitive
///
/// This mutex is implemented using futex and provides safe
/// mutual exclusion for critical sections.
#[repr(C)]
pub struct Mutex {
    /// The mutex state (unlocked, locked, or contended)
    state: AtomicU32,
}

// Mutex is Send and Sync
unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

impl Mutex {
    /// Create a new mutex
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(MUTEX_UNLOCKED),
        }
    }

    /// Acquire the mutex
    ///
    /// This function will block until the mutex is available.
    pub fn lock(&self) {
        // Try to acquire the lock
        if self
            .state
            .compare_exchange(MUTEX_UNLOCKED, MUTEX_LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }

        // Lock is contended, use futex to wait
        self.lock_contended();
    }

    #[cold]
    fn lock_contended(&self) {
        loop {
            // Try to acquire again
            if self
                .state
                .compare_exchange(MUTEX_UNLOCKED, MUTEX_LOCKED, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }

            // Set state to contended if it's just locked
            if self
                .state
                .compare_exchange(MUTEX_LOCKED, MUTEX_CONTENDED, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                // Wait on the futex
                unsafe {
                    libsys::syscall::syscall2(
                        SyscallNumber::FutexWait as u64,
                        &self.state as *const AtomicU32 as u64,
                        MUTEX_CONTENDED as u64,
                    );
                }
            }
        }
    }

    /// Try to acquire the mutex without blocking
    ///
    /// # Returns
    ///
    /// - `Ok(guard)` if the mutex was acquired
    /// - `Err(TryLockError::WouldBlock)` if the mutex is held by another thread
    pub fn try_lock(&self) -> Result<MutexGuard> {
        if self
            .state
            .compare_exchange(MUTEX_UNLOCKED, MUTEX_LOCKED, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Ok(MutexGuard { mutex: self })
        } else {
            Err(Error::new(Status::WouldBlock))
        }
    }

    /// Release the mutex
    ///
    /// # Safety
    ///
    /// This function should only be called by the thread that holds the lock.
    unsafe fn unlock(&self) {
        // Try to set state to unlocked
        if self
            .state
            .swap(MUTEX_UNLOCKED, Ordering::Release)
            == MUTEX_CONTENDED
        {
            // Wake one waiter
            libsys::syscall::syscall1(
                SyscallNumber::FutexWake as u64,
                &self.state as *const AtomicU32 as u64,
            );
        }
    }
}

impl Default for Mutex {
    fn default() -> Self {
        Self::new()
    }
}

/// A guard that releases the mutex when dropped
#[repr(C)]
#[derive(Debug)]
pub struct MutexGuard<'a> {
    mutex: &'a Mutex,
}

impl<'a> MutexGuard<'a> {
    /// Create a new mutex guard
    ///
    /// # Safety
    ///
    /// This function should only be called when the mutex is locked.
    unsafe fn new(mutex: &'a Mutex) -> Self {
        Self { mutex }
    }
}

impl<'a> Drop for MutexGuard<'a> {
    fn drop(&mut self) {
        unsafe {
            self.mutex.unlock();
        }
    }
}

/// A reentrant mutex
///
/// A reentrant mutex allows the same thread to lock the mutex
/// multiple times without deadlocking.
#[repr(C)]
pub struct ReentrantMutex {
    /// The underlying mutex
    mutex: Mutex,
    /// Owner thread ID
    owner: UnsafeCell<ThreadId>,
    /// Lock count
    count: UnsafeCell<u32>,
}

// ReentrantMutex is Send and Sync
unsafe impl Send for ReentrantMutex {}
unsafe impl Sync for ReentrantMutex {}

type ThreadId = usize;

impl ReentrantMutex {
    /// Create a new reentrant mutex
    pub const fn new() -> Self {
        Self {
            mutex: Mutex::new(),
            owner: UnsafeCell::new(0),
            count: UnsafeCell::new(0),
        }
    }

    /// Acquire the reentrant mutex
    pub fn lock(&self) {
        let current_id = self.current_thread_id();

        if self.owner() == current_id {
            // Same thread, increment count
            self.set_count(self.count() + 1);
        } else {
            // Different thread, lock the underlying mutex
            self.mutex.lock();
            self.set_owner(current_id);
            self.set_count(1);
        }
    }

    /// Try to acquire the reentrant mutex without blocking
    pub fn try_lock(&self) -> Result<()> {
        let current_id = self.current_thread_id();

        if self.owner() == current_id {
            // Same thread, increment count
            self.set_count(self.count() + 1);
            Ok(())
        } else if self.mutex.try_lock().is_ok() {
            self.set_owner(current_id);
            self.set_count(1);
            Ok(())
        } else {
            Err(Error::new(Status::WouldBlock))
        }
    }

    /// Release the reentrant mutex
    pub fn unlock(&self) {
        let current_id = self.current_thread_id();

        if self.owner() != current_id {
            panic!("ReentrantMutex::unlock() called from a thread that doesn't own the mutex");
        }

        let new_count = self.count() - 1;
        if new_count == 0 {
            self.set_owner(0);
            unsafe {
                self.mutex.unlock();
            }
        } else {
            self.set_count(new_count);
        }
    }

    fn current_thread_id(&self) -> ThreadId {
        // TODO: Get actual thread ID
        0
    }

    fn owner(&self) -> ThreadId {
        unsafe { *self.owner.get() }
    }

    fn set_owner(&self, id: ThreadId) {
        unsafe { *self.owner.get() = id; }
    }

    fn count(&self) -> u32 {
        unsafe { *self.count.get() }
    }

    fn set_count(&self, count: u32) {
        unsafe { *self.count.get() = count; }
    }
}

impl Default for ReentrantMutex {
    fn default() -> Self {
        Self::new()
    }
}
