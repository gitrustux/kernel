// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Watchdog Timer
//!
//! This module provides watchdog timer functionality for detecting system hangs.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Watchdog magic number for validation
pub const WATCHDOG_MAGIC: u32 = 0x57415447; // "WTG" in hex

/// Default watchdog timeout in milliseconds
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

/// Watchdog structure
#[repr(C)]
pub struct Watchdog {
    /// Magic number for validation
    pub magic: u32,
    /// Watchdog name
    pub name: String,
    /// Whether the watchdog is enabled
    pub enabled: bool,
    /// Timeout in nanoseconds
    pub timeout: u64,
    /// Last pet time (nanoseconds since boot)
    last_pet: AtomicU64,
}

impl Watchdog {
    /// Create a new watchdog
    pub fn new(timeout_ms: u64, name: &str) -> Self {
        Self {
            magic: WATCHDOG_MAGIC,
            name: name.to_string(),
            enabled: false,
            timeout: timeout_ms * 1_000_000, // Convert to nanoseconds
            last_pet: AtomicU64::new(0),
        }
    }

    /// Check if the watchdog has a valid magic number
    pub fn is_valid(&self) -> bool {
        self.magic == WATCHDOG_MAGIC
    }

    /// Initialize the watchdog
    pub fn init(&mut self, timeout_ms: u64, name: &str) {
        self.magic = WATCHDOG_MAGIC;
        self.name = name.to_string();
        self.enabled = false;
        self.timeout = timeout_ms * 1_000_000;
    }

    /// Enable or disable the watchdog
    pub fn set_enabled(&mut self, enabled: bool) {
        if !self.is_valid() {
            return;
        }

        if self.enabled == enabled {
            return;
        }

        self.enabled = enabled;

        if enabled {
            // Start the watchdog timer
            self.pet();
        } else {
            // Cancel the watchdog timer
            // TODO: Implement timer_cancel
        }
    }

    /// Pet the watchdog (reset the timer)
    pub fn pet(&mut self) {
        if !self.is_valid() || !self.enabled {
            return;
        }

        // Update last pet time
        self.last_pet.store(get_current_time_ns(), Ordering::Release);

        // Reset the timer
        // TODO: Reset hardware watchdog timer
    }

    /// Check if the watchdog has expired
    pub fn check_expired(&self) -> bool {
        if !self.is_valid() || !self.enabled {
            return false;
        }

        let now = get_current_time_ns();
        let last_pet = self.last_pet.load(Ordering::Acquire);

        // Check if timeout has elapsed
        now.saturating_sub(last_pet) > self.timeout
    }
}

/// Hardware watchdog state
struct HwWatchdogState {
    /// Whether hardware watchdog is enabled
    enabled: bool,
    /// Pet timeout (nanoseconds)
    pet_timeout: u64,
    /// Last pet time
    last_pet: AtomicU64,
}

/// Global hardware watchdog state
static HW_WATCHDOG: Mutex<HwWatchdogState> = Mutex::new(HwWatchdogState {
    enabled: false,
    pet_timeout: 1_000_000_000, // 1 second default
    last_pet: AtomicU64::new(0),
});

/// Get current time in nanoseconds
///
/// TODO: Implement proper time source
fn get_current_time_ns() -> u64 {
    // For now, return 0
    // In real implementation, this would read from a time source
    0
}

/// Watchdog handler callback
///
/// This function is called when a watchdog expires.
/// The default implementation halts the system.
pub fn watchdog_handler(watchdog: &Watchdog) {
    println!(
        "Watchdog \"{}\" (timeout {} ms) just fired!!",
        watchdog.name,
        watchdog.timeout / 1_000_000
    );

    // Halt the system
    // TODO: platform_halt(HALT_ACTION_HALT, HALT_REASON_SW_RESET);
    panic!("Watchdog expired");
}

/// Initialize hardware watchdog
///
/// # Arguments
///
/// * `timeout_ns` - Timeout in nanoseconds
pub fn watchdog_hw_init(timeout_ns: u64) -> Result<(), i32> {
    assert!(timeout_ns != 0);

    let mut state = HW_WATCHDOG.lock();
    state.enabled = false;
    state.pet_timeout = timeout_ns;
    state.last_pet.store(get_current_time_ns(), Ordering::Release);

    // TODO: platform_watchdog_init(timeout_ns, &state.pet_timeout);

    Ok(())
}

