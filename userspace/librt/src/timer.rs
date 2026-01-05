// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Timer helpers
//!
//! This module provides high-resolution timer functionality.

#![no_std]

use core::sync::atomic::{AtomicU64, Ordering};
use libsys::{Result, Error, Handle, Status, syscall::SyscallNumber};

/// Timer ID type
pub type TimerId = u64;

/// Timer slack modes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerSlack {
    /// Default slack
    Default = 0,
    /// Center slack (default behavior)
    Center = 1,
    /// Early slack
    Early = 2,
    /// Late slack
    Late = 3,
}

/// A high-resolution timer
///
/// Timers can be used to schedule callbacks after a delay or at specific times.
#[repr(C)]
pub struct Timer {
    /// Handle to the timer
    handle: Handle,
    /// Next timer ID
    next_id: AtomicU64,
}

impl Timer {
    /// Create a new timer
    pub fn new() -> Result<Self> {
        // TODO: Implement timer creation
        Ok(Self {
            handle: Handle::INVALID,
            next_id: AtomicU64::new(1),
        })
    }

    /// Set a timer that fires once
    ///
    /// # Arguments
    ///
    /// * `deadline` - Deadline in nanoseconds (monotonic clock)
    /// * `slack` - Slack mode for the timer
    ///
    /// # Returns
    ///
    /// Timer ID that can be used to cancel the timer
    pub fn set_oneshot(&self, deadline: u64, slack: TimerSlack) -> Result<TimerId> {
        unsafe {
            let ret = libsys::syscall::syscall3(
                SyscallNumber::TimerSet as u64,
                self.handle.raw() as u64,
                deadline,
                slack as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            Ok(id)
        }
    }

    /// Set a repeating timer
    ///
    /// # Arguments
    ///
    /// * `interval` - Interval in nanoseconds
    /// * `slack` - Slack mode for the timer
    ///
    /// # Returns
    ///
    /// Timer ID that can be used to cancel the timer
    pub fn set_periodic(&self, interval: u64, slack: TimerSlack) -> Result<TimerId> {
        unsafe {
            let ret = libsys::syscall::syscall4(
                SyscallNumber::TimerSet as u64,
                self.handle.raw() as u64,
                0, // deadline (0 for periodic)
                interval,
                slack as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            Ok(id)
        }
    }

    /// Cancel a timer
    ///
    /// # Arguments
    ///
    /// * `id` - Timer ID to cancel
    pub fn cancel(&self, id: TimerId) -> Result<()> {
        // TODO: Implement timer cancellation
        unsafe {
            let ret = libsys::syscall::syscall2(
                SyscallNumber::TimerCancel as u64,
                self.handle.raw() as u64,
                id,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Wait for the next timer event
    ///
    /// This function blocks until a timer fires.
    pub fn wait(&self) -> Result<TimerId> {
        unsafe {
            let ret = libsys::syscall::syscall3(
                SyscallNumber::ObjectWaitOne as u64,
                self.handle.raw() as u64,
                0, // deadline (wait forever)
                0, // signals
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(ret as TimerId)
        }
    }
}

/// Clock types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// Monotonic clock (cannot go backwards)
    Monotonic = 0,
    /// Real-time clock (wall-clock time)
    Realtime = 1,
    /// Thread CPU time
    Thread = 2,
    /// Process CPU time
    Process = 3,
}

/// Get the current time from a clock
///
/// # Arguments
///
/// * `clock` - Which clock to read
///
/// # Returns
///
/// Time in nanoseconds
pub fn get_time(clock: Clock) -> Result<u64> {
    unsafe {
        let ret = libsys::syscall::syscall1(
            SyscallNumber::ClockGet as u64,
            clock as u64,
        );

        if (ret as i32) < 0 {
            return Err(Error::from_raw(ret as i32));
        }

        Ok(ret)
    }
}

/// Get the monotonic time
///
/// This is the preferred clock for measuring intervals.
#[inline]
pub fn get_monotonic_time() -> u64 {
    // TODO: Use VDSO clock_get_monotonic when available
    // For now, use a simple syscall
    get_time(Clock::Monotonic).unwrap_or(0)
}

/// Get the real-time (wall-clock) time
#[inline]
pub fn get_realtime_time() -> u64 {
    get_time(Clock::Realtime).unwrap_or(0)
}

/// Sleep for the specified duration
///
/// # Arguments
///
/// * `nanos` - Duration to sleep in nanoseconds
pub fn sleep(nanos: u64) {
    unsafe {
        libsys::syscall::syscall2(
            SyscallNumber::ThreadSleep as u64,
            nanos,
            0, // deadline (relative sleep)
        );
    }
}

/// High-resolution sleep function
///
/// # Arguments
///
/// * `secs` - Seconds to sleep
/// * `nanos` - Additional nanoseconds to sleep
pub fn sleep_high_res(secs: u64, nanos: u64) {
    let total_nanos = secs.saturating_mul(1_000_000_000).saturating_add(nanos);
    sleep(total_nanos);
}
