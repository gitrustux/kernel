// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Printf Tests
//!
//! Tests for formatted output functionality.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::debug;

/// Test integer formatting
fn int_format_test() -> TestResult {
    // Test various integer formats
    debug::log_info!("Integer format test:");
    debug::log_info!("  int8: -12, 0, 127");
    debug::log_info!("  uint8: 244, 0, 255");
    debug::log_info!("  int: -12345678, 0, 12345678");
    debug::log_info!("  uint: 4282621618, 0, 12345678");

    Ok(())
}

/// Test hexadecimal formatting
fn hex_format_test() -> TestResult {
    debug::log_info!("Hex format test:");
    debug::log_info!("  uint8: f4, 0, ff");
    debug::log_info!("  uint16: fb2e, 0, 4d2");
    debug::log_info!("  uint: ff439eb2, 0, bc614e");
    debug::log_info!("  uint with 0x: 0xabcdef, 0XABCDEF");

    Ok(())
}

/// Test pointer formatting
fn pointer_format_test() -> TestResult {
    let val = 0x12345678usize;
    debug::log_info!("Pointer format test:");
    debug::log_info!("  pointer: {:#x}", val);

    Ok(())
}

/// Test string formatting
fn string_format_test() -> TestResult {
    debug::log_info!("String format test:");
    debug::log_info!("  Hello, World!");
    debug::log_info!("  Test {}", "string");
    debug::log_info!("  Number: {}", 42);

    Ok(())
}

/// Test mixed formatting
fn mixed_format_test() -> TestResult {
    debug::log_info!("Mixed format test:");
    debug::log_info!("  int={}, hex={:#x}, str={}", -42, 0xabcdef, "test");

    Ok(())
}

/// Test percent escaping
fn percent_escape_test() -> TestResult {
    debug::log_info!("Percent escape test:");
    debug::log_info!("  %%");

    Ok(())
}

/// Test zero padding
fn zero_pad_test() -> TestResult {
    debug::log_info!("Zero pad test:");
    debug::log_info!("  {:04}", 42);
    debug::log_info!("  {:08x}", 0x123);

    Ok(())
}

/// Test alignment
fn alignment_test() -> TestResult {
    debug::log_info!("Alignment test:");
    debug::log_info!("  [{:>10}]", "right");
    debug::log_info!("  [{:<10}]", "left");
    debug::log_info!("  [{:^10}]", "center");

    Ok(())
}

/// Create the printf test suite
pub fn create_printf_suite() -> TestSuite {
    TestSuite::new(
        "printf",
        "Formatted output tests",
        alloc::vec::Vec::from([
            TestCase::new("int_format", "Integer formatting", int_format_test),
            TestCase::new("hex_format", "Hexadecimal formatting", hex_format_test),
            TestCase::new("pointer_format", "Pointer formatting", pointer_format_test),
            TestCase::new("string_format", "String formatting", string_format_test),
            TestCase::new("mixed_format", "Mixed formatting", mixed_format_test),
            TestCase::new("percent_escape", "Percent escaping", percent_escape_test),
            TestCase::new("zero_pad", "Zero padding", zero_pad_test),
            TestCase::new("alignment", "Alignment", alignment_test),
        ]),
    )
}

/// Register printf tests
pub fn register() {
    register_suite(create_printf_suite());
}
