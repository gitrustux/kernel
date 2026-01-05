// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Fibonacci Test (Threaded)
//!
//! Multi-threaded Fibonacci computation test.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::thread;
use crate::kernel::timer;
use crate::debug;

/// Fibonacci worker thread
fn fibo_thread(fib: u64) -> u64 {
    if fib == 0 {
        return 0;
    }
    if fib == 1 {
        return 1;
    }

    // Create two child threads for recursive computation
    let fib1 = fib - 1;
    let fib2 = fib - 2;

    let t1 = thread::Thread::new(
        &alloc::format!("fibo {}", fib1),
        move || fibo_thread(fib1),
        thread::Priority::Default,
    );

    let t2 = thread::Thread::new(
        &alloc::format!("fibo {}", fib2),
        move || fibo_thread(fib2),
        thread::Priority::Default,
    );

    let t1 = match t1 {
        Ok(t) => t,
        Err(_) => return fibo_thread(fib1), // Fallback to sequential
    };

    let t2 = match t2 {
        Ok(t) => t,
        Err(_) => {
            let r1 = t1.join(None).unwrap_or(0);
            return r1 + fibo_thread(fib2);
        }
    };

    t1.resume().ok();
    t2.resume().ok();

    let r1 = t1.join(None).unwrap_or(0);
    let r2 = t2.join(None).unwrap_or(0);

    r1 + r2
}

/// Test threaded Fibonacci computation
fn threaded_fibo_test() -> TestResult {
    const TEST_VALUE: u64 = 25; // F(25) = 75025

    let start = timer::now_monotonic();

    let result = fibo_thread(TEST_VALUE);

    let duration_ms = (timer::now_monotonic() - start) / 1_000_000;

    // Expected: F(25) = 75025
    assert_eq!(result, 75025, "Fibonacci result mismatch");

    debug::log_info!("Fibonacci({}) = {}", TEST_VALUE, result);
    debug::log_info!("Took {} msecs to calculate", duration_ms);

    Ok(())
}

/// Test sequential Fibonacci computation
fn sequential_fibo_test() -> TestResult {
    const TEST_VALUE: u64 = 35;

    fn fib(n: u64) -> u64 {
        if n == 0 { return 0; }
        if n == 1 { return 1; }
        let mut a = 0u64;
        let mut b = 1u64;
        for _ in 2..=n {
            let temp = a + b;
            a = b;
            b = temp;
        }
        b
    }

    let result = fib(TEST_VALUE);

    // F(35) = 9227465
    assert_eq!(result, 9227465, "Fibonacci result mismatch");

    debug::log_info!("Fibonacci({}) = {}", TEST_VALUE, result);

    Ok(())
}

/// Create the Fibonacci test suite
pub fn create_fibo_suite() -> TestSuite {
    TestSuite::new(
        "fibo",
        "Threaded Fibonacci computation tests",
        alloc::vec::Vec::from([
            TestCase::new("threaded", "Threaded Fibonacci", threaded_fibo_test),
            TestCase::new("sequential", "Sequential Fibonacci", sequential_fibo_test),
        ]),
    )
}

/// Register Fibonacci tests
pub fn register() {
    register_suite(create_fibo_suite());
}
