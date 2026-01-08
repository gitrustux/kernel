// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Memory Tests
//!
//! This module contains tests for memory functionality, including:
//! - Memory pattern testing
//! - Moving inversion tests
//! - Address write/read verification
//!
//! Converted from C++ mem_tests.cpp


use crate::kernel::tests::runner::*;
use crate::kernel::vm;
use crate::debug;
use core::ptr;

// ============================================================================
// Memory Test Patterns
// ============================================================================

/// Standard test patterns
const PATTERNS: &[u32] = &[
    0x00000000,
    0xFFFFFFFF,
    0xAAAAAAAA,
    0x55555555,
];

/// Moving inversion patterns
const MOVING_PATTERNS: &[u32] = &[
    0x00000000,
    0xFFFFFFFF,
    0xAAAAAAAA,
    0x55555555,
];

// ============================================================================
// Memory Test Helper Functions
// ============================================================================

/// Write a pattern to memory
fn write_pattern(ptr: *mut u32, len: usize, pattern: u32) {
    let count = len / 4;
    for i in 0..count {
        unsafe {
            ptr.add(i).write_volatile(pattern);
        }
    }
}

/// Verify a pattern in memory
fn verify_pattern(ptr: *const u32, len: usize, pattern: u32) -> TestResult {
    let count = len / 4;
    for i in 0..count {
        unsafe {
            let val = ptr.add(i).read_volatile();
            if val != pattern {
                return Err(format!(
                    "Pattern mismatch at offset {:#x}: expected {:#x}, got {:#x}",
                    i * 4, pattern, val
                ));
            }
        }
    }
    Ok(())
}

/// Moving inversion test
fn moving_inversion_test(ptr: *mut u32, len: usize, pattern: u32) -> TestResult {
    let count = len / 4;

    // Fill memory with pattern
    write_pattern(ptr, len, pattern);

    // From bottom, walk through each cell, inverting the value
    for i in 0..count {
        unsafe {
            let val = ptr.add(i).read_volatile();
            if val != pattern {
                return Err(format!(
                    "Moving inversion verify failed at offset {:#x}: expected {:#x}, got {:#x}",
                    i * 4, pattern, val
                ));
            }
            ptr.add(i).write_volatile(!pattern);
        }
    }

    // From top down, verify inverted and restore
    for i in (0..count).rev() {
        unsafe {
            let val = ptr.add(i).read_volatile();
            if val != !pattern {
                return Err(format!(
                    "Moving inversion invert failed at offset {:#x}: expected {:#x}, got {:#x}",
                    i * 4, !pattern, val
                ));
            }
            ptr.add(i).write_volatile(pattern);
        }
    }

    // Final verification
    verify_pattern(ptr, len, pattern)?;

    Ok(())
}

/// Shift bits through 32-bit word
fn shift_bits_test(ptr: *mut u32, len: usize) -> TestResult {
    let mut pattern: u32 = 1;
    while pattern != 0 {
        write_pattern(ptr, len, pattern);
        verify_pattern(ptr, len, pattern)?;
        pattern = pattern.wrapping_shl(1);
    }
    Ok(())
}

/// Shift bits through 16-bit word with inverted top
fn shift_bits_inverted_test(ptr: *mut u32, len: usize) -> TestResult {
    let mut pattern: u16 = 1;
    while pattern != 0 {
        let full_pattern = ((!(pattern as u32)) << 16) | (pattern as u32);
        write_pattern(ptr, len, full_pattern);
        verify_pattern(ptr, len, full_pattern)?;
        pattern = pattern.wrapping_shl(1);
    }
    Ok(())
}

// ============================================================================
// Basic Memory Tests
// ============================================================================

/// Test 1: Simple address write, read back
fn simple_address_test() -> TestResult {
    // Use a small buffer from the stack for this test
    const TEST_SIZE: usize = 256; // 64 u32 words

    let mut buffer: [u32; TEST_SIZE] = [0; TEST_SIZE];
    let ptr = buffer.as_mut_ptr();

    // Write addresses
    for i in 0..TEST_SIZE {
        unsafe {
            ptr.add(i).write_volatile(i as u32);
        }
    }

    // Read back and verify
    for i in 0..TEST_SIZE {
        unsafe {
            let val = ptr.add(i).read_volatile();
            if val != i as u32 {
                return Err(format!(
                    "Address test failed at index {}: expected {}, got {}",
                    i, i, val
                ));
            }
        }
    }

    debug::log_info!("Simple address test passed");
    Ok(())
}

