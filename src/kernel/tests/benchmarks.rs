// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Performance Benchmarks
//!
//! Microbenchmarks for various kernel operations.


use crate::kernel::tests::runner::*;
use crate::kernel::vm;
use crate::kernel::sync;
use crate::debug;

const BUFSIZE: usize = 3 * 1024 * 1024;
const ITER: usize = (1 * 1024 * 1024 * 1024) / BUFSIZE; // 1GB total

/// Benchmark memory setting (memset equivalent)
fn bench_memset_test() -> TestResult {
    let vaddr = vm::pmm::allocate_aligned(BUFSIZE, 4096)?;

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..ITER {
        let ptr = vaddr as *mut u8;
        for i in 0..BUFSIZE {
            unsafe { ptr.add(i).write_volatile(0) };
        }
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let bytes_cycle = (BUFSIZE * ITER * 1000) / duration_ns.max(1);

    debug::log_info!(
        "memset: {} cycles to clear {} bytes {} times ({} bytes), {}.{} bytes/cycle",
        duration_ns,
        BUFSIZE,
        ITER,
        BUFSIZE * ITER,
        bytes_cycle / 1000,
        bytes_cycle % 1000
    );

    vm::pmm::free(vaddr, BUFSIZE)?;
    Ok(())
}

/// Benchmark memory copying (memcpy equivalent)
fn bench_memcpy_test() -> TestResult {
    let vaddr = vm::pmm::allocate(BUFSIZE)?;

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..ITER {
        let src = (vaddr + BUFSIZE / 2) as *const u8;
        let dst = vaddr as *mut u8;
        for i in 0..(BUFSIZE / 2) {
            unsafe {
                let val = src.add(i).read_volatile();
                dst.add(i).write_volatile(val);
            }
        }
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let bytes_cycle = (BUFSIZE / 2 * ITER * 1000) / duration_ns.max(1);

    debug::log_info!(
        "memcpy: {} cycles to copy {} bytes {} times ({} bytes), {}.{} bytes/cycle",
        duration_ns,
        BUFSIZE / 2,
        ITER,
        BUFSIZE / 2 * ITER,
        bytes_cycle / 1000,
        bytes_cycle % 1000
    );

    vm::pmm::free(vaddr, BUFSIZE)?;
    Ok(())
}

/// Benchmark spinlock acquire/release
fn bench_spinlock_test() -> TestResult {
    use crate::kernel::sync::SpinLock;
    const COUNT: usize = 128 * 1024 * 1024;

    let lock = SpinLock::new(0u32);

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..COUNT {
        let _guard = lock.lock();
        // Critical section
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let cycles_per = duration_ns / COUNT as u64;

    debug::log_info!(
        "spinlock: {} cycles to acquire/release {} times ({} cycles per)",
        duration_ns,
        COUNT,
        cycles_per
    );

    Ok(())
}

/// Benchmark mutex acquire/release
fn bench_mutex_test() -> TestResult {
    use crate::kernel::sync::Mutex;
    const COUNT: usize = 128 * 1024 * 1024;

    let mutex = Mutex::new(0u32);

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..COUNT {
        let _guard = mutex.lock();
        // Critical section
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let cycles_per = duration_ns / COUNT as u64;

    debug::log_info!(
        "mutex: {} cycles to acquire/release {} times ({} cycles per)",
        duration_ns,
        COUNT,
        cycles_per
    );

    Ok(())
}

/// Benchmark context switching (thread yield)
fn bench_context_switch_test() -> TestResult {
    const COUNT: usize = 100000;

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..COUNT {
        crate::kernel::thread::yield();
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let cycles_per = duration_ns / COUNT as u64;

    debug::log_info!(
        "context_switch: {} cycles for {} yields ({} cycles per)",
        duration_ns,
        COUNT,
        cycles_per
    );

    Ok(())
}

/// Benchmark atomic operations
fn bench_atomic_test() -> TestResult {
    use core::sync::atomic::{AtomicUsize, Ordering};
    const COUNT: usize = 128 * 1024 * 1024;

    let counter = AtomicUsize::new(0);

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..COUNT {
        counter.fetch_add(1, Ordering::SeqCst);
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let cycles_per = duration_ns / COUNT as u64;

    debug::log_info!(
        "atomic: {} cycles for {} fetch_add ({} cycles per)",
        duration_ns,
        COUNT,
        cycles_per
    );

    assert_eq!(counter.load(Ordering::SeqCst), COUNT);
    Ok(())
}

/// Create the benchmark test suite
pub fn create_benchmark_suite() -> TestSuite {
    TestSuite::new(
        "benchmarks",
        "Performance microbenchmarks",
        alloc::vec::Vec::from([
            TestCase::new("memset", "Memory setting benchmark", bench_memset_test),
            TestCase::new("memcpy", "Memory copying benchmark", bench_memcpy_test),
            TestCase::new("spinlock", "Spinlock benchmark", bench_spinlock_test),
            TestCase::new("mutex", "Mutex benchmark", bench_mutex_test),
            TestCase::new("context_switch", "Context switch benchmark", bench_context_switch_test),
            TestCase::new("atomic", "Atomic operations benchmark", bench_atomic_test),
        ]),
    )
}

/// Register benchmark tests
pub fn register() {
    register_suite(create_benchmark_suite());
}
