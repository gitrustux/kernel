// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Counters
//!
//! This module provides system-wide counter tracking for monitoring
//! kernel metrics on a per-CPU basis.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::rustux::types::*;

/// Maximum number of CPUs supported
pub const SMP_MAX_CPUS: usize = 64;

/// Fixed-point shift for decimal calculations (8 bits = 256)
const DOT8_SHIFT: u64 = 8;

/// Counter types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterType {
    /// Sum across all CPUs
    Sum = 0,
    /// Maximum value across all CPUs
    Max = 1,
}

/// Counter descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CounterDesc {
    /// Counter name
    pub name: &'static str,
    /// Counter type
    pub counter_type: CounterType,
    /// Variable name (for introspection)
    pub varname: &'static str,
}

/// Watched counter entry for monitoring
#[derive(Debug)]
struct WatchedCounter {
    /// Descriptor for this counter
    desc: CounterDesc,
    /// Whether to show verbose output
    verbose: bool,
}

/// Counter arena storage
///
/// Stores per-CPU counter values indexed by counter ID
pub struct CounterArena {
    /// Per-CPU counter arrays
    percpu_counters: [[u64; SMP_MAX_CPUS]],
}

impl CounterArena {
    /// Create a new counter arena
    pub const fn new() -> Self {
        Self {
            percpu_counters: [[0; SMP_MAX_CPUS]; 0],
        }
    }
}

/// Global counter state
static COUNTER_STATE: Mutex<CounterState> = Mutex::new(CounterState::new());

/// Internal state for counter management
struct CounterState {
    /// Registered counters
    counters: Vec<CounterDesc>,
    /// Watched counters (for monitoring)
    watched: Vec<WatchedCounter>,
}

impl CounterState {
    pub const fn new() -> Self {
        Self {
            counters: Vec::new(),
            watched: Vec::new(),
        }
    }
}

/// Register a counter descriptor
///
/// # Safety
///
/// Must be called during kernel initialization before any counters are used
pub unsafe fn register_counter(desc: CounterDesc) -> usize {
    let mut state = COUNTER_STATE.lock();
    let index = state.counters.len();
    state.counters.push(desc);
    index
}

/// Get the number of registered counters
pub fn get_num_counters() -> usize {
    COUNTER_STATE.lock().counters.len()
}

/// Increment a counter on the current CPU
///
/// # Arguments
///
/// * `counter_id` - Counter ID from registration
/// * `value` - Value to add (default 1)
pub fn counter_add(cpu: usize, counter_id: usize, value: u64) {
    // TODO: Implement per-CPU counter storage
    let _ = cpu;
    let _ = counter_id;
    let _ = value;
}

/// Set a counter value on the current CPU
///
/// # Arguments
///
/// * `cpu` - CPU number
/// * `counter_id` - Counter ID from registration
/// * `value` - Value to set
pub fn counter_set(cpu: usize, counter_id: usize, value: u64) {
    // TODO: Implement per-CPU counter storage
    let _ = cpu;
    let _ = counter_id;
    let _ = value;
}

/// Get counter values from all CPUs
///
/// Returns an array of per-CPU counter values
pub fn counter_get_all_cpus(counter_id: usize) -> [u64; SMP_MAX_CPUS] {
    // TODO: Implement per-CPU counter storage
    let _ = counter_id;
    [0; SMP_MAX_CPUS]
}

/// Clean up counter values (remove zeros and sort)
///
/// Returns only non-zero values, sorted ascending
pub fn counters_clean_up_values(values_in: &[u64]) -> Vec<u64> {
    let mut values: Vec<u64> = values_in
        .iter()
        .copied()
        .filter(|&v| v > 0)
        .collect();
    values.sort();
    values
}

/// Get a percentile from sorted values
///
/// Uses linear interpolation between closest ranks method.
/// `percentage_dot8` is percentage * 256 (e.g., 25% = 64, 75% = 192)
///
/// # Arguments
///
/// * `values` - Sorted array of values
/// * `percentage_dot8` - Percentage in fixed-point (0-256 = 0-100%)
pub fn counters_get_percentile(values: &[u64], percentage_dot8: u64) -> u64 {
    assert!(values.len() >= 2, "Need at least 2 values for percentile");

    let count = values.len() as u64;
    let target_dot8 = (count - 1) * percentage_dot8;
    let low_index = (target_dot8 >> DOT8_SHIFT) as usize;
    let high_index = low_index + 1;
    let fraction_dot8 = target_dot8 & 0xff;

    if high_index >= values.len() {
        return values[low_index];
    }

    let delta = values[high_index].wrapping_sub(values[low_index]);
    (values[low_index] << DOT8_SHIFT) + fraction_dot8 * delta
}