/// Test 2: Write patterns, read back
fn pattern_test() -> TestResult {
    const TEST_SIZE: usize = 256;

    let mut buffer: [u32; TEST_SIZE] = [0; TEST_SIZE];
    let ptr = buffer.as_mut_ptr();

    // Test static patterns
    for &pattern in PATTERNS {
        debug::log_debug!("Testing pattern {:#x}", pattern);
        write_pattern(ptr, TEST_SIZE * 4, pattern);
        verify_pattern(ptr, TEST_SIZE * 4, pattern)?;
    }

    // Test shifting bit patterns
    debug::log_debug!("Testing shifting bit patterns");
    shift_bits_test(ptr, TEST_SIZE * 4)?;

    // Test shifted inverted patterns
    debug::log_debug!("Testing shifted inverted patterns");
    shift_bits_inverted_test(ptr, TEST_SIZE * 4)?;

    debug::log_info!("Pattern test passed");
    Ok(())
}

/// Test 3: Moving inversions with patterns
fn moving_inversion_pattern_test() -> TestResult {
    const TEST_SIZE: usize = 256;

    let mut buffer: [u32; TEST_SIZE] = [0; TEST_SIZE];
    let ptr = buffer.as_mut_ptr();

    // Test standard patterns
    for &pattern in MOVING_PATTERNS {
        debug::log_debug!("Moving inversion with pattern {:#x}", pattern);
        moving_inversion_test(ptr, TEST_SIZE * 4, pattern)?;
    }

    // Test shifting patterns
    let mut pattern: u32 = 1;
    while pattern != 0 {
        debug::log_debug!("Moving inversion with shifting pattern {:#x}", pattern);
        moving_inversion_test(ptr, TEST_SIZE * 4, pattern)?;
        pattern = pattern.wrapping_shl(1);
    }

    // Test shifted inverted patterns
    let mut pattern: u16 = 1;
    while pattern != 0 {
        let full_pattern = ((!(pattern as u32)) << 16) | (pattern as u32);
        debug::log_debug!("Moving inversion with inverted pattern {:#x}", full_pattern);
        moving_inversion_test(ptr, TEST_SIZE * 4, full_pattern)?;
        pattern = pattern.wrapping_shl(1);
    }

    debug::log_info!("Moving inversion test passed");
    Ok(())
}

// ============================================================================
// Allocated Memory Tests
// ============================================================================

