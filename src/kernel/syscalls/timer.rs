// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Timer System Calls
//!
//! This module implements the Timer system calls for high-resolution timers.
//! Timers support one-shot and periodic operation with configurable slack.
//!
//! # Syscalls Implemented
//!
//! - `rx_timer_create` - Create a timer
//! - `rx_timer_set` - Set timer deadline
//! - `rx_timer_cancel` - Cancel a timer
//!
//! # Design
//!
//! - High-resolution: nanosecond precision
//! - One-shot and periodic modes
//! - Configurable slack for power efficiency
//! - Event-based signaling

#![no_std]

use crate::kernel::object::timer::{self, Timer, TimerState};
use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use crate::kernel::sync::Mutex;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Timer Registry
/// ============================================================================

/// Maximum number of timers in the system
const MAX_TIMERS: usize = 65536;

/// Timer registry entry
struct TimerEntry {
    /// Timer ID
    id: timer::TimerId,

    /// Timer object
    timer: Arc<Timer>,
}

/// Global timer registry
///
/// Maps timer IDs to timer objects. This is used to resolve handles to timers.
struct TimerRegistry {
    /// Timer entries
    entries: [Option<TimerEntry>; MAX_TIMERS],

    /// Next timer index to allocate
    next_index: AtomicUsize,

    /// Number of active timers
    count: AtomicUsize,
}

