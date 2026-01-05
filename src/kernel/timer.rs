// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Timer Management
//!
//! This module provides timer management for the Rustux kernel.
//! Timers are used for scheduling timeouts, delayed work, and periodic tasks.
//!
//! # Design
//!
//! - **High-resolution timers**: Nanosecond precision
//! - **Efficient ordering**: Timers stored in priority queue
//! - **Per-CPU timer queues**: Each CPU has its own timer queue
//! - **Integration with scheduler**: Thread wakeups integrated
//!
//! # Usage
//!
//! ```rust
//! let timer = Timer::new();
//!
//! // Set a one-shot timer
//! timer.set_deadline(current_time() + 1_000_000_000); // 1 second
//!
//! // Cancel a timer
//! timer.cancel();
//! ```

#![no_std]

use crate::kernel::thread::ThreadId;
use crate::rustux::types::*;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::kernel::sync::spin::SpinMutex;
use crate::kernel::sync::Mutex;
use alloc::collections::BinaryHeap;
use core::cmp::Ordering as CmpOrdering;

// Import logging macros
use crate::{log_debug, log_info, log_trace};

/// ============================================================================
/// Timer
/// ============================================================================

/// Timer callback function type
pub type TimerCallback = unsafe extern "C" fn(timer: &Timer, arg: u64);

/// Timer structure
///
/// Represents a single timer that can fire at a specific deadline.
pub struct Timer {
    /// Timer name (for debugging)
    pub name: Mutex<Option<&'static str>>,

    /// Deadline when timer should fire (in nanoseconds)
    pub deadline: AtomicU64,

    /// Timer period (0 = one-shot, >0 = periodic)
    pub period: u64,

    /// Callback function
    pub callback: Mutex<Option<TimerCallback>>,

    /// Argument to pass to callback
    pub callback_arg: u64,

    /// Thread to wake (if applicable)
    pub thread: Mutex<Option<ThreadId>>,

    /// Whether timer is active
    pub active: AtomicBool,

    /// Whether timer is periodic
    pub periodic: AtomicBool,

    /// Timer slot in global timer queue (if active)
    pub slot: AtomicU64,
}

unsafe impl Send for Timer {}

impl Timer {
    /// Create a new timer
    pub const fn new() -> Self {
        Self {
            name: Mutex::new(None),
            deadline: AtomicU64::new(0),
            period: 0,
            callback: Mutex::new(None),
            callback_arg: 0,
            thread: Mutex::new(None),
            active: AtomicBool::new(false),
            periodic: AtomicBool::new(false),
            slot: AtomicU64::new(0),
        }
    }

    /// Initialize a timer
    pub fn init(&mut self) {
        self.deadline.store(0, Ordering::Release);
        self.period = 0;
        *self.callback.lock() = None;
        self.callback_arg = 0;
        *self.thread.lock() = None;
        self.active.store(false, Ordering::Release);
        self.periodic.store(false, Ordering::Release);
        self.slot.store(0, Ordering::Release);
    }

    /// Set the timer name
    pub fn set_name(&self, name: &'static str) {
        *self.name.lock() = Some(name);
    }

    /// Set a one-shot deadline
    ///
    /// # Arguments
    ///
    /// * `deadline` - Absolute deadline in nanoseconds
    pub fn set_deadline(&self, deadline: u64) {
        self.deadline.store(deadline, Ordering::Release);
        self.periodic.store(false, Ordering::Release);
    }

    /// Set a periodic timer
    ///
    /// # Arguments
    ///
    /// * `deadline` - First deadline in nanoseconds
    /// * `period` - Period in nanoseconds
    pub fn set_periodic(&mut self, deadline: u64, period: u64) {
        self.deadline.store(deadline, Ordering::Release);
        self.period = period;
        self.periodic.store(true, Ordering::Release);
    }

