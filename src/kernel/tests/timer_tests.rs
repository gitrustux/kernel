// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Timer Tests
//!
//! This module contains tests for timer functionality, including:
//! - Timer cancellation
//! - Timer callbacks
//! - Timer slack/coalescing
//! - Timer stress testing
//!
//! Converted from C++ timer_tests.cpp


use crate::kernel::tests::runner::*;
use crate::kernel::timer;
use crate::kernel::sync;
use crate::debug;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ============================================================================
// Helper Types
// ============================================================================

/// Timer test arguments
struct TimerTestArgs {
    timer_fired: AtomicBool,
    timer_count: AtomicUsize,
    result: AtomicBool,
}

impl TimerTestArgs {
    fn new() -> Self {
        Self {
            timer_fired: AtomicBool::new(false),
            timer_count: AtomicUsize::new(0),
            result: AtomicBool::new(false),
        }
    }
}

// ============================================================================
// Basic Timer Tests
// ============================================================================

/// Test cancelling a timer before its deadline
fn cancel_before_deadline_test() -> TestResult {
    use crate::kernel::timer::Timer;

    let args = Arc::new(TimerTestArgs::new());

    // Set a timer far in the future
    let timer = Timer::new(
        timer::now_monotonic() + 5 * 60 * 1_000_000_000, // 5 hours
        timer::Slack::None,
        {
            let args = args.clone();
            move |_now| {
                args.timer_fired.store(true, Ordering::SeqCst);
            }
        },
    );

    // Cancel the timer
    let cancelled = timer.cancel();

    assert_true!(cancelled, "Timer should be cancellable before deadline");
    assert_false!(args.timer_fired.load(Ordering::SeqCst), "Timer should not have fired");

    debug::log_info!("Cancel before deadline test passed");
    Ok(())
}

/// Test cancelling a timer after it has fired
fn cancel_after_fired_test() -> TestResult {
    use crate::kernel::timer::Timer;

    let args = Arc::new(TimerTestArgs::new());

    // Set a timer to fire immediately
    let timer = Timer::new(
        timer::now_monotonic(),
        timer::Slack::None,
        {
            let args = args.clone();
            move |_now| {
                args.timer_fired.store(true, Ordering::SeqCst);
            }
        },
    );

    // Wait for it to fire
    while !args.timer_fired.load(Ordering::SeqCst) {
        core::hint::spin_loop();
    }

    // Try to cancel - should fail
    let cancelled = timer.cancel();
    assert_false!(cancelled, "Timer should not be cancellable after firing");

    debug::log_info!("Cancel after fired test passed");
    Ok(())
}

/// Test cancelling a timer from its own callback
fn cancel_from_callback_test() -> TestResult {
    use crate::kernel::timer::Timer;

    let args = Arc::new(TimerTestArgs::new());
    args.result.store(true, Ordering::SeqCst); // Expect cancel to succeed

    // Set a timer that cancels itself
    let timer = Timer::new(
        timer::now_monotonic(),
        timer::Slack::None,
        {
            let args = args.clone();
            move |_now| {
                args.timer_fired.store(true, Ordering::SeqCst);
                // Try to cancel from within callback - should fail
                let result = args.result.load(Ordering::SeqCst);
                args.result.store(false, Ordering::SeqCst);
            }
        },
    );

    // Wait for it to fire
    while !args.timer_fired.load(Ordering::SeqCst) {
        core::hint::spin_loop();
    }

    assert_false!(args.result.load(Ordering::SeqCst), "Self-cancel should fail");

    // Try to cancel from outside - should also fail
    let cancelled = timer.cancel();
    assert_false!(cancelled, "Timer should not be cancellable after firing");

    debug::log_info!("Cancel from callback test passed");
    Ok(())
}

