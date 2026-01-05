// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Cache Tests
//!
//! Tests for CPU cache operations.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::vm;
use crate::debug;

/// Test cache line size detection
fn cache_line_size_test() -> TestResult {
    use crate::arch::ArchCache;

    let dcache_line = ArchCache::dcache_line_size();
    let icache_line = ArchCache::icache_line_size();

    // All architectures should have at least 16 byte cache lines
    assert_ge!(dcache_line, 16, "D-cache line size too small");
    assert_le!(dcache_line, 1024, "D-cache line size too large");
    assert_ge!(icache_line, 16, "I-cache line size too small");
    assert_le!(icache_line, 1024, "I-cache line size too large");

    // Cache lines should be power of 2
    assert_true!(dcache_line.is_power_of_two(), "D-cache line size not power of 2");
    assert_true!(icache_line.is_power_of_two(), "I-cache line size not power of 2");

    debug::log_info!("Cache line sizes: D={}, I={}", dcache_line, icache_line);
    Ok(())
}

/// Test cache clean operation
fn cache_clean_test() -> TestResult {
    const TEST_SIZE: usize = 64 * 1024; // 64KB

    let vaddr = vm::pmm::allocate_aligned(TEST_SIZE, 64)?;
    let ptr = vaddr as *mut u8;

    // Write to cache
    for i in 0..TEST_SIZE {
        unsafe { ptr.add(i).write_volatile(0x99) };
    }

    // Clean the cache range
    let start = unsafe { crate::arch::Arch::now_monotonic() };
    unsafe { crate::arch::Arch::clean_cache_range(vaddr, TEST_SIZE) };
    let end = unsafe { crate::arch::Arch::now_monotonic() };

    let duration_ns = end - start;
    debug::log_info!("Cache clean: {} nsecs for {} bytes", duration_ns, TEST_SIZE);

    vm::pmm::free(vaddr, TEST_SIZE)?;
    Ok(())
}

/// Test cache operations on different buffer sizes
fn cache_size_test() -> TestResult {
    let sizes = [2 * 1024, 64 * 1024, 256 * 1024, 1 * 1024 * 1024];

    for size in sizes {
        let vaddr = vm::pmm::allocate_aligned(size, 64)?;
        let ptr = vaddr as *mut u8;

        // Initialize buffer
        for i in 0..size {
            unsafe { ptr.add(i).write_volatile(0xAA) };
        }

        // Clean cache
        unsafe { crate::arch::Arch::clean_cache_range(vaddr, size) };

        vm::pmm::free(vaddr, size)?;

        debug::log_debug!("Cache clean completed for {} bytes", size);
    }

    Ok(())
}

/// Test cache invalidate operation
fn cache_invalidate_test() -> TestResult {
    const TEST_SIZE: usize = 16 * 1024;

    let vaddr = vm::pmm::allocate_aligned(TEST_SIZE, 64)?;

    // Invalidate the cache range
    unsafe { crate::arch::Arch::invalidate_cache_range(vaddr, TEST_SIZE) };

    debug::log_info!("Cache invalidate completed for {} bytes", TEST_SIZE);

    vm::pmm::free(vaddr, TEST_SIZE)?;
    Ok(())
}

/// Test cache sync operation
fn cache_sync_test() -> TestResult {
    const TEST_SIZE: usize = 32 * 1024;

    let vaddr = vm::pmm::allocate_aligned(TEST_SIZE, 64)?;
    let ptr = vaddr as *mut u8;

    // Write data
    for i in 0..TEST_SIZE {
        unsafe { ptr.add(i).write_volatile((i & 0xFF) as u8) };
    }

    // Sync cache (clean + invalidate)
    unsafe { crate::arch::Arch::sync_cache_range(vaddr, TEST_SIZE) };

    // Verify data is still correct
    for i in 0..TEST_SIZE {
        let val = unsafe { ptr.add(i).read_volatile() };
        assert_eq!(val, (i & 0xFF) as u8, "Data mismatch after cache sync");
    }

    vm::pmm::free(vaddr, TEST_SIZE)?;
    debug::log_info!("Cache sync test passed");
    Ok(())
}

trait PowerOfTwo {
    fn is_power_of_two(self) -> bool;
}

impl PowerOfTwo for usize {
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }
}

/// Create the cache test suite
pub fn create_cache_suite() -> TestSuite {
    TestSuite::new(
        "cache",
        "CPU cache operation tests",
        alloc::vec::Vec::from([
            TestCase::new("line_size", "Cache line size detection", cache_line_size_test),
            TestCase::new("clean", "Cache clean operation", cache_clean_test),
            TestCase::new("size", "Cache operations on different sizes", cache_size_test),
            TestCase::new("invalidate", "Cache invalidate operation", cache_invalidate_test),
            TestCase::new("sync", "Cache sync operation", cache_sync_test),
        ]),
    )
}

/// Register cache tests
pub fn register() {
    register_suite(create_cache_suite());
}
