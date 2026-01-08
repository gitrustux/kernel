// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux Kernel Test Suite
//!
//! This module provides a comprehensive test framework for the Rustux kernel.
//! It replaces the C++ STATIC_COMMAND-based test system with a modern Rust-based
//! test runner.
//!
//! # Organization
//!
//! - [`runner`] - Core test runner framework
//! - [`conformance`] - Cross-architecture conformance tests
//! - [`thread_tests`] - Thread management tests
//! - [`timer_tests`] - Timer functionality tests
//! - [`mem_tests`] - Memory management tests
//! - [`alloc_checker_tests`] - Allocation validation tests
//! - [`benchmarks`] - Performance microbenchmarks
//! - [`cache_tests`] - CPU cache operations tests
//! - [`clock_tests`] - Clock and timer tests
//! - [`fibo`] - Threaded Fibonacci computation tests
//! - [`lock_dep_tests`] - Lock dependency tests
//! - [`mp_hotplug_tests`] - Multi-processor and hotplug tests
//! - [`preempt_disable_tests`] - Preemption disable tests
//! - [`printf_tests`] - Formatted output tests
//! - [`resource_tests`] - Resource allocation tests
//! - [`sleep_tests`] - Thread sleep tests
//! - [`string_tests`] - String operations tests
//! - [`sync_ipi_tests`] - Inter-processor interrupt tests
//! - [`uart_tests`] - UART serial output tests
//!
//! # Running Tests
//!
//! ```rust
//! use crate::kernel::tests;
//!
//! // Run all tests
//! let summary = tests::run_all();
//!
//! // Run a specific suite
//! let summary = tests::run_suite("thread");
//!
//! // List available suites
//! tests::list();
//! ```


pub mod runner;
pub mod conformance;
pub mod thread_tests;
pub mod timer_tests;
pub mod mem_tests;
pub mod alloc_checker_tests;
pub mod benchmarks;
pub mod cache_tests;
pub mod clock_tests;
pub mod fibo;
pub mod lock_dep_tests;
pub mod mp_hotplug_tests;
pub mod preempt_disable_tests;
pub mod printf_tests;
pub mod resource_tests;
pub mod sleep_tests;
pub mod string_tests;
pub mod sync_ipi_tests;
pub mod uart_tests;

// Re-exports for convenience
pub use runner::*;
pub use conformance::*;

/// Initialize the test framework
pub fn init() {
    runner::init();

    // Register all test suites
    thread_tests::register();
    timer_tests::register();
    mem_tests::register();
    conformance::register();
    alloc_checker_tests::register();
    benchmarks::register();
    cache_tests::register();
    clock_tests::register();
    fibo::register();
    lock_dep_tests::register();
    mp_hotplug_tests::register();
    preempt_disable_tests::register();
    printf_tests::register();
    resource_tests::register();
    sleep_tests::register();
    string_tests::register();
    sync_ipi_tests::register();
    uart_tests::register();
}

/// Run all registered test suites
pub fn run_all() -> runner::RegistrySummary {
    runner::run_all_tests()
}

/// Run a specific test suite by name
pub fn run_suite(name: &str) -> Option<runner::SuiteSummary> {
    runner::run_test_suite(name)
}

/// List all available test suites
pub fn list() {
    runner::list_test_suites()
}

/// Test command handler
///
/// This function is called from the kernel shell to run test commands.
/// Usage:
///   run all                  - Run all tests
///   run <suite>             - Run specific test suite
///   run list                - List available suites
pub fn handle_test_command(args: &[&str]) -> i32 {
    if args.is_empty() {
        list();
        return 0;
    }

    match args[0] {
        "all" => {
            let summary = run_all();
            if summary.all_passed() {
                0
            } else {
                1
            }
        }
        "list" => {
            list();
            0
        }
        suite_name => {
            if let Some(summary) = run_suite(suite_name) {
                if summary.all_passed() {
                    0
                } else {
                    1
                }
            } else {
                1
            }
        }
    }
}

// ============================================================================
// Legacy Test Commands (for compatibility with C++ test framework)
// ============================================================================

/// Legacy thread_tests command
#[no_mangle]
pub extern "C" fn rustux_thread_tests() -> i32 {
    println!("Running Rust thread tests...");
    if let Some(summary) = run_suite("thread") {
        println!("Thread tests complete: {}/{} passed", summary.passed, summary.total);
        if summary.all_passed() { 0 } else { 1 }
    } else {
        1
    }
}

/// Legacy timer_tests command
#[no_mangle]
pub extern "C" fn rustux_timer_tests() -> i32 {
    println!("Running Rust timer tests...");
    if let Some(summary) = run_suite("timer") {
        println!("Timer tests complete: {}/{} passed", summary.passed, summary.total);
        if summary.all_passed() { 0 } else { 1 }
    } else {
        1
    }
}

/// Legacy mem_test command
#[no_mangle]
pub extern "C" fn rustux_mem_test() -> i32 {
    println!("Running Rust memory tests...");
    if let Some(summary) = run_suite("memory") {
        println!("Memory tests complete: {}/{} passed", summary.passed, summary.total);
        if summary.all_passed() { 0 } else { 1 }
    } else {
        1
    }
}

/// Legacy clock_tests command (uses conformance tests)
#[no_mangle]
pub extern "C" fn rustux_clock_tests() -> i32 {
    println!("Running Rust clock/timer tests...");
    if let Some(summary) = run_suite("timer") {
        println!("Clock tests complete: {}/{} passed", summary.passed, summary.total);
        if summary.all_passed() { 0 } else { 1 }
    } else {
        1
    }
}

/// Run all unit tests
#[no_mangle]
pub extern "C" fn rustux_unittests() -> i32 {
    println!("Running all Rust unit tests...");
    let summary = run_all();
    println!("Unit tests complete: {}/{} passed", summary.passed, summary.total);
    if summary.all_passed() { 0 } else { 1 }
}

// ============================================================================
// Module Initialization
// ============================================================================

/// Module initialization function
pub fn module_init() {
    init();
    debug::log_info!("Rust test suite loaded");
    debug::log_info!("  Available suites:");
    debug::log_info!("    - thread (thread management)");
    debug::log_info!("    - timer (timer functionality)");
    debug::log_info!("    - memory (memory management)");
    debug::log_info!("    - conformance (cross-architecture)");
    debug::log_info!("    - alloc_checker (allocation validation)");
    debug::log_info!("    - benchmarks (performance)");
    debug::log_info!("    - cache (CPU cache operations)");
    debug::log_info!("    - clock (clock and timer)");
    debug::log_info!("    - fibo (threaded Fibonacci)");
    debug::log_info!("    - lock_dep (lock dependencies)");
    debug::log_info!("    - mp_hotplug (multi-processor)");
    debug::log_info!("    - preempt_disable (preemption control)");
    debug::log_info!("    - printf (formatted output)");
    debug::log_info!("    - resource (resource allocation)");
    debug::log_info!("    - sleep (thread sleep)");
    debug::log_info!("    - string (string operations)");
    debug::log_info!("    - sync_ipi (inter-processor interrupts)");
    debug::log_info!("    - uart (UART serial output)");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framework_init() {
        init();
        // Should not panic
    }

    #[test]
    fn test_list_suites() {
        init();
        list();
        // Should not panic
    }
}