/// Enable or disable hardware watchdog
pub fn watchdog_hw_set_enabled(enabled: bool) {
    let mut state = HW_WATCHDOG.lock();

    if state.enabled == enabled {
        return;
    }

    state.enabled = enabled;

    if enabled {
        // Start hardware watchdog
        state.last_pet.store(get_current_time_ns(), Ordering::Release);
        // TODO: platform_watchdog_pet();
    } else {
        // Stop hardware watchdog
        // TODO: platform_watchdog_disable();
    }
}

/// Pet the hardware watchdog
pub fn watchdog_hw_pet() {
    let state = HW_WATCHDOG.lock();

    if !state.enabled {
        return;
    }

    // Update last pet time
    state.last_pet.store(get_current_time_ns(), Ordering::Release);

    // Pet the hardware watchdog
    // TODO: platform_watchdog_pet();
}

/// Hardware watchdog timer callback
///
/// This function is called periodically to pet the hardware watchdog.
pub fn hw_watchdog_timer_callback() {
    let pet_timeout = {
        let state = HW_WATCHDOG.lock();
        if !state.enabled {
            return;
        }
        state.pet_timeout
    };

    // Schedule next callback
    // TODO: timer_set_oneshot(get_current_time_ns() + pet_timeout, hw_watchdog_timer_callback, nullptr);

    // Pet the hardware watchdog
    watchdog_hw_pet();
}

/// Watchdog manager for managing multiple watchdogs
pub struct WatchdogManager {
    /// List of registered watchdogs
    watchdogs: Vec<Arc<Mutex<Watchdog>>>,
}

impl WatchdogManager {
    /// Create a new watchdog manager
    pub fn new() -> Self {
        Self {
            watchdogs: Vec::new(),
        }
    }

    /// Add a watchdog to the manager
    pub fn add(&mut self, watchdog: Arc<Mutex<Watchdog>>) {
        self.watchdogs.push(watchdog);
    }

    /// Check all watchdogs and handle any that have expired
    pub fn check_watchdogs(&self) {
        for watchdog in &self.watchdogs {
            let w = watchdog.lock();
            if w.check_expired() {
                // Call the handler
                watchdog_handler(&w);
            }
        }
    }

    /// Pet all watchdogs
    pub fn pet_all(&self) {
        for watchdog in &self.watchdogs {
            let mut w = watchdog.lock();
            w.pet();
        }
    }
}

impl Default for WatchdogManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global watchdog manager
static WATCHDOG_MANAGER: Mutex<WatchdogManager> = Mutex::new(WatchdogManager::new());

/// Add a watchdog to the global manager
pub fn watchdog_register(watchdog: Arc<Mutex<Watchdog>>) {
    WATCHDOG_MANAGER.lock().add(watchdog);
}

/// Check all registered watchdogs
pub fn watchdog_check_all() {
    WATCHDOG_MANAGER.lock().check_watchdogs();
}

/// Pet all registered watchdogs
pub fn watchdog_pet_all() {
    WATCHDOG_MANAGER.lock().pet_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watchdog_create() {
        let mut wd = Watchdog::new(5000, "test");
        assert!(wd.is_valid());
        assert_eq!(wd.name, "test");
        assert_eq!(wd.timeout, 5_000_000_000);
        assert!(!wd.enabled);
    }

    #[test]
    fn test_watchdog_enable_disable() {
        let mut wd = Watchdog::new(5000, "test");
        assert!(!wd.enabled);

        wd.set_enabled(true);
        assert!(wd.enabled);

        wd.set_enabled(false);
        assert!(!wd.enabled);
    }

    #[test]
    fn test_watchdog_pet() {
        let mut wd = Watchdog::new(5000, "test");
        wd.set_enabled(true);

        // Pet the watchdog
        wd.pet();

        // Check that it hasn't expired
        assert!(!wd.check_expired());
    }

    #[test]
    fn test_watchdog_manager() {
        let mut manager = WatchdogManager::new();
        let wd = Arc::new(Mutex::new(Watchdog::new(5000, "test1")));
        let wd2 = Arc::new(Mutex::new(Watchdog::new(10000, "test2")));

        manager.add(wd.clone());
        manager.add(wd2.clone());

        // Pet all watchdogs
        manager.pet_all();

        // Check none have expired
        manager.check_watchdogs();
    }
}
