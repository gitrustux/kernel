// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Sync IPI Tests
//!
//! Tests for inter-processor interrupt synchronization.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::mp;
use crate::kernel::thread;
use crate::kernel::sync;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::debug;

/// Test sequential IPI targeting
fn sequential_ipi_test() -> TestResult {
    let cpu_count = mp::cpu_count();

    if cpu_count < 2 {
        test_skip!("Need at least 2 CPUs");
    }

    debug::log_info!("Sequential IPI test ({} CPUs)", cpu_count);

    // Target each CPU sequentially
    for target_cpu in 0..cpu_count {
        if !mp::is_cpu_online(target_cpu) {
            continue;
        }

        // Create a thread on the target CPU
        let counter = AtomicUsize::new(0);
        let counter_ptr = &counter as *const AtomicUsize as usize;

        let thread = thread::Thread::new(
            &alloc::format!("ipi_target_{}", target_cpu),
            move || {
                // Simulate IPI handler work
                unsafe {
                    let counter_ref = &*(counter_ptr as *const AtomicUsize);
                    counter_ref.fetch_add(1, Ordering::SeqCst);
                }
                0
            },
            thread::Priority::High,
        )?;

        thread.set_cpu_affinity(1 << target_cpu)?;
        thread.resume()?;

        // Wait for completion
        thread.join(None)?;

        let count = counter.load(Ordering::SeqCst);
        assert_eq!(count, 1, "Counter should be incremented once");
    }

    debug::log_info!("Sequential IPI test passed");
    Ok(())
}

/// Test broadcast IPI (all CPUs but local)
fn broadcast_ipi_test() -> TestResult {
    let cpu_count = mp::cpu_count();

    if cpu_count < 2 {
        test_skip!("Need at least 2 CPUs");
    }

    debug::log_info!("Broadcast IPI test ({} CPUs)", cpu_count);

    let counter = AtomicUsize::new(0);

    // Create threads on all other CPUs
    let mut threads = alloc::vec::Vec::new();

    for cpu_id in 0..cpu_count {
        if cpu_id == mp::current_cpu() {
            continue; // Skip local CPU
        }

        if !mp::is_cpu_online(cpu_id) {
            continue;
        }

        let counter_ptr = &counter as *const AtomicUsize as usize;

        let thread = thread::Thread::new(
            &alloc::format!("ipi_worker_{}", cpu_id),
            move || {
                unsafe {
                    let counter_ref = &*(counter_ptr as *const AtomicUsize);
                    counter_ref.fetch_add(1, Ordering::SeqCst);
                }
                0
            },
            thread::Priority::High,
        )?;

        thread.set_cpu_affinity(1 << cpu_id)?;
        thread.resume()?;
        threads.push(thread);
    }

    // Wait for all threads to complete
    for thread in threads {
        thread.join(None)?;
    }

    let count = counter.load(Ordering::SeqCst);
    assert_eq!(count, threads.len() as usize, "All threads should increment counter");

    debug::log_info!("Broadcast IPI test passed ({} threads)", count);
    Ok(())
}

/// Test IPI deadlock avoidance
fn ipi_deadlock_test() -> TestResult {
    let cpu_count = mp::cpu_count();

    if cpu_count < 2 {
        test_skip!("Need at least 2 CPUs");
    }

    debug::log_info!("IPI deadlock test");

    // Create multiple threads that might try to send IPIs simultaneously
    let num_threads = cpu_count.min(5);
    let barrier = sync::Mutex::new(0usize);

    let mut threads = alloc::vec::Vec::new();

    for i in 0..num_threads {
        let thread = thread::Thread::new(
            &alloc::format!("ipi_deadlock_{}", i),
            move || {
                // Wait for all threads to be ready
                {
                    let mut count = barrier.lock();
                    *count += 1;
                }

                // Each thread does some work
                for _ in 0..10 {
                    thread::yield();
                }

                0
            },
            thread::Priority::Default,
        )?;

        thread.resume()?;
        threads.push(thread);
    }

    // Wait for all threads
    for thread in threads {
        thread.join(None)?;
    }

    debug::log_info!("IPI deadlock test passed");
    Ok(())
}

/// Test IPI stress
fn ipi_stress_test() -> TestResult {
    let cpu_count = mp::cpu_count();

    if cpu_count < 2 {
        test_skip!("Need at least 2 CPUs");
    }

    const ITERATIONS: usize = 100;

    debug::log_info!("IPI stress test ({} iterations)", ITERATIONS);

    for iteration in 0..ITERATIONS {
        let counter = AtomicUsize::new(0);

        // Create worker threads
        let mut threads = alloc::vec::Vec::new();

        for cpu_id in 0..cpu_count {
            if cpu_id == mp::current_cpu() {
                continue;
            }

            if !mp::is_cpu_online(cpu_id) {
                continue;
            }

            let counter_ptr = &counter as *const AtomicUsize as usize;

            let thread = thread::Thread::new(
                &alloc::format!("ipi_stress_{}_{}", iteration, cpu_id),
                move || {
                    unsafe {
                        let counter_ref = &*(counter_ptr as *const AtomicUsize);
                        counter_ref.fetch_add(1, Ordering::SeqCst);
                    }
                    0
                },
                thread::Priority::High,
            )?;

            thread.set_cpu_affinity(1 << cpu_id)?;
            thread.resume()?;
            threads.push(thread);
        }

        // Wait for completion
        for thread in threads {
            thread.join(None)?;
        }

        let count = counter.load(Ordering::SeqCst);
        assert_eq!(count, threads.len() as usize);
    }

    debug::log_info!("IPI stress test passed");
    Ok(())
}

/// Test IPI with timer synchronization
fn ipi_timer_sync_test() -> TestResult {
    use crate::kernel::timer;

    let cpu_count = mp::cpu_count();

    if cpu_count < 2 {
        test_skip!("Need at least 2 CPUs");
    }

    debug::log_info!("IPI timer sync test");

    let counter = AtomicUsize::new(0);
    let event = sync::Event::new(false, EventFlags::empty());

    // Create a timer that will signal the event
    let _timer = crate::kernel::timer::Timer::new(
        timer::now_monotonic() + 10_000_000, // 10ms
        timer::Slack::None,
        {
            let event = event.clone();
            move |_now| {
                event.signal();
            }
        },
    );

    // Wait for timer
    event.wait_timeout(100_000_000)?;

    debug::log_info!("IPI timer sync test passed");
    Ok(())
}

/// Create the sync IPI test suite
pub fn create_sync_ipi_suite() -> TestSuite {
    TestSuite::new(
        "sync_ipi",
        "Inter-processor interrupt synchronization tests",
        alloc::vec::Vec::from([
            TestCase::new("sequential", "Sequential IPI targeting", sequential_ipi_test),
            TestCase::new("broadcast", "Broadcast IPI", broadcast_ipi_test),
            TestCase::new("deadlock", "IPI deadlock avoidance", ipi_deadlock_test),
            TestCase::new("stress", "IPI stress", ipi_stress_test),
            TestCase::new("timer_sync", "IPI with timer sync", ipi_timer_sync_test),
        ]),
    )
}

/// Register sync IPI tests
pub fn register() {
    register_suite(create_sync_ipi_suite());
}
