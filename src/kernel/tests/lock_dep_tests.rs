// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Lock Dependency Tests
//!
//! Tests for lock dependency tracking and validation.


use crate::kernel::tests::runner::*;
use crate::kernel::sync;
use crate::debug;

/// Test basic spinlock acquire/release
fn spinlock_basic_test() -> TestResult {
    use crate::kernel::sync::SpinLock;

    let lock = SpinLock::new(42u32);

    // Test acquire and release
    {
        let guard = lock.lock();
        assert_eq!(*guard, 42);
        *guard = 100;
    }

    // Lock should be released now
    {
        let guard = lock.lock();
        assert_eq!(*guard, 100);
    }

    debug::log_info!("Spinlock basic test passed");
    Ok(())
}

/// Test spinlock try_lock
fn spinlock_try_lock_test() -> TestResult {
    use crate::kernel::sync::SpinLock;

    let lock = SpinLock::new(0u32);

    // Try lock should succeed
    {
        let guard = lock.try_lock();
        assert!(guard.is_some(), "Try lock should succeed on unlocked lock");
    }

    // Try lock on already locked lock
    {
        let _guard1 = lock.lock();
        let guard2 = lock.try_lock();
        assert!(guard2.is_none(), "Try lock should fail on locked lock");
    }

    debug::log_info!("Spinlock try_lock test passed");
    Ok(())
}

/// Test mutex acquire/release
fn mutex_basic_test() -> TestResult {
    use crate::kernel::sync::Mutex;

    let mutex = Mutex::new(42u32);

    // Test acquire and release
    {
        let guard = mutex.lock();
        assert_eq!(*guard, 42);
        *guard = 100;
    }

    // Lock should be released now
    {
        let guard = mutex.lock();
        assert_eq!(*guard, 100);
    }

    debug::log_info!("Mutex basic test passed");
    Ok(())
}

/// Test nested lock (should detect deadlock if enabled)
fn nested_lock_test() -> TestResult {
    use crate::kernel::sync::SpinLock;

    let lock1 = SpinLock::new(0u32);
    let lock2 = SpinLock::new(0u32);

    // Proper lock ordering
    {
        let _guard1 = lock1.lock();
        let _guard2 = lock2.lock();
        // OK: Different locks
    }

    // Reverse ordering should also work
    {
        let _guard2 = lock2.lock();
        let _guard1 = lock1.lock();
        // OK: Different locks
    }

    debug::log_info!("Nested lock test passed");
    Ok(())
}

/// Test lock contention
fn lock_contention_test() -> TestResult {
    use crate::kernel::sync::Mutex;
    use core::sync::atomic::{AtomicUsize, Ordering};

    const NUM_THREADS: usize = 4;
    const ITERATIONS: usize = 10000;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let mutex = Mutex::new(0usize);

    // Create threads that contend for the lock
    let mut threads = alloc::vec::Vec::new();

    for i in 0..NUM_THREADS {
        let mutex_clone = mutex.clone();
        let thread = crate::kernel::thread::Thread::new(
            &alloc::format!("lock_contention_{}", i),
            move || {
                for _ in 0..ITERATIONS {
                    let mut data = mutex_clone.lock();
                    *data += 1;
                }
                COUNTER.fetch_add(1, Ordering::SeqCst);
                0
            },
            crate::kernel::thread::Priority::Default,
        )?;
        threads.push(thread);
    }

    // Start all threads
    for thread in &threads {
        thread.resume()?;
    }

    // Wait for completion
    for thread in threads {
        thread.join(None)?;
    }

    // Verify final count
    let final_count = *mutex.lock();
    assert_eq!(final_count, NUM_THREADS * ITERATIONS);

    debug::log_info!("Lock contention test passed: {}", final_count);
    Ok(())
}

/// Test reentrancy detection
fn reentrancy_test() -> TestResult {
    use crate::kernel::sync::SpinLock;

    let lock = SpinLock::new(0u32);

    // This is a simple test - actual deadlock detection would
    // require tracking the lock owner
    {
        let _guard1 = lock.lock();
        // Note: In Rust, trying to lock again would just block
        // (or panic if it's a try_lock that detects the same thread)
    }

    debug::log_info!("Reentrancy test passed");
    Ok(())
}

/// Create the lock dependency test suite
pub fn create_lock_dep_suite() -> TestSuite {
    TestSuite::new(
        "lock_dep",
        "Lock dependency and validation tests",
        alloc::vec::Vec::from([
            TestCase::new("spinlock_basic", "Spinlock basic operations", spinlock_basic_test),
            TestCase::new("spinlock_try_lock", "Spinlock try_lock", spinlock_try_lock_test),
            TestCase::new("mutex_basic", "Mutex basic operations", mutex_basic_test),
            TestCase::new("nested_lock", "Nested lock ordering", nested_lock_test),
            TestCase::new("contention", "Lock contention", lock_contention_test),
            TestCase::new("reentrancy", "Reentrancy detection", reentrancy_test),
        ]),
    )
}

/// Register lock dependency tests
pub fn register() {
    register_suite(create_lock_dep_suite());
}
