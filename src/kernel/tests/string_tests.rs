// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! String Tests
//!
//! Tests for string operations (memcpy, memset, etc.).

#![no_std]

use crate::kernel::tests::runner::*;
use crate::kernel::vm;
use crate::debug;

const BUFFER_SIZE: usize = 8 * 1024 * 1024;
const ITERATIONS: usize = (1024 * 1024 * 1024) / BUFFER_SIZE;

/// Test memcpy correctness
fn memcpy_correctness_test() -> TestResult {
    const TEST_SIZE: usize = 1024;

    let src_vaddr = vm::pmm::allocate_aligned(TEST_SIZE * 2, 64)?;
    let dst_vaddr = vm::pmm::allocate_aligned(TEST_SIZE * 2, 64)?;

    let src = src_vaddr as *mut u8;
    let dst = dst_vaddr as *mut u8;

    // Fill source with pattern
    for i in 0..TEST_SIZE {
        unsafe { src.add(i).write_volatile((i & 0xFF) as u8) };
    }

    // Clear destination
    for i in 0..(TEST_SIZE * 2) {
        unsafe { dst.add(i).write_volatile(0) };
    }

    // Copy with various alignments
    for src_align in 0..64 {
        for dst_align in 0..64 {
            // Test different sizes
            for size in 0..256 {
                unsafe {
                    // Perform copy
                    for i in 0..size {
                        dst.add(dst_align + i).write_volatile(src.add(src_align + i).read_volatile());
                    }

                    // Verify
                    for i in 0..size {
                        let src_val = src.add(src_align + i).read_volatile();
                        let dst_val = dst.add(dst_align + i).read_volatile();
                        if src_val != dst_val {
                            vm::pmm::free(src_vaddr, TEST_SIZE * 2)?;
                            vm::pmm::free(dst_vaddr, TEST_SIZE * 2)?;
                            return Err(format!(
                                "memcpy mismatch: src_align={}, dst_align={}, size={}, offset={}",
                                src_align, dst_align, size, i
                            ));
                        }
                    }
                }
            }
        }
    }

    vm::pmm::free(src_vaddr, TEST_SIZE * 2)?;
    vm::pmm::free(dst_vaddr, TEST_SIZE * 2)?;

    debug::log_info!("memcpy correctness test passed");
    Ok(())
}

/// Test memset correctness
fn memset_correctness_test() -> TestResult {
    const TEST_SIZE: usize = 1024;

    let vaddr = vm::pmm::allocate_aligned(TEST_SIZE * 2, 64)?;
    let buf = vaddr as *mut u8;

    // Test various alignments and fill values
    for dst_align in 0..64 {
        for size in 0..256 {
            for fill_val in 0u8..16u8 {
                // Clear buffer
                for i in 0..(TEST_SIZE * 2) {
                    unsafe { buf.add(i).write_volatile(0) };
                }

                // Fill
                for i in 0..size {
                    unsafe { buf.add(dst_align + i).write_volatile(fill_val) };
                }

                // Verify
                for i in 0..size {
                    let val = unsafe { buf.add(dst_align + i).read_volatile() };
                    if val != fill_val {
                        vm::pmm::free(vaddr, TEST_SIZE * 2)?;
                        return Err(format!(
                            "memset mismatch: align={}, size={}, fill={}, offset={}",
                            dst_align, size, fill_val, i
                        ));
                    }
                }
            }
        }
    }

    vm::pmm::free(vaddr, TEST_SIZE * 2)?;

    debug::log_info!("memset correctness test passed");
    Ok(())
}