/// Test with dynamically allocated memory
fn allocated_memory_test() -> TestResult {
    use crate::kernel::vm;

    const TEST_SIZE: usize = 16 * 1024; // 16KB

    // Allocate a test region
    let vaddr = vm::pmm::allocate_contiguous(TEST_SIZE, vm::layout::KERNEL_HEAP_BASE)?;
    if vaddr == 0 {
        return Err("Failed to allocate test memory".to_string());
    }

    let ptr = vaddr as *mut u32;

    debug::log_debug!("Testing allocated memory at {:#x}, size {} bytes", vaddr, TEST_SIZE);

    // Run simple address test
    for i in 0..(TEST_SIZE / 4) {
        unsafe {
            ptr.add(i).write_volatile(i as u32);
        }
    }

    for i in 0..(TEST_SIZE / 4) {
        unsafe {
            let val = ptr.add(i).read_volatile();
            if val != i as u32 {
                // Clean up before returning error
                let _ = vm::pmm::free(vaddr, TEST_SIZE);
                return Err(format!(
                    "Allocated memory test failed at offset {:#x}: expected {:#x}, got {:#x}",
                    i * 4, i, val
                ));
            }
        }
    }

    // Run pattern tests
    for &pattern in PATTERNS {
        write_pattern(ptr, TEST_SIZE, pattern);
        if let Err(e) = verify_pattern(ptr, TEST_SIZE, pattern) {
            let _ = vm::pmm::free(vaddr, TEST_SIZE);
            return Err(e);
        }
    }

    // Free the memory
    vm::pmm::free(vaddr, TEST_SIZE)?;

    debug::log_info!("Allocated memory test passed");
    Ok(())
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Test memory alignment
fn alignment_test() -> TestResult {
    const TEST_SIZE: usize = 128;

    let mut buffer: [u32; TEST_SIZE] = [0; TEST_SIZE];

    // Test aligned access
    let aligned_ptr = buffer.as_mut_ptr();
    for i in 0..TEST_SIZE {
        unsafe {
            aligned_ptr.add(i).write_volatile(0xDEADBEEF);
            let val = aligned_ptr.add(i).read_volatile();
            if val != 0xDEADBEEF {
                return Err(format!("Aligned access failed at index {}", i));
            }
        }
    }

    debug::log_info!("Alignment test passed");
    Ok(())
}

/// Test memory boundaries
fn boundary_test() -> TestResult {
    const TEST_SIZE: usize = 64;

    let mut buffer: [u32; TEST_SIZE] = [0; TEST_SIZE];
    let ptr = buffer.as_mut_ptr();

    // Test first word
    unsafe {
        ptr.write_volatile(0x11111111);
        if ptr.read_volatile() != 0x11111111 {
            return Err("First word test failed".to_string());
        }
    }

    // Test last word
    unsafe {
        ptr.add(TEST_SIZE - 1).write_volatile(0x22222222);
        if ptr.add(TEST_SIZE - 1).read_volatile() != 0x22222222 {
            return Err("Last word test failed".to_string());
        }
    }

    // Test middle word
    unsafe {
        ptr.add(TEST_SIZE / 2).write_volatile(0x33333333);
        if ptr.add(TEST_SIZE / 2).read_volatile() != 0x33333333 {
            return Err("Middle word test failed".to_string());
        }
    }

    debug::log_info!("Boundary test passed");
    Ok(())
}

/// Test zero initialization
fn zero_init_test() -> TestResult {
    const TEST_SIZE: usize = 256;

    let mut buffer: [u32; TEST_SIZE] = [0xFFFFFFFF; TEST_SIZE];

    // Clear to zero
    for i in 0..TEST_SIZE {
        buffer[i] = 0;
    }

    // Verify all zeros
    for i in 0..TEST_SIZE {
        if buffer[i] != 0 {
            return Err(format!("Zero init test failed at index {}: got {:#x}", i, buffer[i]));
        }
    }

    debug::log_info!("Zero init test passed");
    Ok(())
}

// ============================================================================
// Memory Performance Tests
// ============================================================================

/// Test sequential memory access performance
fn sequential_access_test() -> TestResult {
    const TEST_SIZE: usize = 4096; // 16KB

    let mut buffer: [u32; TEST_SIZE] = [0; TEST_SIZE];

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    // Sequential write
    for i in 0..TEST_SIZE {
        buffer[i] = i as u32;
    }

    // Sequential read
    let mut sum: u64 = 0;
    for i in 0..TEST_SIZE {
        sum += buffer[i] as u64;
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let duration_us = duration_ns / 1000;
    let bytes_per_sec = (TEST_SIZE * 4 * 2) as u64 * 1_000_000_000 / duration_ns.max(1);

    debug::log_debug!("Sequential access: {} us, {} MB/s", duration_us, bytes_per_sec / 1_000_000);

    // Verify sum
    let expected_sum: u64 = (0..TEST_SIZE).map(|i| i as u64).sum();
    if sum != expected_sum {
        return Err(format!("Sequential access sum mismatch: expected {}, got {}", expected_sum, sum));
    }

    debug::log_info!("Sequential access test passed");
    Ok(())
}

/// Test random memory access
fn random_access_test() -> TestResult {
    const TEST_SIZE: usize = 1024;
    const NUM_ACCESSES: usize = 10000;

    let mut buffer: [u32; TEST_SIZE] = [0; TEST_SIZE];

    // Initialize buffer with indices
    for i in 0..TEST_SIZE {
        buffer[i] = i as u32;
    }

    let mut sum: u64 = 0;

    // Simple pseudo-random access pattern
    let mut seed: u32 = 12345;
    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..NUM_ACCESSES {
        // Linear congruential generator
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let index = (seed as usize) % TEST_SIZE;
        sum += buffer[index] as u64;
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;

    debug::log_debug!("Random access: {} accesses, {} ns total, {} ns/access",
        NUM_ACCESSES, duration_ns, duration_ns / NUM_ACCESSES as u64);

    debug::log_info!("Random access test passed");
    Ok(())
}

// ============================================================================
// Test Suite Registration
// ============================================================================

/// Create the memory test suite
pub fn create_memory_suite() -> TestSuite {
    TestSuite::new(
        "memory",
        "Memory management tests",
        alloc::vec::Vec::from([
            TestCase::new("simple_address", "Simple address write/read test", simple_address_test),
            TestCase::new("pattern", "Pattern write/read test", pattern_test),
            TestCase::new("moving_inversion", "Moving inversion test", moving_inversion_pattern_test),
            TestCase::new("allocated_memory", "Allocated memory test", allocated_memory_test),
            TestCase::new("alignment", "Memory alignment test", alignment_test),
            TestCase::new("boundary", "Memory boundary test", boundary_test),
            TestCase::new("zero_init", "Zero initialization test", zero_init_test),
            TestCase::new("sequential_access", "Sequential access performance test", sequential_access_test),
            TestCase::new("random_access", "Random access test", random_access_test),
        ]),
    )
}

/// Register all memory tests
pub fn register() {
    register_suite(create_memory_suite());
}
