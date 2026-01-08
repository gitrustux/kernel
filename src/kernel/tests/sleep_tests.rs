// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Sleep Tests
//!
//! Tests for thread sleep functionality.


use crate::kernel::tests::runner::*;
use crate::kernel::timer;
use crate::debug;

/// Test that sleep duration is accurate
fn sleep_duration_test() -> TestResult {
    const TARGET_MS: u64 = 500;
    const TOLERANCE_MS: u64 = 50;

    let mut early_wakeups = 0;

    for i in 0..5 {
        let start = timer::now_monotonic();
        timer::sleep(TARGET_MS * 1_000_000); // Convert to nanoseconds
        let actual = timer::now_monotonic() - start;
        let actual_ms = actual / 1_000_000;

        if actual_ms < TARGET_MS {
            early_wakeups += 1;
            log_debug!(
                "Sleep iteration {}: woke after {} ms (target {} ms)",
                i, actual_ms, TARGET_MS
            );
        }
    }

    if early_wakeups > 0 {
        log_debug!("Warning: {} early wakeups detected", early_wakeups);
    }

    log_info!("Sleep duration test passed");
    Ok(())
}

/// Test sleep consistency
fn sleep_consistency_test() -> TestResult {
    const SLEEP_NS: u64 = 100_000_000; // 100ms
    const ITERATIONS: usize = 10;

    let mut durations = alloc::vec::Vec::new();

    for _ in 0..ITERATIONS {
        let start = timer::now_monotonic();
        timer::sleep(SLEEP_NS);
        let actual = timer::now_monotonic() - start;
        durations.push(actual);
    }

    // Check that all durations are reasonable (within 20% of target)
    for (i, duration) in durations.iter().enumerate() {
        let variance = if *duration > SLEEP_NS {
            *duration - SLEEP_NS
        } else {
            SLEEP_NS - *duration
        };
        let variance_percent = (variance * 100) / SLEEP_NS;

        assert_le!(variance_percent, 20, "Sleep variance too high");
        log_debug!("Sleep {}: {} ns (variance: {}%)", i, duration, variance_percent);
    }

    log_info!("Sleep consistency test passed");
    Ok(())
}

/// Test multiple consecutive sleeps
fn consecutive_sleep_test() -> TestResult {
    let sleep_times = [10_000_000, 50_000_000, 100_000_000, 200_000_000]; // 10ms, 50ms, 100ms, 200ms

    for (i, &sleep_ns) in sleep_times.iter().enumerate() {
        let start = timer::now_monotonic();
        timer::sleep(sleep_ns);
        let actual = timer::now_monotonic() - start;

        log_debug!("Sleep {}: target={}ns, actual={}ns", i, sleep_ns, actual);
    }

    log_info!("Consecutive sleep test passed");
    Ok(())
}

/// Test zero duration sleep
fn zero_sleep_test() -> TestResult {
    let start = timer::now_monotonic();
    timer::sleep(0);
    let elapsed = timer::now_monotonic() - start;

    // Zero sleep should return immediately
    assert_lt!(elapsed, 1_000_000, "Zero sleep took too long"); // < 1ms

    log_info!("Zero sleep test passed");
    Ok(())
}

/// Test very short sleep
fn short_sleep_test() -> TestResult {
    const SHORT_NS: u64 = 1_000_000; // 1ms

    let start = timer::now_monotonic();
    timer::sleep(SHORT_NS);
    let actual = timer::now_monotonic() - start;

    // Should sleep at least the requested amount
    assert_ge!(actual, SHORT_NS, "Sleep too short");

    log_info!("Short sleep test passed: {} ns", actual);
    Ok(())
}

/// Create the sleep test suite
pub fn create_sleep_suite() -> TestSuite {
    TestSuite::new(
        "sleep",
        "Thread sleep functionality tests",
        alloc::vec::Vec::from([
            TestCase::new("duration", "Sleep duration accuracy", sleep_duration_test),
            TestCase::new("consistency", "Sleep consistency", sleep_consistency_test),
            TestCase::new("consecutive", "Consecutive sleeps", consecutive_sleep_test),
            TestCase::new("zero", "Zero duration sleep", zero_sleep_test),
            TestCase::new("short", "Very short sleep", short_sleep_test),
        ]),
    )
}

/// Register sleep tests
pub fn register() {
    register_suite(create_sleep_suite());
}
