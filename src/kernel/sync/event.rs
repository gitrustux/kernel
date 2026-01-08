// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Event
//!
//! This module provides event synchronization primitives for the Rustux kernel.
//! Events allow threads to wait for signals and be woken up when the event
//! is signaled.
//!
//! # Design
//!
//! - **Manual reset**: Event remains signaled until explicitly unsignaled
//! - **Auto reset**: Event automatically resets after waking one waiter
//! - **Fair wake ordering**: Waiters are woken in FIFO order
//!
//! # Usage
//!
//! ```rust
//! let event = Event::new(false, EventFlags::empty());
//!
//! // Wait for the event (blocks until signaled)
//! event.wait();
//!
//! // Signal the event (wakes waiters)
//! event.signal();
//!
//! // Clear the signal
//! event.unsignal();
//! ```


use crate::kernel::thread::ThreadId;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::kernel::sync::spin::SpinMutex;
use crate::kernel::sync::Mutex;
use alloc::vec::Vec;

// Import logging macros
use crate::log_debug;

/// ============================================================================
/// Event Flags
/// ============================================================================

/// Event flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFlags {
    /// Auto-unsignal after waking one thread
    pub auto_unsignal: bool,

    _reserved: u32,
}

impl EventFlags {
    /// No flags
    pub const fn empty() -> Self {
        Self {
            auto_unsignal: false,
            _reserved: 0,
        }
    }

    /// Auto unsignal flag
    pub const fn auto_unsignal() -> Self {
        Self {
            auto_unsignal: true,
            _reserved: 0,
        }
    }

    /// Convert to raw value
    pub const fn into_raw(self) -> u32 {
        let mut bits = 0u32;
        if self.auto_unsignal {
            bits |= 0x01;
        }
        bits
    }

    /// Convert from raw value
    pub const fn from_raw(bits: u32) -> Self {
        Self {
            auto_unsignal: (bits & 0x01) != 0,
            _reserved: 0,
        }
    }
}

/// ============================================================================
/// Event
/// ============================================================================

/// Magic number for event validation
const EVENT_MAGIC: u32 = 0x45564E54; // "EVNT" in hex

/// Event synchronization primitive
///
/// Threads can wait on events and be woken when the event is signaled.
pub struct Event {
    /// Whether the event is currently signaled
    signaled: AtomicBool,

    /// Event flags
    flags: AtomicU32,

    /// Magic number for validation
    magic: u32,

    /// Threads waiting on this event
    waiters: Mutex<Vec<ThreadId>>,
}

impl Event {
    /// Create a new event
    ///
    /// # Arguments
    ///
    /// * `initial` - Initial signaled state
    /// * `flags` - Event flags
    pub const fn new(initial: bool, flags: EventFlags) -> Self {
        Self {
            signaled: AtomicBool::new(initial),
            flags: AtomicU32::new(flags.into_raw()),
            magic: EVENT_MAGIC,
            waiters: Mutex::new(Vec::new()),
        }
    }

    /// Initialize an event (for heap-allocated events)
    pub fn init(&mut self, initial: bool, flags: EventFlags) {
        self.signaled.store(initial, Ordering::Release);
        self.flags.store(flags.into_raw(), Ordering::Release);
        self.magic = EVENT_MAGIC;
        self.waiters.lock().clear();
    }

    /// Destroy an event
    ///
    /// Panics if there are threads still waiting.
    pub fn destroy(&self) {
        self.validate();

        if !self.waiters.lock().is_empty() {
            panic!("event_destroy: threads still waiting");
        }

        // Clear magic to mark as destroyed
        // Note: This is a const operation issue - in real code we'd use interior mutability
    }

    /// Wait for the event to be signaled
    ///
    /// If already signaled, returns immediately.
    /// Otherwise, blocks until signaled.
    pub fn wait(&self) {
        self.wait_deadline(u64::MAX);
    }

    /// Wait for the event with a deadline
    ///
    /// # Arguments
    ///
    /// * `deadline` - Deadline in nanoseconds (u64::MAX = infinite)
    ///
    /// # Returns
    ///
    /// - `Ok(())` if signaled
    /// - `Err(RX_ERR_TIMED_OUT)` if deadline reached
    pub fn wait_deadline(&self, deadline: u64) -> Result {
        self.validate();

        // Fast path: already signaled
        if self.is_signaled() {
            // Auto-unsignal if flag is set
            if self.flags().auto_unsignal {
                self.signaled.store(false, Ordering::Release);
            }
            return Ok(());
        }

        // Slow path: need to wait
        let current_tid = ThreadId::current(); // Stub

        // Add to wait queue
        {
            let mut waiters = self.waiters.lock();
            waiters.push(current_tid);
        }

        // Block current thread
        // In a real implementation, this would integrate with scheduler
        // and check the deadline
        log_debug!("Thread {} waiting on event", current_tid);

        // TODO: Block on wait queue with deadline
        // crate::kernel::sched::block_current(BlockReason::ChannelRead);

        // Check if we timed out
        let now = self.current_time();
        if now >= deadline {
            return Err(RX_ERR_TIMED_OUT);
        }

        Ok(())
    }

