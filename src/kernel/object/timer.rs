// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Timer Objects
//!
//! Timer objects provide high-resolution timers for user-space processes.
//! They support one-shot and periodic timers.
//!
//! # Design
//!
//! - **High-resolution**: Nanosecond precision
//! - **One-shot**: Fire once at specified deadline
//! - **Periodic**: Fire repeatedly at specified interval
//! - **Slack**: Allow coalescing for power efficiency
//!
//! # Usage
//!
//! ```rust
//! let timer = Timer::create()?;
//! timer.set(deadline, slack)?;
//! timer.wait()?;
//! ```


use crate::kernel::sync::event::{Event, EventFlags};
use crate::kernel::sync::Mutex;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};

/// ============================================================================
/// Timer ID
/// ============================================================================

/// Timer identifier
pub type TimerId = u64;

/// Next timer ID counter
static mut NEXT_TIMER_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new timer ID
fn alloc_timer_id() -> TimerId {
    unsafe { NEXT_TIMER_ID.fetch_add(1, Ordering::Relaxed) }
}

/// ============================================================================
/// Timer State
/// ============================================================================

/// Timer state
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    /// Timer is disarmed
    Disarmed = 0,

    /// Timer is armed (waiting for deadline)
    Armed = 1,

    /// Timer has fired
    Fired = 2,

    /// Timer was canceled
    Canceled = 3,
}

impl TimerState {
    /// Create from raw value
    pub const fn from_raw(raw: u8) -> Self {
        match raw {
            1 => Self::Armed,
            2 => Self::Fired,
            3 => Self::Canceled,
            _ => Self::Disarmed,
        }
    }

    /// Get raw value
    pub const fn into_raw(self) -> u8 {
        self as u8
    }
}

/// ============================================================================
/// Slack Policy
/// ============================================================================

/// Timer slack policy
///
/// Determines how much the timer deadline can be adjusted for power efficiency.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackPolicy {
    /// No slack (precise timing)
    None = 0,

    /// Small slack (default)
    Small = 1,

    /// Medium slack
    Medium = 2,

    /// Large slack (maximum coalescing)
    Large = 3,
}

impl SlackPolicy {
    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Small,
            2 => Self::Medium,
            3 => Self::Large,
            _ => Self::None,
        }
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self as u32
    }

    /// Get slack duration in nanoseconds
    pub const fn duration(self) -> u64 {
        match self {
            Self::None => 0,
            Self::Small => 100_000,       // 100us
            Self::Medium => 1_000_000,    // 1ms
            Self::Large => 10_000_000,    // 10ms
        }
    }
}

/// ============================================================================
/// Timer
/// ============================================================================

/// Timer object
///
/// Provides high-resolution timer functionality.
pub struct Timer {
    /// Timer ID
    pub id: TimerId,

    /// Timer deadline (in nanoseconds)
    pub deadline: AtomicU64,

    /// Timer slack (in nanoseconds)
    pub slack: AtomicU64,

    /// Timer period (None = one-shot, Some = periodic)
    pub period: Mutex<Option<NonZeroU64>>,

    /// Timer state
    pub state: AtomicU8,

    /// Event signaled when timer fires
    pub event: Event,

    /// Slack policy
    pub slack_policy: Mutex<SlackPolicy>,

    /// Reference count
    pub ref_count: AtomicUsize,
}

impl Timer {
    /// Create a new timer
    ///
    /// Initially disarmed.
    pub fn create() -> Result<Self> {
        Ok(Self {
            id: alloc_timer_id(),
            deadline: AtomicU64::new(0),
            slack: AtomicU64::new(0),
            period: Mutex::new(None),
            state: AtomicU8::new(TimerState::Disarmed as u8),
            event: Event::new(false, EventFlags::empty()),
            slack_policy: Mutex::new(SlackPolicy::Small),
            ref_count: AtomicUsize::new(1),
        })
    }

    /// Get timer ID
    pub const fn id(&self) -> TimerId {
        self.id
    }

    /// Get timer state
    pub fn state(&self) -> TimerState {
        TimerState::from_raw(self.state.load(Ordering::Acquire))
    }

    /// Set the timer
    ///
    /// # Arguments
    ///
    /// * `deadline` - Absolute deadline in nanoseconds
    /// * `slack` - Optional slack duration in nanoseconds
    ///
    /// If the timer is already armed, this cancels the previous deadline.
    pub fn set(&self, deadline: u64, slack: Option<u64>) -> Result {
        // Update deadline and slack
        self.deadline.store(deadline, Ordering::Release);
        self.slack.store(slack.unwrap_or(0), Ordering::Release);

        // Update state
        self.state.store(TimerState::Armed as u8, Ordering::Release);

        // Unsignal event
        self.event.unsignal();

        // TODO: Add to global timer queue

        Ok(())
    }

