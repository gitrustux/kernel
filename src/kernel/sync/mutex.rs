// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Mutex
//!
//! This module provides mutual exclusion locks for the Rustux kernel.
//! These mutexes track ownership and integrate with the scheduler
//! for proper blocking behavior.
//!
//! # Design
//!
//! - **Ownership tracking**: Each mutex knows which thread owns it
//! - **Fair scheduling**: Waiters are woken in FIFO order
//! - **Priority inheritance**: (TODO) Owner inherits priority of waiters
//! - **Deadlock detection**: (TODO) Detect if a thread tries to lock its own mutex
//!
//! # Usage
//!
//! ```rust
//! let mutex = Mutex::new();
//!
//! // Acquire the mutex
//! mutex.lock();
//!
//! // Critical section
//! // ...
//!
//! // Release the mutex
//! mutex.unlock();
//! ```

#![no_std]

use crate::kernel::thread::{Thread, ThreadId};
use crate::rustux::types::*;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::kernel::sync::spin::SpinMutex as SpinMutex;
use alloc::vec::Vec;

// Import logging macros
use crate::log_debug;

/// ============================================================================
/// Mutex
/// ============================================================================

/// Mutual exclusion lock
///
/// Tracks ownership and integrates with scheduler for blocking.
pub struct Mutex<T> {
    /// Protected data
    data: T,

    /// Current owner (0 = unlocked, otherwise ThreadId)
    owner: AtomicU64,

    /// Flag indicating if there are waiters
    has_waiters: AtomicBool,

    /// Magic number for validation
    magic: u32,

    /// Wait queue of threads waiting for this mutex
    waiters: SpinMutex<Vec<ThreadId>>,
}

/// Magic number for mutex validation
const MUTEX_MAGIC: u32 = 0x4D555478; // "MUTx" in hex

impl<T> Mutex<T> {
    /// Create a new mutex
    pub const fn new(data: T) -> Self {
        Self {
            data,
            owner: AtomicU64::new(0),
            has_waiters: AtomicBool::new(false),
            magic: MUTEX_MAGIC,
            waiters: SpinMutex::new(Vec::new()),
        }
    }

    /// Initialize a mutex (for heap-allocated mutexes)
    pub fn init(&mut self) {
        self.owner.store(0, Ordering::Release);
        self.has_waiters.store(false, Ordering::Release);
        self.magic = MUTEX_MAGIC;
        self.waiters.lock().clear();
    }

    /// Destroy a mutex
    ///
    /// Panics if the mutex is currently locked.
    pub fn destroy(&self) {
        self.validate();

        if self.is_locked() {
            panic!("mutex_destroy: tried to destroy locked mutex");
        }

        // Clear magic to mark as destroyed
        // Note: This is a const operation issue - in real code we'd use interior mutability
    }

    /// Acquire the mutex
    ///
    /// Blocks the current thread until the mutex is available.
    pub fn lock(&self) -> MutexGuard<T> {
        self.validate();

        let current_tid = ThreadId::current(); // This would need to be implemented

        // Fast path: try to acquire if unlocked
        if self.try_lock_fast(current_tid) {
            return MutexGuard::new(self);
        }

        // Slow path: need to block
        self.lock_contended(current_tid);
        MutexGuard::new(self)
    }

    /// Try to acquire the mutex without blocking
    ///
    /// Returns Some(guard) if acquired, None if already locked.
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        self.validate();

