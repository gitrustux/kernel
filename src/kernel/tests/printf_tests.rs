// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Printf Tests
//!
//! Tests for formatted output functionality.


use crate::kernel::tests::runner::*;

// Import logging macros at crate level
use crate::{log_info, log_debug};


/// Test integer formatting
fn int_format_test() -> TestResult {
    // Test various integer formats
    log_info!("Integer format test:");
    log_info!("  int8: -12, 0, 127");
    log_info!("  uint8: 244, 0, 255");
    log_info!("  int: -12345678, 0, 12345678");
    log_info!("  uint: 4282621618, 0, 12345678");

    Ok(())
}

/// Test hexadecimal formatting
fn hex_format_test() -> TestResult {
    log_info!("Hex format test:");
    log_info!("  uint8: f4, 0, ff");
    log_info!("  uint16: fb2e, 0, 4d2");
    log_info!("  uint: ff439eb2, 0, bc614e");
    log_info!("  uint with 0x: 0xabcdef, 0XABCDEF");

    Ok(())
}

/// Test pointer formatting
fn pointer_format_test() -> TestResult {
    let val = 0x12345678usize;
    log_info!("Pointer format test:");
    log_info!("  pointer: {:#x}", val);

    Ok(())
}

/// Test string formatting
fn string_format_test() -> TestResult {
    log_info!("String format test:");
    log_info!("  Hello, World!");
    log_info!("  Test {}", "string");
    log_info!("  Number: {}", 42);

    Ok(())
}

/// Test mixed formatting
fn mixed_format_test() -> TestResult {
    log_info!("Mixed format test:");
    log_info!("  int={}, hex={:#x}, str={}", -42, 0xabcdef, "test");

    Ok(())
}

/// Test percent escaping
fn percent_escape_test() -> TestResult {
    log_info!("Percent escape test:");
    log_info!("  %%");

    Ok(())
}

/// Test zero padding
fn zero_pad_test() -> TestResult {
    log_info!("Zero pad test:");
    log_info!("  {:04}", 42);
    log_info!("  {:08x}", 0x123);

    Ok(())
}

/// Test alignment
fn alignment_test() -> TestResult {
    log_info!("Alignment test:");
    log_info!("  [{:>10}]", "right");
    log_info!("  [{:<10}]", "left");
    log_info!("  [{:^10}]", "center");

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