/// Check if counter values have outliers using Tukey's fences
///
/// Returns true if any value is outside [Q1 - 1.5*IQR, Q3 + 1.5*IQR]
pub fn counters_has_outlier(values_in: &[u64]) -> bool {
    let values = counters_clean_up_values(values_in);
    if values.len() < 2 {
        return false;
    }

    // Calculate Q1 (25th percentile) and Q3 (75th percentile)
    let q1_dot8 = counters_get_percentile(&values, 64); // 0.25 * 256
    let q3_dot8 = counters_get_percentile(&values, 192); // 0.75 * 256

    // Tukey's fences use k = 1.5
    let k_dot8 = 384; // 1.5 * 256
    let q_delta_dot8 = q3_dot8.wrapping_sub(q1_dot8);

    let low_fence = q1_dot8 as i64 - ((k_dot8 * q_delta_dot8) >> DOT8_SHIFT) as i64;
    let high_fence = q3_dot8 as i64 + ((k_dot8 * q_delta_dot8) >> DOT8_SHIFT) as i64;

    // Check if any value is an outlier
    for &value in &values {
        let scaled_value = (value as i64) << DOT8_SHIFT;
        if scaled_value < low_fence || scaled_value > high_fence {
            return true;
        }
    }

    false
}

/// Dump a single counter's values
pub fn dump_counter(desc: &CounterDesc, verbose: bool) {
    let values = counter_get_all_cpus(0); // TODO: Get actual counter ID

    let summary = if desc.counter_type == CounterType::Max {
        *values.iter().max().unwrap_or(&0)
    } else {
        values.iter().sum()
    };

    println!("[{:02}] {} = {}", 0, desc.name, summary);

    if summary == 0 {
        return;
    }

    // Print per-core values if verbose or if there are outliers
    if verbose || counters_has_outlier(&values) {
        print!("     ");
        for (ix, &value) in values.iter().enumerate() {
            if value > 0 {
                print!("[{}:{}]", ix, value);
            }
        }
        println!();
    }
}

/// Dump all registered counters
pub fn dump_all_counters(verbose: bool) {
    let state = COUNTER_STATE.lock();
    println!("{} counters available:", state.counters.len());
    for desc in &state.counters {
        dump_counter(desc, verbose);
    }
}

/// Find counters matching a prefix
///
/// Returns matching counter descriptors
pub fn find_counters_by_prefix(prefix: &str) -> Vec<CounterDesc> {
    let state = COUNTER_STATE.lock();
    state
        .counters
        .iter()
        .filter(|desc| desc.name.starts_with(prefix))
        .copied()
        .collect()
}

/// Binary search for upper bound of counter name
///
/// For finding counters starting with a given prefix
pub fn upper_bound_counter_name(name: &str) -> usize {
    let state = COUNTER_STATE.lock();
    // Simple linear search for now - could be optimized with binary search
    state
        .counters
        .iter()
        .position(|desc| desc.name > name)
        .unwrap_or(state.counters.len())
}

/// Add a counter to the watch list
///
/// Monitored counters are periodically dumped
pub fn watch_counter(desc: CounterDesc, verbose: bool) {
    let mut state = COUNTER_STATE.lock();
    state.watched.push(WatchedCounter { desc, verbose });
}

/// Stop watching all counters
pub fn stop_watching_counters() {
    let mut state = COUNTER_STATE.lock();
    state.watched.clear();
}

/// Dump all watched counters
///
/// Called periodically by the watcher thread
pub fn dump_watched_counters() {
    let state = COUNTER_STATE.lock();
    for wc in &state.watched {
        dump_counter(&wc.desc, wc.verbose);
    }
}

/// Initialize the counter subsystem
///
/// Called during kernel initialization
pub fn counters_init() {
    // Per-CPU counter arrays are initialized in the BSS
    // Wire them up here
    // TODO: Implement proper per-CPU initialization
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_up_values() {
        let values = vec![0, 5, 0, 10, 0, 3, 0];
        let cleaned = counters_clean_up_values(&values);
        assert_eq!(cleaned, vec![3, 5, 10]);
    }

    #[test]
    fn test_percentile() {
        let values = vec![10, 20, 30, 40, 50];
        let q1 = counters_get_percentile(&values, 64); // 25%
        let q3 = counters_get_percentile(&values, 192); // 75%
        // With linear interpolation, Q1 ≈ 20, Q3 ≈ 40
        assert!(q1 >= 19 && q1 <= 21);
        assert!(q3 >= 39 && q3 <= 41);
    }

    #[test]
    fn test_outlier_detection() {
        // No outliers
        let values1 = vec![10, 12, 11, 13, 10, 14, 11];
        assert!(!counters_has_outlier(&values1));

        // With outlier (100)
        let values2 = vec![10, 12, 11, 100, 10, 14, 11];
        assert!(counters_has_outlier(&values2));
    }
}