        let current_tid = ThreadId::current();
        if self.try_lock_fast(current_tid) {
            Some(MutexGuard::new(self))
        } else {
            None
        }
    }

    /// Get raw access to the data (without locking)
    ///
    /// # Safety
    ///
    /// This is unsafe and should only be used when you know the mutex is locked.
    pub unsafe fn raw_data(&self) -> &T {
        &self.data
    }

    /// Get raw mutable access to the data (without locking)
    ///
    /// # Safety
    ///
    /// This is unsafe and should only be used when you know the mutex is locked.
    pub unsafe fn raw_data_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Release the mutex
    ///
    /// Panics if the current thread doesn't own the mutex.
    pub fn unlock(&self) {
        self.validate();

        let current_tid = ThreadId::current();

        // Check that we own it
        if self.owner.load(Ordering::Acquire) != current_tid {
            panic!("mutex_release: thread tried to release mutex it doesn't own");
        }

        // Fast path: no waiters
        if !self.has_waiters.load(Ordering::Acquire) {
            self.owner.store(0, Ordering::Release);
            return;
        }

        // Slow path: wake a waiter
        self.unlock_contended(current_tid);
    }

    /// Check if the mutex is currently locked
    pub fn is_locked(&self) -> bool {
        self.owner.load(Ordering::Acquire) != 0
    }

    /// Get the owner thread ID
    ///
    /// Returns None if unlocked.
    pub fn owner(&self) -> Option<ThreadId> {
        let owner = self.owner.load(Ordering::Acquire);
        if owner == 0 {
            None
        } else {
            Some(owner)
        }
    }

    /// Validate that this is a valid mutex
    fn validate(&self) {
        debug_assert_eq!(self.magic, MUTEX_MAGIC, "invalid mutex magic");
    }

    /// Fast path: try to acquire without blocking
    fn try_lock_fast(&self, tid: ThreadId) -> bool {
        self.owner
            .compare_exchange(0, tid, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Contended acquire path
    fn lock_contended(&self, tid: ThreadId) {
        // Set the has_waiters flag
        self.has_waiters.store(true, Ordering::Release);

        // Add ourselves to the wait queue
        {
            let mut waiters = self.waiters.lock();
            waiters.push(tid);
        }

        // Block the current thread
        // In a real implementation, this would integrate with the scheduler
        // For now, this is a stub
        log_debug!("Thread {} blocking on mutex", tid);

        // TODO: Block on wait queue
        // crate::kernel::sched::block_current(BlockReason::Lock);
    }

    /// Contended release path
    fn unlock_contended(&self, current_tid: ThreadId) {
        // Wake the next waiter
        let next_tid = {
            let mut waiters = self.waiters.lock();
            if waiters.is_empty() {
                // No waiters despite the flag - clear it and return
                self.has_waiters.store(false, Ordering::Release);
                self.owner.store(0, Ordering::Release);
                return;
            }

            // Dequeue the first waiter
            waiters.remove(0)
        };

        // Clear has_waiters if queue is now empty
        {
            let waiters = self.waiters.lock();
            if waiters.is_empty() {
                self.has_waiters.store(false, Ordering::Release);
            }
        }

        // Transfer ownership
        self.owner.store(next_tid, Ordering::Release);

        // Wake the waiter
        // TODO: Integrate with scheduler
        log_debug!("Waking thread {} for mutex", next_tid);
        // crate::kernel::sched::wake(next_tid);
    }
}

/// ============================================================================
/// Mutex Guard (RAII)
/// ============================================================================

/// RAII guard for a mutex
///
/// Automatically releases the mutex when dropped.
pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> MutexGuard<'a, T> {
    /// Create a new guard from a mutex
    ///
    /// The mutex must already be locked.
    pub const fn new(mutex: &'a Mutex<T>) -> Self {
        Self { mutex }
    }

    /// Access the protected data
    pub fn data(&self) -> &T {
        &self.mutex.data
    }

    /// Mutably access the protected data
    pub fn data_mut(&mut self) -> &mut T {
        // SAFETY: We have exclusive access through the guard
        unsafe { &mut *((&self.mutex.data as *const T) as *mut T) }
    }
}

impl<'a, T> core::ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.mutex.data
    }
}

impl<'a, T> core::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: We have exclusive access through the guard
        unsafe { &mut *((&self.mutex.data as *const T) as *mut T) }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.unlock();
    }
}

/// ============================================================================
/// ThreadId Extension
/// ============================================================================

/// Extension trait for ThreadId to get current thread
///
/// This is a stub - in a real implementation this would
/// access thread-local storage or a per-CPU variable.
pub trait ThreadIdExt {
    /// Get the current thread ID
    fn current() -> Self;
}

impl ThreadIdExt for ThreadId {
    fn current() -> Self {
        // Stub implementation
        // In a real implementation, this would access TLS or per-CPU data
        1
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutex_new() {
        let mutex = Mutex::new();
        assert!(!mutex.is_locked());
        assert!(mutex.owner().is_none());
    }

    #[test]
    fn test_mutex_try_lock() {
        let mutex = Mutex::new();

        // First lock should succeed
        assert!(mutex.try_lock());
        assert!(mutex.is_locked());

        // Second lock should fail
        assert!(!mutex.try_lock());

        // Unlock
        mutex.unlock();
        assert!(!mutex.is_locked());
    }

    #[test]
    fn test_mutex_magic() {
        let mutex = Mutex::new();
        assert_eq!(mutex.magic, MUTEX_MAGIC);
    }
}