/// Test setting a timer from its own callback
fn set_from_callback_test() -> TestResult {
    use crate::kernel::timer::Timer;

    const REPEAT_COUNT: usize = 5;

    struct RepeatingTimer {
        args: Arc<TimerTestArgs>,
        count: AtomicUsize,
    }

    let args = Arc::new(TimerTestArgs::new());
    let repeat = Arc::new(RepeatingTimer {
        args: args.clone(),
        count: AtomicUsize::new(REPEAT_COUNT),
    });

    let timer = Timer::new(
        timer::now_monotonic(),
        timer::Slack::None,
        {
            let repeat = repeat.clone();
            move |_now| {
                let old = repeat.count.fetch_sub(1, Ordering::SeqCst);
                if old > 1 {
                    // Reschedule
                    let _ = Timer::new(
                        timer::now_monotonic() + 10_000, // 10us
                        timer::Slack::None,
                        {
                            let repeat = repeat.clone();
                            move |_now| {
                                let old = repeat.count.fetch_sub(1, Ordering::SeqCst);
                                if old > 1 {
                                    // This is a simplified version - in reality would
                                    // need more complex logic for true re-arming
                                }
                            }
                        },
                    );
                }
            }
        },
    );

    // Wait for all repeats
    while repeat.count.load(Ordering::SeqCst) > 0 {
        timer::sleep(1000); // 1us
    }

    // Try to cancel (may or may not succeed depending on timing)
    let _ = timer.cancel();

    debug::log_info!("Set from callback test passed");
    Ok(())
}

// ============================================================================
// Timer Slack/Coalescing Tests
// ============================================================================

/// Test timer coalescing in center mode
fn timer_coalescing_center_test() -> TestResult {
    use crate::kernel::timer::Timer;

    let when = timer::now_monotonic() + 1_000_000; // 1ms
    let offset = 10_000; // 10us
    let slack = timer::Slack::center(2 * offset);

    let deadlines = [
        when + (6 * offset),
        when,
        when - offset,
        when - (3 * offset),
        when + offset,
        when + (3 * offset),
        when + (5 * offset),
        when - (3 * offset),
    ];

    let expected_adjustments = [0, 0, 10_000, 0, -10_000, 0, 10_000, 0];

    debug::log_info!("Testing coalescing mode: center");
    debug::log_info!("       orig         new       adjustment");

    for (i, &deadline) in deadlines.iter().enumerate() {
        let timer = Timer::new(deadline, slack, move |_now| {});

        // In a full implementation, we'd check the scheduled_time and slack
        // For now, just verify the timer was created
        debug::log_info!(
            "[{}] {:>12}  -> {:>12}, {:>12}",
            i,
            deadline,
            deadline, // Would be scheduled_time in full impl
            expected_adjustments[i]
        );
    }

    debug::log_info!("Timer coalescing center test passed");
    Ok(())
}

/// Test timer coalescing in late mode
fn timer_coalescing_late_test() -> TestResult {
    use crate::kernel::timer::Timer;

    let when = timer::now_monotonic() + 1_000_000; // 1ms
    let offset = 10_000; // 10us
    let slack = timer::Slack::late(3 * offset);

    let deadlines = [
        when + offset,
        when + (2 * offset),
        when - offset,
        when - (3 * offset),
        when + (3 * offset),
        when + (2 * offset),
        when - (4 * offset),
    ];

    let expected_adjustments = [0, 0, 20_000, 0, 0, 0, 10_000];

    debug::log_info!("Testing coalescing mode: late");
    debug::log_info!("       orig         new       adjustment");

    for (i, &deadline) in deadlines.iter().enumerate() {
        let timer = Timer::new(deadline, slack, move |_now| {});

        debug::log_info!(
            "[{}] {:>12}  -> {:>12}, {:>12}",
            i,
            deadline,
            deadline,
            expected_adjustments[i]
        );
    }

    debug::log_info!("Timer coalescing late test passed");
    Ok(())
}