    /// Signal the event
    ///
    /// Wakes up waiting threads. If auto_unsignal is set,
    /// only one thread is woken and the event is cleared.
    /// Otherwise, all waiters are woken.
    ///
    /// # Returns
    ///
    /// Number of threads woken
    pub fn signal(&self) -> i32 {
        self.validate();
        self.signal_internal(false)
    }

    /// Signal the event and reschedule
    ///
    /// Same as signal() but forces a reschedule.
    pub fn signal_and_reschedule(&self) -> i32 {
        self.validate();
        self.signal_internal(true)
    }

    /// Clear the signaled state
    pub fn unsignal(&self) {
        self.validate();
        self.signaled.store(false, Ordering::Release);
    }

    /// Check if the event is signaled
    pub fn is_signaled(&self) -> bool {
        self.signaled.load(Ordering::Acquire)
    }

    /// Get the event flags
    pub fn flags(&self) -> EventFlags {
        EventFlags::from_raw(self.flags.load(Ordering::Acquire))
    }

    /// Internal signal implementation
    fn signal_internal(&self, reschedule: bool) -> i32 {
        // Already signaled - nothing to do
        if self.signaled.load(Ordering::Acquire) {
            return 0;
        }

        let flags = self.flags();
        let mut wake_count = 0;

        if flags.auto_unsignal {
            // Try to wake one thread
            let tid = {
                let mut waiters = self.waiters.lock();
                if waiters.is_empty() {
                    None
                } else {
                    Some(waiters.remove(0))
                }
            };

            if let Some(tid) = tid {
                // Wake the thread
                log_debug!("Waking thread {} for event", tid);
                // TODO: crate::kernel::sched::wake(tid);
                wake_count = 1;
            } else {
                // No threads to wake, set signaled state
                self.signaled.store(true, Ordering::Release);
            }
        } else {
            // Wake all threads
            let waiters = {
                let mut w = self.waiters.lock();
                core::mem::take(&mut *w)
            };

            for tid in waiters {
                log_debug!("Waking thread {} for event", tid);
                // TODO: crate::kernel::sched::wake(tid);
                wake_count += 1;
            }

            // Set signaled state
            self.signaled.store(true, Ordering::Release);
        }

        // Reschedule if requested
        if reschedule && wake_count > 0 {
            // TODO: crate::kernel::sched::yield_current();
        }

        wake_count
    }

    /// Validate that this is a valid event
    fn validate(&self) {
        debug_assert_eq!(self.magic, EVENT_MAGIC, "invalid event magic");
    }

    /// Get current time (for deadline checking)
    fn current_time(&self) -> u64 {
        // Stub implementation
        // In real code, this would call into the timer subsystem
        #[cfg(target_arch = "aarch64")]
        {
            crate::arch::arm64::timer::arm64_current_time()
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
}

/// ============================================================================
/// ThreadId Extension
/// ============================================================================

pub trait ThreadIdExt {
    fn current() -> Self;
}

impl ThreadIdExt for ThreadId {
    fn current() -> Self {
        // Stub implementation
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
    fn test_event_new() {
        let event = Event::new(false, EventFlags::empty());
        assert!(!event.is_signaled());
        assert_eq!(event.flags().into_raw(), 0);
    }

    #[test]
    fn test_event_signaled() {
        let event = Event::new(true, EventFlags::empty());
        assert!(event.is_signaled());
    }

    #[test]
    fn test_event_unsignal() {
        let event = Event::new(true, EventFlags::empty());
        assert!(event.is_signaled());

        event.unsignal();
        assert!(!event.is_signaled());
    }

    #[test]
    fn test_event_signal() {
        let event = Event::new(false, EventFlags::empty());

        let count = event.signal();
        assert_eq!(count, 0); // No waiters
        assert!(event.is_signaled());
    }

    #[test]
    fn test_event_flags() {
        let flags = EventFlags::auto_unsignal();
        assert!(flags.auto_unsignal);

        let raw = flags.into_raw();
        assert_eq!(raw, 0x01);

        let flags2 = EventFlags::from_raw(raw);
        assert_eq!(flags2.auto_unsignal, flags.auto_unsignal);
    }

    #[test]
    fn test_event_magic() {
        let event = Event::new(false, EventFlags::empty());
        assert_eq!(event.magic, EVENT_MAGIC);
    }
}
