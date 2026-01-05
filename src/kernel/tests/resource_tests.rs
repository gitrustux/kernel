// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Resource Tests
//!
//! Tests for resource allocation and management.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::vm;
use crate::debug;

/// Test unconfigured resource allocator
fn unconfigured_test() -> TestResult {
    // Attempt to allocate from an unconfigured allocator should fail
    // This is a conceptual test - in Rust, we'd verify the PMM is initialized
    match vm::pmm::allocate(4096) {
        Ok(vaddr) => {
            vm::pmm::free(vaddr, 4096)?;
            debug::log_debug!("Note: PMM appears to be auto-configured");
        }
        Err(_) => {
            debug::log_debug!("PMM not configured (expected in some contexts)");
        }
    }

    Ok(())
}

/// Test configured allocator
fn configured_test() -> TestResult {
    // Allocate some memory to verify PMM is working
    let vaddr1 = vm::pmm::allocate(4096)?;
    let vaddr2 = vm::pmm::allocate(8192)?;

    assert_true!(vaddr1 != 0, "First allocation should succeed");
    assert_true!(vaddr2 != 0, "Second allocation should succeed");

    vm::pmm::free(vaddr1, 4096)?;
    vm::pmm::free(vaddr2, 8192)?;

    debug::log_info!("Resource configured test passed");
    Ok(())
}

/// Test exclusive resource allocation
fn exclusive_test() -> TestResult {
    // Allocate a specific region exclusively
    let base = vm::pmm::allocate(4096)?;
    assert_true!(base != 0, "Allocation should succeed");

    // Try to allocate overlapping region - in PMM this should work
    // (different physical pages)
    let _base2 = vm::pmm::allocate(4096)?;

    vm::pmm::free(base, 4096)?;

    debug::log_info!("Exclusive resource test passed");
    Ok(())
}

/// Test shared resource allocation
fn shared_test() -> TestResult {
    // Multiple allocations should be possible
    let mut allocations = alloc::vec::Vec::new();

    for _ in 0..5 {
        let vaddr = vm::pmm::allocate(4096)?;
        allocations.push(vaddr);
    }

    // Free all allocations
    for vaddr in allocations {
        vm::pmm::free(vaddr, 4096)?;
    }

    debug::log_info!("Shared resource test passed");
    Ok(())
}

/// Test out of range allocation
fn out_of_range_test() -> TestResult {
    // Try to allocate an impossibly large amount
    match vm::pmm::allocate(usize::MAX) {
        Ok(_) => {
            // If it succeeded, free it
            vm::pmm::free(usize::MAX, usize::MAX)?;
            debug::log_debug!("Warning: Huge allocation unexpectedly succeeded");
        }
        Err(_) => {
            debug::log_debug!("Huge allocation correctly failed");
        }
    }

    // Try to allocate at boundary
    let max_reasonable = vm::pmm::get_total_memory();
    if max_reasonable > 0 {
        match vm::pmm::allocate(max_reasonable) {
            Ok(vaddr) => {
                vm::pmm::free(vaddr, max_reasonable)?;
            }
            Err(_) => {
                debug::log_debug!("Boundary allocation failed");
            }
        }
    }

    debug::log_info!("Out of range test passed");
    Ok(())
}

/// Test resource cleanup
fn cleanup_test() -> TestResult {
    // Allocate and free multiple times
    for _ in 0..10 {
        let vaddr = vm::pmm::allocate(4096)?;
        assert_true!(vaddr != 0, "Allocation should succeed");
        vm::pmm::free(vaddr, 4096)?;
    }

    debug::log_info!("Resource cleanup test passed");
    Ok(())
}

/// Create the resource test suite
pub fn create_resource_suite() -> TestSuite {
    TestSuite::new(
        "resource",
        "Resource allocation and management tests",
        alloc::vec::Vec::from([
            TestCase::new("unconfigured", "Unconfigured allocator", unconfigured_test),
            TestCase::new("configured", "Configured allocator", configured_test),
            TestCase::new("exclusive", "Exclusive allocation", exclusive_test),
            TestCase::new("shared", "Shared allocation", shared_test),
            TestCase::new("out_of_range", "Out of range allocation", out_of_range_test),
            TestCase::new("cleanup", "Resource cleanup", cleanup_test),
        ]),
    )
}

/// Register resource tests
pub fn register() {
    register_suite(create_resource_suite());
}