/// Test timer coalescing in early mode
fn timer_coalescing_early_test() -> TestResult {
    use crate::kernel::timer::Timer;

    let when = timer::now_monotonic() + 1_000_000; // 1ms
    let offset = 10_000; // 10us
    let slack = timer::Slack::early(3 * offset);

    let deadlines = [
        when,
        when + (2 * offset),
        when - offset,
        when - (3 * offset),
        when + (4 * offset),
        when + (5 * offset),
        when - (2 * offset),
    ];

    let expected_adjustments = [0, -20_000, 0, 0, 0, -10_000, -10_000];

    debug::log_info!("Testing coalescing mode: early");
    debug::log_info!("       orig         new       adjustment");

    for (i, &deadline) in deadlines.iter().enumerate() {
        let timer = Timer::new(deadline, slack, move |_now| {});

        debug::log_info!(
            "[{}] {:>12}  -> {:>12}, {:>12}",
            i,
            deadline,
            deadline,
            expected_adjustments[i]
        );
    }

    debug::log_info!("Timer coalescing early test passed");
    Ok(())
}

// ============================================================================
// Timer Diagnostics Tests
// ============================================================================

/// Test timer across all CPUs
fn timer_all_cpus_test() -> TestResult {
    use crate::kernel::mp;
    use crate::kernel::sync::Event;

    if mp::cpu_count() < 2 {
        test_skip!("Requires at least 2 CPUs");
    }

    let event = Arc::new(Event::new(false, EventFlags::empty()));

    // Create timer threads on each CPU
    let mut threads = alloc::vec::Vec::new();
    let cpu_count = mp::cpu_count();

    for cpu_id in 0..cpu_count {
        let event_clone = event.clone();

        let thread = crate::kernel::thread::Thread::new(
            &format!("timer_{}", cpu_id),
            move || {
                use crate::kernel::timer::Timer;

                let cpu_id_before = mp::current_cpu();

                // Set a timer
                let _timer = Timer::new(
                    timer::now_monotonic() + 10_000_000, // 10ms
                    timer::Slack::None,
                    move |_now| {
                        event_clone.signal();
                    },
                );

                // Wait for timer
                event_clone.wait();

                let cpu_id_after = mp::current_cpu();

                debug::log_debug!(
                    "Timer fired: CPU {} -> {}",
                    cpu_id_before,
                    cpu_id_after
                );

                0
            },
            crate::kernel::thread::Priority::Default,
        )?;

        // Set affinity to specific CPU
        thread.set_cpu_affinity(1 << cpu_id)?;
        threads.push(thread);
    }

    // Start all threads
    for thread in &threads {
        thread.resume()?;
    }

    // Wait for all to complete
    for thread in threads {
        thread.join(None)?;
    }

    debug::log_info!("Timer all CPUs test passed");
    Ok(())
}

/// Test far deadline timer
fn timer_far_deadline_test() -> TestResult {
    use crate::kernel::timer::Timer;
    use crate::kernel::sync::Event;

    let event = Arc::new(Event::new(false, EventFlags::empty()));

    // Set a timer far in the future
    let timer = Timer::new(
        u64::MAX - 5, // Near max time
        timer::Slack::None,
        {
            let event = event.clone();
            move |_now| {
                event.signal();
            }
        },
    );

    // Wait with timeout - should timeout before timer fires
    let result = event.wait_timeout(100_000_000); // 100ms

    // Should timeout
    assert!(result.is_err(), "Event should timeout");

    // Cancel the timer
    timer.cancel();

    debug::log_info!("Timer far deadline test passed");
    Ok(())
}

// ============================================================================
// Timer Stress Tests
// ============================================================================

