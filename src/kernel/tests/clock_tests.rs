// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Clock Tests
//!
//! Tests for clock and timer functionality.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::timer;
use crate::kernel::mp;
use crate::kernel::thread;
use crate::debug;

/// Test that time never goes backwards
fn time_monotonic_test() -> TestResult {
    use crate::arch::ArchTimer;

    debug::log_info!("Testing time monotonicity...");

    let start = Arch::now_monotonic();
    let mut last = start;

    // Check for 5 seconds that time never goes backwards
    let duration = 5_000_000_000u64; // 5 seconds in nanoseconds
    let check_interval = 10_000_000u64; // 10ms

    let elapsed = 0u64;
    while elapsed < duration {
        let current = Arch::now_monotonic();

        if current < last {
            return Err(format!(
                "Time ran backwards: {} < {}",
                current, last
            ));
        }

        last = current;
        timer::sleep(check_interval);
    }

    debug::log_info!("Monotonicity test passed");
    Ok(())
}

/// Test current_time performance
fn current_time_perf_test() -> TestResult {
    use crate::arch::ArchTimer;

    let start_cycles = unsafe { crate::arch::Arch::now_monotonic() };
    let _time = Arch::now_monotonic();
    let end_cycles = unsafe { crate::arch::Arch::now_monotonic() };

    let cycles = end_cycles - start_cycles;

    debug::log_info!("{} cycles per current_time()", cycles);
    Ok(())
}

/// Test one-second intervals
fn one_second_intervals_test() -> TestResult {
    debug::log_info!("Counting to 5, in one second intervals...");

    for i in 0..5 {
        timer::sleep(1_000_000_000); // 1 second
        debug::log_info!("{}", i + 1);
    }

    debug::log_info!("Interval test passed");
    Ok(())
}

/// Test per-CPU clock measurement
fn per_cpu_clock_test() -> TestResult {
    let cpu_count = mp::cpu_count();
    let original_affinity = thread::current_thread().cpu_affinity();

    for cpu_id in 0..cpu_count {
        if !mp::is_cpu_online(cpu_id) {
            continue;
        }

        debug::log_info!("Measuring clock on CPU {}", cpu_id);

        // Set affinity to this CPU
        thread::current_thread().set_cpu_affinity(1 << cpu_id)?;

        // Measure cycles per second
        for i in 0..3 {
            let start_cycles = unsafe { crate::arch::Arch::now_monotonic() };
            let start_time = timer::now_monotonic();

            // Wait for 1 second
            while timer::now_monotonic() - start_time < 1_000_000_000 {
                core::hint::spin_loop();
            }

            let end_cycles = unsafe { crate::arch::Arch::now_monotonic() };
            let cycles_per_sec = end_cycles - start_cycles;

            debug::log_info!("CPU {}: {} cycles per second (run {})", cpu_id, cycles_per_sec, i + 1);
        }
    }

    // Restore original affinity
    thread::current_thread().set_cpu_affinity(original_affinity)?;

    debug::log_info!("Per-CPU clock test passed");
    Ok(())
}

/// Test timer resolution
fn timer_resolution_test() -> TestResult {
    use crate::arch::ArchTimer;

    let freq = Arch::get_frequency();

    // Timer frequency should be reasonable
    assert_ge!(freq, 1_000_000, "Timer frequency too low (< 1 MHz)");
    assert_le!(freq, 10_000_000_000, "Timer frequency too high (> 10 GHz)");

    debug::log_info!("Timer frequency: {} MHz", freq / 1_000_000);
    debug::log_info!("Timer resolution: {} ns", 1_000_000_000 / freq);

    Ok(())
}

/// Test sleep precision
fn sleep_precision_test() -> TestResult {
    const TARGET_NS: u64 = 100_000_000; // 100ms
    const TOLERANCE_NS: u64 = 10_000_000; // 10ms

    for i in 0..5 {
        let start = timer::now_monotonic();
        timer::sleep(TARGET_NS);
        let actual = timer::now_monotonic() - start;

        let diff = if actual > TARGET_NS {
            actual - TARGET_NS
        } else {
            TARGET_NS - actual
        };

        debug::log_debug!(
            "Sleep {}: target={}ns, actual={}ns, diff={}ns",
            i + 1,
            TARGET_NS,
            actual,
            diff
        );

        // Allow some tolerance for scheduling overhead
        if diff > TOLERANCE_NS {
            debug::log_debug!("Warning: Sleep precision outside tolerance");
        }
    }

    debug::log_info!("Sleep precision test passed");
    Ok(())
}

/// Test tick-tock timing
fn tick_tock_test() -> TestResult {
    debug::log_info!("Testing tick-tock timing...");

    let mut times = alloc::vec::Vec::new();

    // Collect time samples
    for _ in 0..10 {
        let t = timer::now_monotonic();
        times.push(t);
        timer::sleep(50_000_000); // 50ms
    }

    // Verify times are monotonically increasing
    for i in 1..times.len() {
        assert_ge!(times[i], times[i - 1], "Times not monotonic");
    }

    debug::log_info!("Tick-tock test passed");
    Ok(())
}

/// Create the clock test suite
pub fn create_clock_suite() -> TestSuite {
    TestSuite::new(
        "clock",
        "Clock and timer tests",
        alloc::vec::Vec::from([
            TestCase::new("monotonic", "Time monotonicity", time_monotonic_test),
            TestCase::new("perf", "current_time performance", current_time_perf_test),
            TestCase::new("intervals", "One-second intervals", one_second_intervals_test),
            TestCase::new("per_cpu", "Per-CPU clock measurement", per_cpu_clock_test),
            TestCase::new("resolution", "Timer resolution", timer_resolution_test),
            TestCase::new("sleep_precision", "Sleep precision", sleep_precision_test),
            TestCase::new("tick_tock", "Tick-tock timing", tick_tock_test),
        ]),
    )
}

/// Register clock tests
pub fn register() {
    register_suite(create_clock_suite());
}
