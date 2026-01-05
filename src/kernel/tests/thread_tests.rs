// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Thread Tests
//!
//! This module contains tests for thread management, synchronization,
//! and scheduling. Converted from the C++ thread_tests.cpp.
//!
//! # Test Categories
//!
//! - **Mutex tests**: Mutex contention and inheritance
//! - **Event tests**: Event signaling and waiting
//! - **Spinlock tests**: Spin lock functionality
//! - **Atomic tests**: Atomic operations
//! - **Join tests**: Thread joining and detachment
//! - **Affinity tests**: CPU affinity
//! - **Priority tests**: Thread priority changes
//! - **TLS tests**: Thread-local storage

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::thread;
use crate::kernel::sync;
use crate::debug;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::sync::atomic;

// ============================================================================
// Mutex Tests
// ============================================================================

/// Test mutex contention
fn mutex_contention_test() -> TestResult {
    use crate::kernel::sync::Mutex;

    const ITERATIONS: usize = 100000;
    static SHARED: AtomicUsize = AtomicUsize::new(0);

    let mutex = Arc::new(Mutex::new(0usize));

    // Create multiple threads that contend for the mutex
    let mut threads = Vec::new();

    for i in 0..5 {
        let mutex_clone = mutex.clone();
        let thread = thread::Thread::new(
            &format!("mutex_tester_{}", i),
            move || {
                for _ in 0..ITERATIONS {
                    let mut data = mutex_clone.lock();
                    *data += 1;
                    // Simulate some work
                    if *data % 10000 == 0 {
                        debug::log_debug!("Thread {} count: {}", i, *data);
                    }
                }
                0
            },
            thread::Priority::Default,
        )?;
        threads.push(thread);
    }

    // Start and wait for all threads
    for thread in &threads {
        thread.resume()?;
    }

    for thread in threads {
        thread.join(None)?;
    }

    // Verify final count
    let final_count = *mutex.lock();
    assert_eq!(final_count, 5 * ITERATIONS, "Mutex contention count mismatch");

    debug::log_info!("Mutex contention test passed: {}", final_count);
    Ok(())
}