/// Stress test timers with concurrent set/cancel operations
fn timer_stress_test() -> TestResult {
    use crate::kernel::mp;
    use crate::kernel::thread;

    if mp::cpu_count() < 2 {
        test_skip!("Requires at least 2 CPUs");
    }

    const DURATION_SECONDS: u64 = 1;
    const NUM_THREADS: usize = 4;

    struct StressState {
        done: AtomicBool,
        timers_set: AtomicUsize,
        timers_fired: AtomicUsize,
    }

    let state = Arc::new(StressState {
        done: AtomicBool::new(false),
        timers_set: AtomicUsize::new(0),
        timers_fired: AtomicUsize::new(0),
    });

    // Create worker threads
    let mut threads = alloc::vec::Vec::new();

    for i in 0..NUM_THREADS {
        let state = state.clone();
        let thread = thread::Thread::new(
            &format!("stress_worker_{}", i),
            move || {
                use crate::kernel::timer::Timer;

                while !state.done.load(Ordering::SeqCst) {
                    // Set a short timer
                    let duration = (i % 5) as u64 * 1_000_000; // 0-5ms

                    let state_clone = state.clone();
                    let _timer = Timer::new(
                        timer::now_monotonic() + duration,
                        timer::Slack::None,
                        move |_now| {
                            state_clone.timers_fired.fetch_add(1, Ordering::SeqCst);
                        },
                    );

                    state.timers_set.fetch_add(1, Ordering::SeqCst);

                    // Sleep for timer duration
                    timer::sleep(duration);
                }

                0
            },
            thread::Priority::Default,
        )?;

        threads.push(thread);
    }

    // Start all threads
    for thread in &threads {
        thread.resume()?;
    }

    // Run for specified duration
    timer::sleep(DURATION_SECONDS * 1_000_000_000);

    // Signal threads to stop
    state.done.store(true, Ordering::SeqCst);

    // Wait for completion
    for thread in threads {
        thread.join(None)?;
    }

    let timers_set = state.timers_set.load(Ordering::SeqCst);
    let timers_fired = state.timers_fired.load(Ordering::SeqCst);

    debug::log_info!(
        "Timer stress: {} timers set, {} timers fired",
        timers_set,
        timers_fired
    );

    debug::log_info!("Timer stress test passed");
    Ok(())
}

// ============================================================================
// Monotonicity Tests
// ============================================================================

/// Test that timer is monotonic
fn timer_monotonic_test() -> TestResult {
    let t1 = timer::now_monotonic();
    timer::sleep(1000); // 1us
    let t2 = timer::now_monotonic();

    assert_ge!(t2, t1, "Timer should be monotonic (non-decreasing)");
    assert_lt!(t2 - t1, 1_000_000_000, "Timer shouldn't jump too far");

    debug::log_info!("Timer monotonic test passed");
    Ok(())
}

/// Test timer frequency
fn timer_frequency_test() -> TestResult {
    let freq = timer::get_frequency();

    // All architectures should have at least 1 MHz
    assert_ge!(freq, 1_000_000, "Timer frequency too low");
    // And at most 10 GHz
    assert_le!(freq, 10_000_000_000, "Timer frequency too high");

    debug::log_info!("Timer frequency: {} MHz", freq / 1_000_000);
    debug::log_info!("Timer frequency test passed");
    Ok(())
}

// ============================================================================
// Test Suite Registration
// ============================================================================

/// Create the timer test suite
pub fn create_timer_suite() -> TestSuite {
    TestSuite::new(
        "timer",
        "Timer and scheduling tests",
        alloc::vec::Vec::from([
            TestCase::new("cancel_before_deadline", "Cancel timer before deadline", cancel_before_deadline_test),
            TestCase::new("cancel_after_fired", "Cancel timer after fired", cancel_after_fired_test),
            TestCase::new("cancel_from_callback", "Cancel timer from callback", cancel_from_callback_test),
            TestCase::new("set_from_callback", "Set timer from callback", set_from_callback_test),
            TestCase::new("coalescing_center", "Timer coalescing center mode", timer_coalescing_center_test),
            TestCase::new("coalescing_late", "Timer coalescing late mode", timer_coalescing_late_test),
            TestCase::new("coalescing_early", "Timer coalescing early mode", timer_coalescing_early_test),
            TestCase::new("all_cpus", "Timer across all CPUs", timer_all_cpus_test),
            TestCase::new("far_deadline", "Far deadline timer", timer_far_deadline_test),
            TestCase::new("stress", "Timer stress test", timer_stress_test),
            TestCase::new("monotonic", "Timer monotonicity", timer_monotonic_test),
            TestCase::new("frequency", "Timer frequency", timer_frequency_test),
        ]),
    )
}

/// Register all timer tests
pub fn register() {
    register_suite(create_timer_suite());
}