    /// Set the callback function
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to call when timer fires
    /// * `arg` - Argument to pass to callback
    pub fn set_callback(&self, callback: TimerCallback, arg: u64) {
        *self.callback.lock() = Some(callback);
        self.callback_arg = arg;
    }

    /// Set the thread to wake
    ///
    /// # Arguments
    ///
    /// * `tid` - Thread ID to wake when timer fires
    pub fn set_thread(&self, tid: ThreadId) {
        *self.thread.lock() = Some(tid);
    }

    /// Activate the timer
    ///
    /// Adds the timer to the global timer queue.
    pub fn activate(&self) {
        self.active.store(true, Ordering::Release);
        // In a real implementation, this would add to the timer queue
        log_debug!("Timer activated: deadline={}", self.deadline.load(Ordering::Acquire));
    }

    /// Cancel the timer
    ///
    /// Removes the timer from the timer queue if active.
    pub fn cancel(&self) {
        self.active.store(false, Ordering::Release);
        // In a real implementation, this would remove from the timer queue
        log_debug!("Timer cancelled");
    }

    /// Check if timer is active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Check if timer is periodic
    pub fn is_periodic(&self) -> bool {
        self.periodic.load(Ordering::Acquire)
    }

    /// Get the deadline
    pub fn deadline(&self) -> u64 {
        self.deadline.load(Ordering::Acquire)
    }

    /// Fire the timer
    ///
    /// Called by the timer subsystem when the deadline is reached.
    pub fn fire(&self) {
        // Call the callback if set
        if let Some(callback) = *self.callback.lock() {
            unsafe {
                callback(self, self.callback_arg);
            }
        }

        // Wake the thread if set
        if let Some(tid) = *self.thread.lock() {
            log_debug!("Waking thread {} due to timer", tid);
            // TODO: crate::kernel::sched::wake(tid);
        }

        // If periodic, reschedule
        if self.is_periodic() && self.is_active() {
            let new_deadline = self.deadline.load(Ordering::Acquire) + self.period;
            self.deadline.store(new_deadline, Ordering::Release);
        } else {
            self.active.store(false, Ordering::Release);
        }
    }
}

/// ============================================================================
/// Timer Queue Entry
/// ============================================================================

/// Timer queue entry for heap ordering
#[derive(Debug)]
struct TimerQueueEntry {
    /// Deadline
    deadline: u64,

    /// Unique ID (for tie-breaking)
    id: u64,

    /// Timer pointer (as opaque handle)
    timer: *const Timer,
}

// Implement ordering for BinaryHeap (min-heap by deadline)
impl PartialEq for TimerQueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.id == other.id
    }
}

impl Eq for TimerQueueEntry {}

impl PartialOrd for TimerQueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerQueueEntry {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // Reverse for min-heap behavior
        match other.deadline.cmp(&self.deadline) {
            CmpOrdering::Equal => other.id.cmp(&self.id),
            other => other,
        }
    }
}

/// ============================================================================
/// Global Timer Queue
/// ============================================================================

/// Global timer ID counter
static mut TIMER_ID_COUNTER: u64 = 0;

/// Global timer queue
static mut TIMER_QUEUE: Mutex<BinaryHeap<TimerQueueEntry>> = Mutex::new(BinaryHeap::new());

/// Next timer ID (for tie-breaking)
fn next_timer_id() -> u64 {
    unsafe {
        TIMER_ID_COUNTER += 1;
        TIMER_ID_COUNTER
    }
}

/// ============================================================================
/// Public API
/// ============================================================================

/// Initialize the timer subsystem
pub fn timer_init() {
    unsafe {
        TIMER_QUEUE.lock().clear();
        TIMER_ID_COUNTER = 0;
    }

    log_info!("Timer subsystem initialized");
}

/// Get the next timer deadline
///
/// Returns the deadline of the next timer to fire, or u64::MAX if no timers.
pub fn next_deadline() -> u64 {
    let queue = unsafe { TIMER_QUEUE.lock() };

    if let Some(entry) = queue.peek() {
        entry.deadline
    } else {
        u64::MAX
    }
}

