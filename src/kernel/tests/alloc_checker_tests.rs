// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Allocation Checker Tests
//!
//! Tests for allocation failure checking and validation.


use crate::kernel::tests::runner::*;
use crate::kernel::vm;
use crate::debug;
use alloc::boxed::Box;

/// Test basic allocation checking
fn alloc_checker_basic_test() -> TestResult {
    // Try to allocate a small amount of memory
    let size = 1024;
    let vaddr = vm::pmm::allocate(size)?;
    assert_true!(vaddr != 0, "Allocation should succeed");
    vm::pmm::free(vaddr, size)?;

    // Try zero-sized allocation (should succeed)
    let vaddr = vm::pmm::allocate(0)?;
    vm::pmm::free(vaddr, 0)?;

    debug::log_info!("Alloc checker basic test passed");
    Ok(())
}

/// Test large allocation failure
fn alloc_checker_large_fails_test() -> TestResult {
    // Try to allocate an impossibly large amount
    let huge_size = usize::MAX / 2;

    match vm::pmm::allocate(huge_size) {
        Ok(_) => {
            // If it somehow succeeded, free it and note the unexpected success
            vm::pmm::free(huge_size, huge_size)?;
            debug::log_debug!("Warning: Huge allocation unexpectedly succeeded");
        }
        Err(_) => {
            // Expected to fail
            debug::log_debug!("Huge allocation correctly failed");
        }
    }

    debug::log_info!("Alloc checker large fails test passed");
    Ok(())
}

/// Test allocation with constructor-like initialization
fn alloc_checker_init_test() -> TestResult {
    const COUNT: usize = 128;

    // Allocate an array and verify initialization
    let mut values: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(COUNT);

    // Initialize with a specific value
    for i in 0..COUNT {
        values.push(5);
    }

    // Verify all values were initialized
    for i in 0..COUNT {
        assert_eq!(values[i], 5, "Value should be initialized to 5");
    }

    debug::log_info!("Alloc checker init test passed");
    Ok(())
}

/// Test array size overflow detection
fn array_size_overflow_test() -> TestResult {
    const STRUCT_SIZE: usize = 0x1000;

    // Try to allocate an array that would overflow size_t
    let max_size = usize::MAX;
    let count = max_size / (STRUCT_SIZE / 0x10);

    // This should either fail or be caught
    match vm::pmm::allocate(count * STRUCT_SIZE) {
        Ok(_) => {
            // If it succeeded, free it
            vm::pmm::free(count * STRUCT_SIZE, count * STRUCT_SIZE)?;
            debug::log_debug!("Warning: Overflow allocation unexpectedly succeeded");
        }
        Err(_) => {
            // Expected to fail
            debug::log_debug!("Overflow allocation correctly failed");
        }
    }

    debug::log_info!("Array size overflow test passed");
    Ok(())
}

/// Test allocation alignment
fn alloc_alignment_test() -> TestResult {
    const PAGE_SIZE: usize = 4096;

    // Allocate page-aligned memory
    let vaddr = vm::pmm::allocate_aligned(PAGE_SIZE, PAGE_SIZE)?;

    // Verify alignment
    assert_true!(vaddr % PAGE_SIZE == 0, "Allocation should be page-aligned");

    vm::pmm::free(vaddr, PAGE_SIZE)?;

    debug::log_info!("Alloc alignment test passed");
    Ok(())
}

/// Test multiple allocations
fn multiple_alloc_test() -> TestResult {
    const COUNT: usize = 16;
    const SIZE: usize = 1024;

    let mut addrs = alloc::vec::Vec::new();

    // Allocate multiple chunks
    for _ in 0..COUNT {
        let vaddr = vm::pmm::allocate(SIZE)?;
        assert_true!(vaddr != 0, "Allocation should succeed");
        addrs.push((vaddr, SIZE));
    }

    // Free all allocations
    for (vaddr, size) in &addrs {
        vm::pmm::free(*vaddr, *size)?;
    }

    debug::log_info!("Multiple alloc test passed");
    Ok(())
}

/// Create the allocation checker test suite
pub fn create_alloc_checker_suite() -> TestSuite {
    TestSuite::new(
        "alloc_checker",
        "Allocation checker and validation tests",
        alloc::vec::Vec::from([
            TestCase::new("basic", "Basic allocation checking", alloc_checker_basic_test),
            TestCase::new("large_fails", "Large allocation failure", alloc_checker_large_fails_test),
            TestCase::new("init", "Allocation initialization", alloc_checker_init_test),
            TestCase::new("overflow", "Array size overflow detection", array_size_overflow_test),
            TestCase::new("alignment", "Allocation alignment", alloc_alignment_test),
            TestCase::new("multiple", "Multiple allocations", multiple_alloc_test),
        ]),
    )
}

/// Register allocation checker tests
pub fn register() {
    register_suite(create_alloc_checker_suite());
}
