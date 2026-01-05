// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Condition variable implementation
//!
//! This module provides condition variables for thread synchronization.

#![no_std]

use core::sync::atomic::{AtomicU32, Ordering};
use libsys::{Result, Error, Status, syscall::SyscallNumber};

/// Condition variable state
const CONDVAR_NO_WAITERS: u32 = 0;
const CONDVAR_HAS_WAITERS: u32 = 1;

/// Result from waiting on a condition variable
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    /// Wait completed successfully
    Success,
    /// Wait timed out
    TimedOut,
    /// Condition variable was destroyed
    Invalid,
}

/// A condition variable
///
/// Condition variables allow threads to block until a condition is met.
#[repr(C)]
pub struct Condvar {
    /// Internal state
    state: AtomicU32,
    /// Sequence counter for tracking wake-ups
    sequence: AtomicU32,
}

// Condvar is Send and Sync
unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    /// Create a new condition variable
    pub const fn new() -> Self {
        Self {
            state: AtomicU32::new(CONDVAR_NO_WAITERS),
            sequence: AtomicU32::new(0),
        }
    }

    /// Wait on the condition variable
    ///
    /// This function atomically releases the mutex and blocks the current thread.
    /// When the condition variable is signaled, the mutex is re-acquired.
    ///
    /// # Arguments
    ///
    /// * `mutex` - The mutex to release while waiting
    pub fn wait(&self, mutex: &super::mutex::Mutex) {
        self.wait_with(mutex, || true)
    }

    /// Wait on the condition variable with a timeout
    ///
    /// # Arguments
    ///
    /// * `mutex` - The mutex to release while waiting
    /// * `nanos` - Timeout in nanoseconds
    pub fn wait_timeout(&self, mutex: &super::mutex::Mutex, nanos: u64) -> WaitResult {
        self.wait_with_timeout(mutex, || true, nanos)
    }

    /// Wait while a condition is true
    ///
    /// This function will continue waiting as long as the condition
    /// callback returns true.
    ///
    /// # Arguments
    ///
    /// * `mutex` - The mutex to release while waiting
    /// * `condition` - Function that returns true to continue waiting
    pub fn wait_while(&self, mutex: &super::mutex::Mutex, condition: impl Fn() -> bool) {
        self.wait_with(mutex, condition)
    }

    /// Internal wait implementation
    fn wait_with(&self, mutex: &super::mutex::Mutex, mut condition: impl Fn() -> bool) {
        // Increment sequence
        let seq = self.sequence.fetch_add(1, Ordering::Acquire);

        loop {
            // Release mutex while waiting
            unsafe { mutex.unlock() };

            // Wait on the futex
            unsafe {
                libsys::syscall::syscall2(
                    SyscallNumber::FutexWait as u64,
                    &self.sequence as *const AtomicU32 as u64,
                    seq as u64,
                );
            }

            // Re-acquire mutex
            mutex.lock();

            // Check if condition is satisfied
            if !condition() {
                break;
            }
        }
    }

    /// Internal wait implementation with timeout
    fn wait_with_timeout(
        &self,
        mutex: &super::mutex::Mutex,
        condition: impl Fn() -> bool,
        nanos: u64,
    ) -> WaitResult {
        // Increment sequence
        let seq = self.sequence.fetch_add(1, Ordering::Acquire);

        // Calculate deadline
        let start = self.get_time();
        let deadline = start.saturating_add(nanos);

        loop {
            // Release mutex while waiting
            unsafe { mutex.unlock() };

            // Wait on the futex with timeout
            unsafe {
                libsys::syscall::syscall3(
                    SyscallNumber::FutexWait as u64,
                    &self.sequence as *const AtomicU32 as u64,
                    seq as u64,
                    deadline,
                );
            }

            // Re-acquire mutex
            mutex.lock();

            // Check if condition is satisfied
            if !condition() {
                return WaitResult::Success;
            }

            // Check for timeout
            let now = self.get_time();
            if now >= deadline {
                return WaitResult::TimedOut;
            }
        }
    }

    /// Get the current time in nanoseconds
    #[inline]
    fn get_time(&self) -> u64 {
        // TODO: Use VDSO clock_get_monotonic when available
        // For now, use a simple counter
        0
    }

    /// Signal one waiting thread
    ///
    /// If any threads are waiting on this condition variable, one will be woken.
    pub fn notify_one(&self) {
        self.sequence.fetch_add(1, Ordering::Release);

        unsafe {
            libsys::syscall::syscall2(
                SyscallNumber::FutexWake as u64,
                &self.sequence as *const AtomicU32 as u64,
                1, // wake one
            );
        }
    }

    /// Signal all waiting threads
    ///
    /// All threads waiting on this condition variable will be woken.
    pub fn notify_all(&self) {
        self.sequence.fetch_add(1, Ordering::Release);

        unsafe {
            libsys::syscall::syscall2(
                SyscallNumber::FutexWake as u64,
                &self.sequence as *const AtomicU32 as u64,
                u32::MAX as u64, // wake all
            );
        }
    }
}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}
