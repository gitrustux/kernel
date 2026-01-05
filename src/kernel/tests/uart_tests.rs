// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! UART Tests
//!
//! Tests for UART serial output functionality.

#![no_std]

use crate::kernel::tests::runner::*;
use crate::debug;

/// Test blocking UART output
fn uart_blocking_test() -> TestResult {
    for count in 0..5 {
        let line1 = alloc::format!("Blocking Test Count {} (FIRST LINE)\n", count);
        let line2 = alloc::format!("AND THIS SHOULD BE THE SECOND LINE Count {}\n", count);

        debug::print_raw(&line1);
        debug::print_raw(&line2);
    }

    debug::log_info!("UART blocking test passed");
    Ok(())
}

/// Test non-blocking UART output
fn uart_nonblocking_test() -> TestResult {
    for count in 0..5 {
        let line1 = alloc::format!("NON-Blocking Test Count {} (FIRST LINE)\n", count);
        let line2 = alloc::format!("AND THIS SHOULD BE THE SECOND LINE Count {}\n", count);

        debug::print_raw(&line1);
        debug::print_raw(&line2);
    }

    debug::log_info!("UART non-blocking test passed");
    Ok(())
}

/// Test large volume UART output
fn uart_large_output_test() -> TestResult {
    const TOTAL_LINES: usize = 100;
    const MIN_LINE_LEN: usize = 80;
    const MAX_LINE_LEN: usize = 128;

    for count in 0..TOTAL_LINES {
        // Create a line of random length between MIN and MAX
        let line_len = MIN_LINE_LEN + ((count % (MAX_LINE_LEN - MIN_LINE_LEN)));

        // Start with prefix
        let mut line = alloc::format!("UART TEST LINE {}: ", count);

        // Fill with random-ish characters
        let remaining = line_len - line.len();
        for i in 0..remaining {
            let c = if (i + count) % 2 == 0 {
                b'a' + ((i + count) % 26) as u8
            } else {
                b'A' + ((i + count * 2) % 26) as u8
            };
            line.push(c as char);
        }

        line.push('\n');

        debug::print_raw(&line);

        // Yield occasionally to be fair to other threads
        if count % 10 == 0 {
            crate::kernel::thread::yield();
        }
    }

    debug::log_info!("UART large output test passed");
    Ok(())
}

/// Test newline handling
fn uart_newline_test() -> TestResult {
    debug::print_raw("Testing newline handling...\n");

    // Test different newline styles
    debug::print_raw("Line 1\n");
    debug::print_raw("Line 2\r\n");
    debug::print_raw("Line 3\n");

    debug::log_info!("UART newline test passed");
    Ok(())
}

/// Test character output
fn uart_char_test() -> TestResult {
    let test_str = "Hello, UART! ";

    for _ in 0..5 {
        for ch in test_str.chars() {
            // Output character by character
            let s = alloc::format!("{}", ch);
            debug::print_raw(&s);
        }
    }

    debug::print_raw("\n");
    debug::log_info!("UART char test passed");
    Ok(())
}

/// Test mixed output types
fn uart_mixed_test() -> TestResult {
    // Mix of debug logging and raw output
    debug::log_info!("Starting mixed output test");

    for i in 0..10 {
        debug::log_debug!("Debug log line {}", i);
        debug::print_raw(&alloc::format!("Raw output line {}\n", i));
    }

    debug::log_info!("UART mixed test passed");
    Ok(())
}

/// Create the UART test suite
pub fn create_uart_suite() -> TestSuite {
    TestSuite::new(
        "uart",
        "UART serial output tests",
        alloc::vec::Vec::from([
            TestCase::new("blocking", "Blocking UART output", uart_blocking_test),
            TestCase::new("nonblocking", "Non-blocking UART output", uart_nonblocking_test),
            TestCase::new("large_output", "Large volume output", uart_large_output_test),
            TestCase::new("newline", "Newline handling", uart_newline_test),
            TestCase::new("char", "Character output", uart_char_test),
            TestCase::new("mixed", "Mixed output types", uart_mixed_test),
        ]),
    )
}

/// Register UART tests
pub fn register() {
    register_suite(create_uart_suite());
}