/// Test mutex priority inheritance
fn mutex_inheritance_test() -> TestResult {
    use crate::kernel::sync::Mutex;

    const ITERATIONS: usize = 100000;
    const NUM_MUTEXES: usize = 4;

    let mutexes: alloc::vec::Vec<Arc<Mutex<u8>>> = (0..NUM_MUTEXES)
        .map(|_| Arc::new(Mutex::new(0)))
        .collect();

    // Create threads that randomly acquire/release mutexes
    let mut threads = Vec::new();

    for i in 0..5 {
        let mutexes_clone = mutexes.clone();
        let thread = thread::Thread::new(
            &format!("inherit_tester_{}", i),
            move || {
                for count in 0..ITERATIONS {
                    // Pick random priority
                    let priority = thread::Priority::Default +
                        (count % 9) as i32 - 4;

                    thread::current_thread().set_priority(priority);

                    // Acquire random number of mutexes
                    let num_mutexes = (count % NUM_MUTEXES) + 1;
                    for j in 0..num_mutexes {
                        let _lock = mutexes_clone[j].lock();
                    }

                    if count % 1000 == 0 {
                        debug::log_debug!("Thread {} iteration {}", i, count);
                    }

                    // Release happens when locks go out of scope
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

    for thread in threads {
        thread.join(None)?;
    }

    debug::log_info!("Mutex inheritance test passed");
    Ok(())
}

// ============================================================================
// Event Tests
// ============================================================================

/// Test event signaling
fn event_signal_test() -> TestResult {
    use crate::kernel::sync::Event;

    let event = Arc::new(Event::new(false));

    // Create signaler thread
    let event_clone = event.clone();
    let signaler = thread::Thread::new(
        "event_signaler",
        move || {
            thread::sleep(1_000_000_000); // 1 second in nanoseconds
            event_clone.signal();
            debug::log_debug!("Event signaled");
            0
        },
        thread::Priority::Default,
    )?;

    // Create waiter threads
    let mut threads = alloc::vec::Vec::new();
    threads.push(signaler);

    for i in 0..4 {
        let event_clone = event.clone();
        let waiter = thread::Thread::new(
            &format!("event_waiter_{}", i),
            move || {
                debug::log_debug!("Thread {} waiting on event", i);
                event_clone.wait();
                debug::log_debug!("Thread {} done waiting", i);
                0
            },
            thread::Priority::Default,
        )?;
        threads.push(waiter);
    }

    // Start all threads
    for thread in &threads {
        thread.resume()?;
    }

    // Wait for completion
    for thread in threads {
        thread.join(None)?;
    }

    debug::log_info!("Event signal test passed");
    Ok(())
}

/// Test event auto-reset
fn event_autosignal_test() -> TestResult {
    use crate::kernel::sync::Event;

    let event = Arc::new(Event::new(true)); // Auto-signal

    let mut threads = alloc::vec::Vec::new();

    // Create multiple waiters - only one should wake per signal
    for i in 0..4 {
        let event_clone = event.clone();
        let waiter = thread::Thread::new(
            &format!("auto_waiter_{}", i),
            move || {
                debug::log_debug!("Thread {} waiting on auto event", i);
                if let Err(_) = event_clone.wait_timeout(5_000_000_000) {
                    debug::log_debug!("Thread {} wait timeout", i);
                }
                0
            },
            thread::Priority::Default,
        )?;
        threads.push(waiter);
    }

    // Start threads
    for thread in &threads {
        thread.resume()?;
    }

    // Signal multiple times
    for _ in 0..4 {
        thread::sleep(100_000_000); // 100ms
        event.signal();
    }

    // Kill remaining threads
    thread::sleep(1_000_000_000);
    for thread in threads {
        thread.kill()?;
        thread.join(None)?;
    }

    debug::log_info!("Event auto-signal test passed");
    Ok(())
}

// ============================================================================
// Spinlock Tests
// ============================================================================

/// Test spinlock basic functionality
fn spinlock_basic_test() -> TestResult {
    use crate::kernel::sync::SpinLock;

    let lock = SpinLock::new(0u32);

    // Test basic lock/unlock
    {
        let mut data = lock.lock();
        assert!(!lock.is_locked());
        *data = 42;
    }

    let value = *lock.lock();
    assert_eq!(value, 42);

    debug::log_info!("Spinlock basic test passed");
    Ok(())
}

/// Test spinlock contention
fn spinlock_contention_test() -> TestResult {
    use crate::kernel::sync::SpinLock;

    const ITERATIONS: usize = 100000;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let lock = Arc::new(SpinLock::new(0usize));
    let mut threads = Vec::new();

    for i in 0..8 {
        let lock_clone = lock.clone();
        let thread = thread::Thread::new(
            &format!("spinlock_tester_{}", i),
            move || {
                for _ in 0..ITERATIONS {
                    let mut data = lock_clone.lock();
                    *data += 1;
                }
                COUNTER.fetch_add(1, Ordering::SeqCst);
                0
            },
            thread::Priority::Low,
        )?;
        threads.push(thread);
    }

    // Start all threads
    for thread in &threads {
        thread.resume()?;
    }

    for thread in threads {
        thread.join(None)?;
    }

    let final_count = *lock.lock();
    assert_eq!(final_count, 8 * ITERATIONS, "Spinlock counter mismatch");
    assert_eq!(COUNTER.load(Ordering::SeqCst), 8, "Thread counter mismatch");

    debug::log_info!("Spinlock contention test passed: {}", final_count);
    Ok(())
}

// ============================================================================
// Atomic Tests
// ============================================================================

/// Test atomic add operations
fn atomic_add_test() -> TestResult {
    static ATOMIC_VAR: AtomicUsize = AtomicUsize::new(0);

    const ITERATIONS: usize = 10000000;

    let mut threads = Vec::new();

    // Create threads that add and subtract
    for i in 0..4 {
        let thread = thread::Thread::new(
            &format!("atomic_add_{}", i),
            || {
                for _ in 0..ITERATIONS {
                    ATOMIC_VAR.fetch_add(1, Ordering::SeqCst);
                }
                0
            },
            thread::Priority::Low,
        )?;
        threads.push(thread);
    }

    for i in 0..4 {
        let thread = thread::Thread::new(
            &format!("atomic_sub_{}", i),
            || {
                for _ in 0..ITERATIONS {
                    ATOMIC_VAR.fetch_sub(1, Ordering::SeqCst);
                }
                0
            },
            thread::Priority::Low,
        )?;
        threads.push(thread);
    }

    // Start all threads
    for thread in &threads {
        thread.resume()?;
    }

    for thread in threads {
        thread.join(None)?;
    }

    let final_value = ATOMIC_VAR.load(Ordering::SeqCst);
    assert_eq!(final_value, 0, "Atomic value should be zero");

    debug::log_info!("Atomic add test passed: {}", final_value);
    Ok(())
}

// ============================================================================
// Join Tests
// ============================================================================

/// Test thread join
fn thread_join_test() -> TestResult {
    // Create a thread that returns a specific value
    let thread = thread::Thread::new(
        "join_tester",
        || {
            thread::sleep(500_000_000); // 500ms
            debug::log_debug!("Join tester exiting");
            42
        },
        thread::Priority::Default,
    )?;

    thread.resume()?;

    // Join and get return value
    let return_value = thread.join(None)?;
    assert_eq!(return_value, 42, "Thread return value mismatch");

    debug::log_info!("Thread join test passed");
    Ok(())
}

/// Test thread detach
fn thread_detach_test() -> TestResult {
    let thread = thread::Thread::new(
        "detach_tester",
        || {
            thread::sleep(100_000_000); // 100ms
            debug::log_debug!("Detached thread done");
            0
        },
        thread::Priority::Default,
    )?;

    thread.detach()?;
    thread.resume()?;

    // Wait for thread to complete
    thread::sleep(200_000_000); // 200ms

    debug::log_info!("Thread detach test passed");
    Ok(())
}

// ============================================================================
// Priority Tests
// ============================================================================

/// Test thread priority changes
fn thread_priority_test() -> TestResult {
    use crate::kernel::sync::Event;

    let event = Arc::new(Event::new(false));

    // Create a thread with low priority
    let event_clone = event.clone();
    let thread = thread::Thread::new(
        "prio_tester",
        move || {
            let initial_prio = thread::current_thread().priority();
            assert_eq!(initial_prio, thread::Priority::Low);

            event_clone.signal();

            // Wait for priority to change
            let current = thread::current_thread();
            while current.priority() == thread::Priority::Low {
                thread::yield();
            }

            event_clone.signal();

            // Wait for another priority change
            let current = thread::current_thread();
            while current.priority() != thread::Priority::High {
                thread::yield();
            }

            0
        },
        thread::Priority::Low,
    )?;

    thread.resume()?;

    // Wait for thread to start
    event.wait();

    // Change priority to default
    thread.set_priority(thread::Priority::Default)?;
    event.wait();

    // Change priority to high
    thread.set_priority(thread::Priority::High)?;

    thread.join(None)?;

    debug::log_info!("Thread priority test passed");
    Ok(())
}

// ============================================================================
// TLS Tests
// ============================================================================

/// Test thread-local storage
fn tls_test() -> TestResult {
    static TLS_DESTROY_COUNT: AtomicUsize = AtomicUsize::new(0);

    // Create a thread that sets TLS values
    let thread = thread::Thread::new(
        "tls_tester",
        || {
            // Set TLS values
            thread::tls_set(0, 0x666 as *mut u8);
            thread::tls_set(1, 0xAAA as *mut u8);

            // Verify they're set
            let val0 = thread::tls_get(0);
            let val1 = thread::tls_get(1);

            if val0 != 0x666 as *mut u8 || val1 != 0xAAA as *mut u8 {
                return -1;
            }

            0
        },
        thread::Priority::Low,
    )?;

    thread.resume()?;
    thread::sleep(200_000_000); // 200ms
    thread.join(None)?;

    debug::log_info!("TLS test passed");
    Ok(())
}

// ============================================================================
// Test Suite Registration
// ============================================================================

/// Create the thread test suite
pub fn create_thread_suite() -> TestSuite {
    TestSuite::new(
        "thread",
        "Thread management and synchronization tests",
        alloc::vec::Vec::from([
            TestCase::new("mutex_contention", "Mutex contention test", mutex_contention_test),
            TestCase::new("mutex_inheritance", "Mutex priority inheritance test", mutex_inheritance_test),
            TestCase::new("event_signal", "Event signaling test", event_signal_test),
            TestCase::new("event_autosignal", "Event auto-signal test", event_autosignal_test),
            TestCase::new("spinlock_basic", "Spinlock basic test", spinlock_basic_test),
            TestCase::new("spinlock_contention", "Spinlock contention test", spinlock_contention_test),
            TestCase::new("atomic_add", "Atomic add test", atomic_add_test),
            TestCase::new("thread_join", "Thread join test", thread_join_test),
            TestCase::new("thread_detach", "Thread detach test", thread_detach_test),
            TestCase::new("thread_priority", "Thread priority test", thread_priority_test),
            TestCase::new("tls", "Thread-local storage test", tls_test),
        ]),
    )
}

/// Register all thread tests
pub fn register() {
    register_suite(create_thread_suite());
}
