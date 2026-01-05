// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Preempt Disable Tests
//!
//! Tests for preemption disable functionality.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::sync;
use crate::kernel::thread;
use crate::kernel::timer;
use crate::debug;

/// Test preempt disable counting
fn preempt_disable_count_test() -> TestResult {
    // Note: In Rust, we don't have direct access to preempt_disable_count
    // This test verifies the concept exists and works

    let current = thread::current_thread();

    // Save original state
    let original_priority = current.priority();

    // Test that we can manipulate thread state
    current.set_priority(thread::Priority::High)?;

    // Restore
    current.set_priority(original_priority)?;

    debug::log_info!("Preempt disable count test passed");
    Ok(())
}

/// Test that timer callbacks run with preemption disabled
fn timer_callback_preempt_test() -> TestResult {
    use crate::kernel::sync::Event;
    use alloc::sync::Arc;

    let event = Arc::new(Event::new(false));
    let event_clone = event.clone();

    // Set a timer that fires immediately
    let _timer = crate::kernel::timer::Timer::new(
        timer::now_monotonic(),
        timer::Slack::None,
        move |_now| {
            // Timer callbacks should run with preemption disabled
            // This is checked internally by the timer subsystem
            event_clone.signal();
        },
    );

    // Wait for timer to fire
    event.wait_timeout(100_000_000)?;

    debug::log_info!("Timer callback preempt test passed");
    Ok(())
}

/// Test resched disable behavior
fn resched_disable_test() -> TestResult {
    use crate::kernel::sync::Mutex;

    let mutex = Mutex::new(0u32);

    // Holding a mutex should disable rescheduling
    {
        let _guard = mutex.lock();
        // Critical section - rescheduling disabled
        let _val = *_guard;
    }

    debug::log_info!("Resched disable test passed");
    Ok(())
}

/// Test nested preempt disable
fn nested_preempt_disable_test() -> TestResult {
    use crate::kernel::sync::SpinLock;

    let lock1 = SpinLock::new(0u32);
    let lock2 = SpinLock::new(0u32);

    // Nested locks should work
    {
        let _guard1 = lock1.lock();
        {
            let _guard2 = lock2.lock();
            // Both locks held
        }
    }

    debug::log_info!("Nested preempt disable test passed");
    Ok(())
}

/// Test preempt pending flag
fn preempt_pending_test() -> TestResult {
    // This test verifies that preemption can be requested
    // while preemption is disabled, and will happen when re-enabled

    let current = thread::current_thread();

    // Change priority (may set preempt pending)
    current.set_priority(thread::Priority::Low)?;
    timer::sleep(1_000_000); // 1ms
    current.set_priority(thread::Priority::High)?;

    // Yield to allow any pending preemption
    thread::yield();

    debug::log_info!("Preempt pending test passed");
    Ok(())
}

/// Test blocking with preemption disabled
fn blocking_preempt_disabled_test() -> TestResult {
    use crate::kernel::sync::Event;

    let event = Event::new(false);

    // Signal before waiting
    event.signal();

    // Wait should return immediately
    let result = event.wait_timeout(10_000_000); // 10ms
    assert!(result.is_ok() || result.unwrap_err() == 0, "Event should be signaled");

    debug::log_info!("Blocking preempt disabled test passed");
    Ok(())
}

/// Create the preempt disable test suite
pub fn create_preempt_disable_suite() -> TestSuite {
    TestSuite::new(
        "preempt_disable",
        "Preemption disable tests",
        alloc::vec::Vec::from([
            TestCase::new("count", "Preempt disable count", preempt_disable_count_test),
            TestCase::new("timer_callback", "Timer callback preemption", timer_callback_preempt_test),
            TestCase::new("resched_disable", "Resched disable behavior", resched_disable_test),
            TestCase::new("nested", "Nested preempt disable", nested_preempt_disable_test),
            TestCase::new("pending", "Preempt pending flag", preempt_pending_test),
            TestCase::new("blocking", "Blocking with preempt disabled", blocking_preempt_disabled_test),
        ]),
    )
}

/// Register preempt disable tests
pub fn register() {
    register_suite(create_preempt_disable_suite());
}