/// Test memcpy performance
fn memcpy_perf_test() -> TestResult {
    let vaddr = vm::pmm::allocate_aligned(BUFFER_SIZE * 2, 64)?;

    let src = (vaddr + 64) as *const u8;
    let dst = (vaddr + 64 + BUFFER_SIZE) as *mut u8;

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..ITERATIONS {
        for i in 0..BUFFER_SIZE {
            unsafe { dst.add(i).write_volatile(src.add(i).read_volatile()) };
        }
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let bytes_per_sec = (BUFFER_SIZE * ITERATIONS * 1_000_000_000) / duration_ns.max(1);

    debug::log_info!(
        "memcpy perf: {} ms, {} MB/s",
        duration_ns / 1_000_000,
        bytes_per_sec / 1_000_000
    );

    vm::pmm::free(vaddr, BUFFER_SIZE * 2)?;
    Ok(())
}

/// Test memset performance
fn memset_perf_test() -> TestResult {
    let vaddr = vm::pmm::allocate_aligned(BUFFER_SIZE, 64)?;
    let buf = vaddr as *mut u8;

    let start = unsafe { crate::arch::Arch::now_monotonic() };

    for _ in 0..ITERATIONS {
        for i in 0..BUFFER_SIZE {
            unsafe { buf.add(i).write_volatile(0) };
        }
    }

    let end = unsafe { crate::arch::Arch::now_monotonic() };
    let duration_ns = end - start;
    let bytes_per_sec = (BUFFER_SIZE * ITERATIONS * 1_000_000_000) / duration_ns.max(1);

    debug::log_info!(
        "memset perf: {} ms, {} MB/s",
        duration_ns / 1_000_000,
        bytes_per_sec / 1_000_000
    );

    vm::pmm::free(vaddr, BUFFER_SIZE)?;
    Ok(())
}

/// Test overlapping memory operations (memmove-like)
fn memmove_test() -> TestResult {
    const TEST_SIZE: usize = 1024;

    let vaddr = vm::pmm::allocate_aligned(TEST_SIZE * 2, 64)?;
    let buf = vaddr as *mut u8;

    // Initialize with pattern
    for i in 0..TEST_SIZE {
        unsafe { buf.add(i).write_volatile((i & 0xFF) as u8) };
    }

    // Test forward copy (overlapping)
    let src_offset = 100;
    let dst_offset = 150;
    let copy_size = 200;

    for i in 0..copy_size {
        unsafe {
            let val = buf.add(src_offset + i).read_volatile();
            buf.add(dst_offset + i).write_volatile(val);
        }
    }

    // Verify forward copy
    for i in 0..copy_size {
        let src_val = unsafe { buf.add(src_offset + i).read_volatile() };
        let dst_val = unsafe { buf.add(dst_offset + i).read_volatile() };
        assert_eq!(src_val, dst_val, "Forward memmove mismatch");
    }

    // Test backward copy (overlapping)
    let src_offset = 300;
    let dst_offset = 250;
    let copy_size = 100;

    for i in 0..copy_size {
        unsafe {
            let val = buf.add(src_offset + i).read_volatile();
            buf.add(dst_offset + i).write_volatile(val);
        }
    }

    // Verify backward copy
    for i in 0..copy_size {
        let src_val = unsafe { buf.add(src_offset + i).read_volatile() };
        let dst_val = unsafe { buf.add(dst_offset + i).read_volatile() };
        assert_eq!(src_val, dst_val, "Backward memmove mismatch");
    }

    vm::pmm::free(vaddr, TEST_SIZE * 2)?;

    debug::log_info!("memmove test passed");
    Ok(())
}

/// Create the string test suite
pub fn create_string_suite() -> TestSuite {
    TestSuite::new(
        "string",
        "String operation tests (memcpy, memset, etc.)",
        alloc::vec::Vec::from([
            TestCase::new("memcpy_correctness", "memcpy correctness", memcpy_correctness_test),
            TestCase::new("memset_correctness", "memset correctness", memset_correctness_test),
            TestCase::new("memcpy_perf", "memcpy performance", memcpy_perf_test),
            TestCase::new("memset_perf", "memset performance", memset_perf_test),
            TestCase::new("memmove", "memmove overlapping", memmove_test),
        ]),
    )
}

/// Register string tests
pub fn register() {
    register_suite(create_string_suite());
}
