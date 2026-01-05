// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Event Objects
//!
//! Events are simple synchronization primitives that can be signaled
//! and waited upon. They support both auto-reset and manual-reset modes.
//!
//! # Design
//!
//! - **Simple signaling**: Binary state (signaled/not signaled)
//! - **Auto-reset**: Automatically clears when a waiter wakes
//! - **Manual-reset**: Remains signaled until explicitly cleared
//! - **Wait queues**: Multiple threads can wait on same event
//!
//! # Usage
//!
//! ```rust
//! let event = Event::new(false, EventFlags::MANUAL_RESET)?;
//! event.signal();
//! event.wait()?;
//! event.unsignal();
//! ```

#![no_std]

use crate::kernel::sync::wait_queue::WaitQueue;
use crate::kernel::sync::Mutex;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use crate::kernel::sync::spin::SpinMutex;

/// ============================================================================
/// Event ID
/// ============================================================================

/// Event identifier
pub type EventId = u64;

/// Next event ID counter
static mut NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new event ID
fn alloc_event_id() -> EventId {
    unsafe { NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed) }
}

/// ============================================================================
/// Event Flags
/// ============================================================================

/// Event creation flags
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFlags(pub u32);

impl EventFlags {
    /// No flags
    pub const empty: Self = Self(0);

    /// Manual reset (stays signaled until explicitly cleared)
    pub const MANUAL_RESET: Self = Self(0x01);

    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self.0
    }

    /// Check if manual reset
    pub const fn is_manual_reset(self) -> bool {
        (self.0 & Self::MANUAL_RESET.0) != 0
    }
}

/// ============================================================================
/// Event
/// ============================================================================

/// Event object
///
/// A simple synchronization primitive for signaling between threads.
pub struct Event {
    /// Event ID
    pub id: EventId,

    /// Current signal state
    pub signaled: AtomicBool,

    /// Event flags
    pub flags: EventFlags,

    /// Wait queue for blocked threads
    pub waiters: Mutex<WaitQueue>,

    /// Reference count
    pub ref_count: AtomicUsize,
}

impl Event {
    /// Create a new event
    ///
    /// # Arguments
    ///
    /// * `signaled` - Initial signal state
    /// * `flags` - Event flags
    pub const fn new(signaled: bool, flags: EventFlags) -> Self {
        Self {
            id: alloc_event_id(),
            signaled: AtomicBool::new(signaled),
            flags,
            waiters: Mutex::new(WaitQueue::new()),
            ref_count: AtomicUsize::new(1),
        }
    }

    /// Get event ID
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// Check if event is signaled
    pub fn is_signaled(&self) -> bool {
        self.signaled.load(Ordering::Acquire)
    }

    /// Signal the event
    ///
    /// Wakes up all waiting threads.
    pub fn signal(&self) {
        self.signaled.store(true, Ordering::Release);

        // Wake all waiters
        let mut waiters = self.waiters.lock();
        waiters.wake_all();
    }

    /// Signal the event and trigger reschedule
    ///
    /// Like signal(), but also triggers scheduler to run waiting threads.
    pub fn signal_and_reschedule(&self) {
        self.signal();

        // TODO: Trigger scheduler
    }

    /// Unsignal the event
    ///
    /// Clears the signal state. Only meaningful for manual-reset events.
    pub fn unsignal(&self) {
        self.signaled.store(false, Ordering::Release);
    }

    /// Wait for the event to be signaled
    ///
    /// # Arguments
    ///
    /// * `deadline` - Optional deadline in nanoseconds (None = wait forever)
    ///
    /// # Returns
    ///
    /// - Ok(()) if event was signaled
    /// - Err(RX_ERR_TIMED_OUT) if deadline expired
    pub fn wait(&self, deadline: Option<u64>) -> Result {
        // Fast path: already signaled
        if self.is_signaled() {
            // Auto-reset: clear signal
            if !self.flags.is_manual_reset() {
                self.signaled.store(false, Ordering::Release);
            }
            return Ok(());
        }

        // Slow path: need to wait
        // In a real implementation, this would block the current thread
        // For now, return error to indicate not yet implemented
        Err(RX_ERR_NOT_SUPPORTED)
    }

    /// Wait with absolute deadline
    ///
    /// # Arguments
    ///
    /// * `deadline` - Absolute deadline in nanoseconds
    pub fn wait_until(&self, deadline: u64) -> Result {
        self.wait(Some(deadline))
    }

    /// Increment reference count
    pub fn ref_inc(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference.
    pub fn ref_dec(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Release) == 1
    }
}

/// ============================================================================
/// Event Pair
/// ============================================================================

/// EventPair ID
pub type EventPairId = u64;

/// Next event pair ID counter
static mut NEXT_EVENTPAIR_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new event pair ID
fn alloc_eventpair_id() -> EventPairId {
    unsafe { NEXT_EVENTPAIR_ID.fetch_add(1, Ordering::Relaxed) }
}

/// Event pair
///
/// A pair of events that signal each other.
/// When one event is signaled, the other becomes unsignaled.
pub struct EventPair {
    /// Event pair ID
    pub id: EventPairId,

    /// First event
    pub event_a: Event,

    /// Second event
    pub event_b: Event,

    /// Peer reference (for notification)
    pub peer: AtomicUsize,

    /// Reference count
    pub ref_count: AtomicUsize,
}

impl EventPair {
    /// Create an event pair
    ///
    /// Returns a pair of event IDs.
    pub fn create() -> Result<(Self, Self)> {
        let id_a = alloc_eventpair_id();
        let id_b = alloc_eventpair_id();

        let pair_a = Self {
            id: id_a,
            event_a: Event::new(false, EventFlags::empty),
            event_b: Event::new(false, EventFlags::empty),
            peer: AtomicUsize::new(id_b as usize),
            ref_count: AtomicUsize::new(1),
        };

        let pair_b = Self {
            id: id_b,
            event_a: Event::new(false, EventFlags::empty),
            event_b: Event::new(false, EventFlags::empty),
            peer: AtomicUsize::new(id_a as usize),
            ref_count: AtomicUsize::new(1),
        };

        Ok((pair_a, pair_b))
    }

    /// Signal this event (and unsignal peer)
    pub fn signal(&self) {
        // Signal our event
        self.event_a.signal();

        // Unsignal peer's event
        // In real implementation, would notify peer
    }

    /// Wait for this event to be signaled
    pub fn wait(&self) -> Result {
        self.event_a.wait(None)
    }

    /// Increment reference count
    pub fn ref_inc(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference.
    pub fn ref_dec(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Release) == 1
    }
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_flags() {
        let flags = EventFlags::MANUAL_RESET;
        assert!(flags.is_manual_reset());

        let empty = EventFlags::empty();
        assert!(!empty.is_manual_reset());
    }

    #[test]
    fn test_event_create() {
        let event = Event::new(false, EventFlags::empty());
        assert!(!event.is_signaled());

        let event = Event::new(true, EventFlags::MANUAL_RESET);
        assert!(event.is_signaled());
    }

    #[test]
    fn test_event_signal() {
        let event = Event::new(false, EventFlags::empty());
        assert!(!event.is_signaled());

        event.signal();
        assert!(event.is_signaled());

        event.unsignal();
        assert!(!event.is_signaled());
    }

    #[test]
    fn test_eventpair_create() {
        let (pair_a, pair_b) = EventPair::create().unwrap();
        assert_ne!(pair_a.id, pair_b.id);
    }
}