    /// Set a periodic timer
    ///
    /// # Arguments
    ///
    /// * `deadline` - First deadline in nanoseconds
    /// * `period` - Period in nanoseconds
    /// * `slack` - Optional slack duration in nanoseconds
    pub fn set_periodic(&self, deadline: u64, period: u64, slack: Option<u64>) -> Result {
        if period == 0 {
            return Err(RX_ERR_INVALID_ARGS);
        }

        // Set period
        *self.period.lock() = Some(NonZeroU64::new(period).unwrap());

        // Set timer
        self.set(deadline, slack)
    }

    /// Cancel the timer
    ///
    /// # Returns
    ///
    /// - Ok(()) if timer was canceled
    /// - Err(RX_ERR_BAD_STATE) if timer has already fired
    pub fn cancel(&self) -> Result {
        let state = self.state();

        match state {
            TimerState::Disarmed | TimerState::Canceled => {
                return Err(RX_ERR_BAD_STATE);
            }
            TimerState::Fired => {
                return Err(RX_ERR_BAD_STATE);
            }
            TimerState::Armed => {
                // Cancel timer
                self.state.store(TimerState::Canceled as u8, Ordering::Release);

                // TODO: Remove from global timer queue

                // Unsignal event
                self.event.unsignal();

                Ok(())
            }
        }
    }

    /// Wait for timer to fire
    ///
    /// # Returns
    ///
    /// - Ok(()) if timer fired
    /// - Err(RX_ERR_TIMED_OUT) if wait was interrupted
    /// - Err(RX_ERR_CANCELED) if timer was canceled
    pub fn wait(&self) -> Result {
        // Wait on event
        self.event.wait();

        // Check if timer was canceled
        if self.state() == TimerState::Canceled {
            return Err(RX_ERR_CANCELED);
        }

        Ok(())
    }

    /// Get current deadline
    pub fn deadline(&self) -> u64 {
        self.deadline.load(Ordering::Acquire)
    }

    /// Get current slack
    pub fn slack(&self) -> u64 {
        self.slack.load(Ordering::Acquire)
    }

    /// Get current period
    pub fn period(&self) -> Option<u64> {
        self.period.lock().map(|p| p.get())
    }

    /// Set slack policy
    pub fn set_slack_policy(&self, policy: SlackPolicy) {
        *self.slack_policy.lock() = policy;
    }

    /// Get slack policy
    pub fn slack_policy(&self) -> SlackPolicy {
        *self.slack_policy.lock()
    }

    /// Handle timer fire
    ///
    /// Called by timer subsystem when deadline is reached.
    pub fn on_fire(&self) {
        // Update state
        self.state.store(TimerState::Fired as u8, Ordering::Release);

        // Signal event
        self.event.signal();

        // If periodic, reschedule
        if let Some(period) = self.period() {
            let new_deadline = self.deadline() + period;
            self.deadline.store(new_deadline, Ordering::Release);
            self.state.store(TimerState::Armed as u8, Ordering::Release);

            // Unsignal event for next period
            self.event.unsignal();

            // TODO: Re-add to timer queue
        }
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
    fn test_timer_state() {
        let state = TimerState::Armed;
        assert_eq!(TimerState::from_raw(1), state);
        assert_eq!(state.into_raw(), 1);
    }

    #[test]
    fn test_timer_create() {
        let timer = Timer::create().unwrap();
        assert_eq!(timer.state(), TimerState::Disarmed);
        assert_eq!(timer.period(), None);
    }

    #[test]
    fn test_timer_set() {
        let timer = Timer::create().unwrap();
        timer.set(1_000_000, Some(1000)).unwrap();

        assert_eq!(timer.state(), TimerState::Armed);
        assert_eq!(timer.deadline(), 1_000_000);
        assert_eq!(timer.slack(), 1000);
    }

    #[test]
    fn test_timer_set_periodic() {
        let timer = Timer::create().unwrap();
        timer.set_periodic(1_000_000, 100_000, None).unwrap();

        assert_eq!(timer.state(), TimerState::Armed);
        assert_eq!(timer.period(), Some(100_000));
    }

    #[test]
    fn test_slack_policy() {
        let policy = SlackPolicy::Medium;
        assert_eq!(SlackPolicy::from_raw(2), policy);
        assert_eq!(policy.into_raw(), 2);
        assert_eq!(policy.duration(), 1_000_000);
    }
}