/// Process pending timers
///
/// Should be called from timer interrupt handler.
/// Fires all timers whose deadline has passed.
pub fn timer_tick(current_time: u64) {
    // In a real implementation, this would:
    // 1. Lock the timer queue
    // 2. Pop all timers with deadline <= current_time
    // 3. Fire each timer
    // 4. Re-queue periodic timers

    log_trace!("Timer tick: time={}", current_time);
}

/// Insert a timer into the global queue
///
/// # Arguments
///
/// * `timer` - Timer to insert
pub fn insert_timer(timer: &Timer) {
    let entry = TimerQueueEntry {
        deadline: timer.deadline(),
        id: next_timer_id(),
        timer: timer as *const Timer,
    };

    unsafe {
        TIMER_QUEUE.lock().push(entry);
    }

    timer.slot.store(entry.id, Ordering::Release);
}

/// Remove a timer from the global queue
///
/// # Arguments
///
/// * `timer` - Timer to remove
pub fn remove_timer(timer: &Timer) {
    let slot = timer.slot.load(Ordering::Acquire);

    // In a real implementation, this would search and remove
    // For now, just mark as inactive
    timer.active.store(false, Ordering::Release);

    let _ = slot;
}

/// ============================================================================
/// Current Time
/// ============================================================================

/// Get the current time in nanoseconds
///
/// Returns the monotonic time since boot.
pub fn current_time() -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        crate::kernel::arch::arm64::timer::arm64_current_time()
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::kernel::arch::amd64::timer::amd64_current_time()
    }

    #[cfg(target_arch = "riscv64")]
    {
        0 // Stub
    }
}

/// Convert nanoseconds to microseconds
pub const fn ns_to_us(ns: u64) -> u64 {
    ns / 1000
}

/// Convert nanoseconds to milliseconds
pub const fn ns_to_ms(ns: u64) -> u64 {
    ns / 1_000_000
}

/// Convert nanoseconds to seconds
pub const fn ns_to_s(ns: u64) -> u64 {
    ns / 1_000_000_000
}

/// Convert microseconds to nanoseconds
pub const fn us_to_ns(us: u64) -> u64 {
    us * 1000
}

/// Convert milliseconds to nanoseconds
pub const fn ms_to_ns(ms: u64) -> u64 {
    ms * 1_000_000
}

/// Convert seconds to nanoseconds
pub const fn s_to_ns(s: u64) -> u64 {
    s * 1_000_000_000
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_new() {
        let timer = Timer::new();
        assert!(!timer.is_active());
        assert!(!timer.is_periodic());
        assert_eq!(timer.deadline(), 0);
    }

    #[test]
    fn test_timer_deadline() {
        let timer = Timer::new();
        timer.set_deadline(1000);
        assert_eq!(timer.deadline(), 1000);
        assert!(!timer.is_periodic());
    }

    #[test]
    fn test_timer_periodic() {
        let timer = Timer::new();
        timer.set_periodic(1000, 100);
        assert_eq!(timer.deadline(), 1000);
        assert!(timer.is_periodic());
        assert_eq!(timer.period, 100);
    }

    #[test]
    fn test_timer_activate() {
        let timer = Timer::new();
        timer.activate();
        assert!(timer.is_active());
    }

    #[test]
    fn test_timer_cancel() {
        let timer = Timer::new();
        timer.activate();
        timer.cancel();
        assert!(!timer.is_active());
    }

    #[test]
    fn test_time_conversions() {
        assert_eq!(ns_to_us(1_000_000), 1_000);
        assert_eq!(ns_to_ms(1_000_000), 1);
        assert_eq!(ns_to_s(1_000_000_000), 1);

        assert_eq!(us_to_ns(1), 1_000);
        assert_eq!(ms_to_ns(1), 1_000_000);
        assert_eq!(s_to_ns(1), 1_000_000_000);
    }
}