impl TimerRegistry {
    /// Create a new timer registry
    const fn new() -> Self {
        const INIT: Option<TimerEntry> = None;

        Self {
            entries: [INIT; MAX_TIMERS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert a timer into the registry
    pub fn insert(&mut self, timer: Arc<Timer>) -> Result<timer::TimerId> {
        let id = timer.id;

        // Find a free slot
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (id as usize) % MAX_TIMERS;

        loop {
            // Try to allocate at current index
            if self.entries[idx].is_none() {
                self.entries[idx] = Some(TimerEntry { id, timer });
                self.count.fetch_add(1, Ordering::Relaxed);
                self.next_index.store((idx + 1) % MAX_TIMERS, Ordering::Relaxed);
                return Ok(id);
            }

            // Linear probe
            idx = (idx + 1) % MAX_TIMERS;

            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    /// Get a timer from the registry
    pub fn get(&self, id: timer::TimerId) -> Option<Arc<Timer>> {
        let idx = (id as usize) % MAX_TIMERS;

        self.entries[idx]
            .as_ref()
            .filter(|entry| entry.id == id)
            .map(|entry| entry.timer.clone())
    }

    /// Remove a timer from the registry
    pub fn remove(&mut self, id: timer::TimerId) -> Option<Arc<Timer>> {
        let idx = (id as usize) % MAX_TIMERS;

        if let Some(entry) = self.entries[idx].take() {
            if entry.id == id {
                self.count.fetch_sub(1, Ordering::Relaxed);
                return Some(entry.timer);
            }
        }

        None
    }

    /// Get the number of active timers
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

impl Default for TimerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global timer registry
static TIMER_REGISTRY: Mutex<TimerRegistry> = Mutex::new(TimerRegistry::new());

/// ============================================================================
/// Handle to Timer Resolution
/// ============================================================================

/// Get the current process's handle table
///
/// This is a placeholder that returns NULL for now.
/// In a real implementation, this would use thread-local storage
/// or per-CPU data to get the current process.
fn current_process_handle_table() -> Option<&'static HandleTable> {
    // TODO: Implement proper current process tracking
    // For now, return None to indicate not implemented
    None
}

/// Look up a timer from a handle value
///
/// This function:
/// 1. Gets the current process's handle table
/// 2. Looks up the handle in the table
/// 3. Validates the handle type and rights
/// 4. Returns the timer object
fn lookup_timer_from_handle(
    handle_val: u32,
    required_rights: Rights,
) -> Result<(Arc<Timer>, Handle)> {
    // Get current process handle table
    let handle_table = current_process_handle_table()
        .ok_or(RX_ERR_NOT_SUPPORTED)?;

    // Get the handle from the table
    let handle = handle_table.get(handle_val)
        .ok_or(RX_ERR_INVALID_ARGS)?;

    // Validate object type
    if handle.obj_type() != ObjectType::Timer {
        return Err(RX_ERR_WRONG_TYPE);
    }

    // Validate rights
    handle.require(required_rights)?;

    // Get timer ID from handle (stored as part of base pointer for now)
    let timer_id = handle.id as timer::TimerId;

    // Get timer from registry
    let timer = TIMER_REGISTRY.lock().get(timer_id)
        .ok_or(RX_ERR_NOT_FOUND)?;

    Ok((timer, handle))
}

/// ============================================================================
/// Timer Kernel Object Base
/// ============================================================================

/// Create a kernel object base for a timer
fn timer_to_kernel_base(timer: &Arc<Timer>) -> KernelObjectBase {
    KernelObjectBase::new(ObjectType::Timer)
}

/// ============================================================================
/// Syscall: Timer Create
/// ============================================================================

/// Clock ID for monotonic clock
const CLOCK_MONOTONIC: u32 = 0;

/// Create a new timer syscall handler
///
/// # Arguments
///
/// * `options` - Timer creation options (must be 0)
/// * `clock_id` - Clock ID (must be CLOCK_MONOTONIC)
///
/// # Returns
///
/// * On success: Handle value for the new timer
/// * On error: Negative error code
pub fn sys_timer_create_impl(options: u32, clock_id: u32) -> SyscallRet {
    log_debug!("sys_timer_create: options={:#x} clock_id={}", options, clock_id);

    // Validate options (must be 0)
    if options != 0 {
        log_error!("sys_timer_create: invalid options {:#x}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate clock_id (must be monotonic)
    if clock_id != CLOCK_MONOTONIC {
        log_error!("sys_timer_create: invalid clock_id {}", clock_id);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Create the timer
    let timer = match Timer::create() {
        Ok(t) => t,
        Err(err) => {
            log_error!("sys_timer_create: failed to create timer: {:?}", err);
            return err_to_ret(err);
        }
    };

    log_debug!("sys_timer_create: created timer id={}", timer.id);

    // Wrap in Arc for registry
    let timer_arc = Arc::new(timer);

    // Insert into timer registry
    let timer_id = match TIMER_REGISTRY.lock().insert(timer_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_timer_create: failed to insert timer into registry: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Create kernel object base
    let base = timer_to_kernel_base(&timer_arc);

    // Create handle with default rights (WRITE | SIGNAL)
    let rights = Rights::WRITE | Rights::SIGNAL;
    let handle = Handle::new(&base as *const KernelObjectBase, rights);

    // TODO: Add handle to current process's handle table
    // For now, return the timer ID as the handle value
    let handle_value = timer_id as u32;

    log_debug!("sys_timer_create: success handle={}", handle_value);

    ok_to_ret(handle_value as usize)
}

/// ============================================================================
/// Syscall: Timer Set
/// ============================================================================

/// Set timer syscall handler
///
/// # Arguments
///
/// * `handle_val` - Timer handle value
/// * `deadline` - Absolute deadline in nanoseconds
/// * `slack` - Slack duration in nanoseconds (must be >= 0)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_timer_set_impl(handle_val: u32, deadline: u64, slack: i64) -> SyscallRet {
    log_debug!(
        "sys_timer_set: handle={} deadline={} slack={}",
        handle_val, deadline, slack
    );

    // Validate slack (must be >= 0)
    if slack < 0 {
        log_error!("sys_timer_set: invalid slack {}", slack);
        return err_to_ret(RX_ERR_OUT_OF_RANGE);
    }

    // Look up timer from handle (requires WRITE right)
    let (timer, _handle) = match lookup_timer_from_handle(handle_val, Rights::WRITE) {
        Ok(t) => t,
        Err(err) => {
            log_error!("sys_timer_set: failed to lookup timer: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Set timer (convert slack to Option<u64>)
    let slack_option = if slack >= 0 { Some(slack as u64) } else { None };

    match timer.set(deadline, slack_option) {
        Ok(()) => {
            log_debug!("sys_timer_set: success");
            ok_to_ret(0)
        }
        Err(err) => {
            log_error!("sys_timer_set: timer set failed: {:?}", err);
            err_to_ret(err)
        }
    }
}

/// ============================================================================
/// Syscall: Timer Cancel
/// ============================================================================

/// Cancel timer syscall handler
///
/// # Arguments
///
/// * `handle_val` - Timer handle value
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_timer_cancel_impl(handle_val: u32) -> SyscallRet {
    log_debug!("sys_timer_cancel: handle={}", handle_val);

    // Look up timer from handle (requires WRITE right)
    let (timer, _handle) = match lookup_timer_from_handle(handle_val, Rights::WRITE) {
        Ok(t) => t,
        Err(err) => {
            log_error!("sys_timer_cancel: failed to lookup timer: {:?}", err);
            return err_to_ret(err);
        }
    };

    match timer.cancel() {
        Ok(()) => {
            log_debug!("sys_timer_cancel: success");
            ok_to_ret(0)
        }
        Err(err) => {
            // Some errors are expected (e.g., timer not armed)
            // Map BAD_STATE to OK for compatibility with Zircon semantics
            if err == RX_ERR_BAD_STATE {
                log_debug!("sys_timer_cancel: timer not armed (ok)");
                ok_to_ret(0)
            } else {
                log_error!("sys_timer_cancel: timer cancel failed: {:?}", err);
                err_to_ret(err)
            }
        }
    }
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get timer subsystem statistics
pub fn get_stats() -> TimerStats {
    TimerStats {
        total_timers: TIMER_REGISTRY.lock().count(),
        armed_timers: 0, // TODO: Track armed timers
        fired_count: 0,  // TODO: Track fired timers
    }
}

/// Timer subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimerStats {
    /// Total number of timers
    pub total_timers: usize,

    /// Number of armed timers
    pub armed_timers: usize,

    /// Number of timers that have fired
    pub fired_count: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the timer syscall subsystem
pub fn init() {
    log_info!("Timer syscall subsystem initialized");
    log_info!("  Max timers: {}", MAX_TIMERS);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_registry_insert_get() {
        let timer = Timer::create().unwrap();
        let timer_arc = Arc::new(timer);

        let id = TIMER_REGISTRY.lock().insert(timer_arc.clone()).unwrap();
        assert_eq!(id, timer_arc.id);

        let retrieved = TIMER_REGISTRY.lock().get(id).unwrap();
        assert_eq!(retrieved.id, timer_arc.id);
    }

    #[test]
    fn test_timer_registry_remove() {
        let timer = Timer::create().unwrap();
        let timer_arc = Arc::new(timer);

        let id = TIMER_REGISTRY.lock().insert(timer_arc.clone()).unwrap();
        let removed = TIMER_REGISTRY.lock().remove(id).unwrap();

        assert_eq!(removed.id, timer_arc.id);
        assert!(TIMER_REGISTRY.lock().get(id).is_none());
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
    fn test_timer_cancel() {
        let timer = Timer::create().unwrap();
        timer.set(1_000_000, None).unwrap();

        assert!(timer.cancel().is_ok());
        assert_eq!(timer.state(), TimerState::Canceled);
    }

    #[test]
    fn test_timer_set_periodic() {
        let timer = Timer::create().unwrap();
        timer.set_periodic(1_000_000, 100_000, None).unwrap();

        assert_eq!(timer.state(), TimerState::Armed);
        assert_eq!(timer.period(), Some(100_000));
    }

    #[test]
    fn test_slack_validation() {
        // Valid slack values
        assert!(sys_timer_set_impl(0, 1000, 0) >= 0);     // slack = 0
        assert!(sys_timer_set_impl(0, 1000, 1000) >= 0);  // slack = 1000

        // Invalid slack (negative)
        assert!(sys_timer_set_impl(0, 1000, -1) < 0);      // negative slack
    }

    #[test]
    fn test_clock_id_validation() {
        // Valid clock ID
        assert!(sys_timer_create_impl(0, CLOCK_MONOTONIC) >= 0);

        // Invalid clock ID
        assert!(sys_timer_create_impl(0, 999) < 0);
    }
}
