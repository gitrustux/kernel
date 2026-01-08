// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Test Runner Framework
//!
//! This module provides a unified test framework for the Rustux kernel.
//! It replaces the C++ STATIC_COMMAND-based test system with a Rust-based
//! test runner that can execute unit tests, integration tests, and benchmarks.
//!
//! # Features
//!
//! - **Test discovery**: Automatic test registration
//! - **Result tracking**: Pass/fail counting and reporting
//! - **Timing**: Per-test and suite timing
//! - **Hierarchical tests**: Support for test suites and individual tests
//! - **Assertions**: Macro-based assertion helpers
//!
//! # Usage
//!
//! ```rust
//! use crate::kernel::tests::runner::*;
//!
//! #[test_case]
//! fn test_example() -> TestResult {
//!     assert_eq!(1 + 1, 2);
//!     Ok(())
//! }
//! ```


use crate::debug;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Test result type
pub type TestResult = Result<(), String>;

/// Individual test case
pub struct TestCase {
    /// Test name
    pub name: &'static str,
    /// Test description
    pub description: &'static str,
    /// Test function
    pub test_fn: fn() -> TestResult,
}

impl TestCase {
    /// Create a new test case
    pub const fn new(
        name: &'static str,
        description: &'static str,
        test_fn: fn() -> TestResult,
    ) -> Self {
        Self {
            name,
            description,
            test_fn,
        }
    }

    /// Run the test case
    pub fn run(&self) -> TestOutcome {
        print!("  {}... ", self.name);

        let start = self.time_now();
        let result = (self.test_fn)();
        let duration = self.time_now() - start;

        match result {
            Ok(()) => {
                println!("✅ PASS ({} ns)", duration);
                TestOutcome::Passed { duration }
            }
            Err(e) => {
                println!("❌ FAIL: {}", e);
                TestOutcome::Failed {
                    duration,
                    error: e,
                }
            }
        }
    }

    /// Get current time in nanoseconds
    fn time_now(&self) -> u64 {
        #[cfg(feature = "kernel")]
        unsafe {
            crate::arch::Arch::now_monotonic()
        }
        #[cfg(not(feature = "kernel"))]
        0
    }
}

/// Test outcome
#[derive(Debug)]
pub enum TestOutcome {
    /// Test passed
    Passed { duration: u64 },
    /// Test failed
    Failed { duration: u64, error: String },
    /// Test was skipped
    Skipped,
}

impl TestOutcome {
    /// Check if test passed
    pub fn is_passed(&self) -> bool {
        matches!(self, TestOutcome::Passed { .. })
    }

    /// Get test duration
    pub fn duration(&self) -> u64 {
        match self {
            TestOutcome::Passed { duration } => *duration,
            TestOutcome::Failed { duration, .. } => *duration,
            TestOutcome::Skipped => 0,
        }
    }
}

/// Test suite
pub struct TestSuite {
    /// Suite name
    pub name: &'static str,
    /// Suite description
    pub description: &'static str,
    /// Test cases
    pub tests: Vec<TestCase>,
}

impl TestSuite {
    /// Create a new test suite
    pub const fn new(
        name: &'static str,
        description: &'static str,
        tests: Vec<TestCase>,
    ) -> Self {
        Self {
            name,
            description,
            tests,
        }
    }

    /// Run all tests in the suite
    pub fn run(&self) -> SuiteSummary {
        println!("\n=== Test Suite: {} ===", self.name);
        if !self.description.is_empty() {
            println!("{}", self.description);
        }
        println!("Running {} test(s)...", self.tests.len());

        let start = self.time_now();
        let mut outcomes = Vec::new();

        for test in &self.tests {
            outcomes.push(test.run());
        }

        let total_duration = self.time_now() - start;

        let passed = outcomes.iter().filter(|o| o.is_passed()).count();
        let failed = outcomes.len() - passed;

        println!("\n--- Results ---");
        println!(
            "Passed: {}/{} ({}%)",
            passed,
            outcomes.len(),
            (passed * 100 / outcomes.len().max(1))
        );
        if failed > 0 {
            println!("Failed: {}/{}", failed, outcomes.len());
        }
        println!("Total time: {} ns", total_duration);
        println!("====================\n");

        SuiteSummary {
            name: self.name,
            total: outcomes.len(),
            passed,
            failed,
            duration: total_duration,
        }
    }

    /// Get current time in nanoseconds
    fn time_now(&self) -> u64 {
        #[cfg(feature = "kernel")]
        unsafe {
            crate::arch::Arch::now_monotonic()
        }
        #[cfg(not(feature = "kernel"))]
        0
    }
}

/// Test suite summary
#[derive(Debug)]
pub struct SuiteSummary {
    /// Suite name
    pub name: &'static str,
    /// Total tests
    pub total: usize,
    /// Passed tests
    pub passed: usize,
    /// Failed tests
    pub failed: usize,
    /// Total duration in nanoseconds
    pub duration: u64,
}

impl SuiteSummary {
    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Get pass rate as percentage
    pub fn pass_rate(&self) -> usize {
        if self.total == 0 {
            return 100;
        }
        (self.passed * 100) / self.total
    }
}

/// Test registry
pub struct TestRegistry {
    /// Registered test suites
    suites: BTreeMap<&'static str, TestSuite>,
}

impl TestRegistry {
    /// Create a new test registry
    pub const fn new() -> Self {
        Self {
            suites: BTreeMap::new(),
        }
    }

    /// Register a test suite
    pub fn register(&mut self, suite: TestSuite) {
        let name = suite.name;
        self.suites.insert(name, suite);
    }

    /// Run all registered test suites
    pub fn run_all(&self) -> RegistrySummary {
        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║     Rustux Kernel Test Suite                       ║");
        println!("╚═══════════════════════════════════════════════════════╝");

        let start = self.time_now();
        let mut summaries = Vec::new();

        for (_, suite) in &self.suites {
            summaries.push(suite.run());
        }

        let total_duration = self.time_now() - start;

        let total: usize = summaries.iter().map(|s| s.total).sum();
        let passed: usize = summaries.iter().map(|s| s.passed).sum();
        let failed: usize = summaries.iter().map(|s| s.failed).sum();

        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║ Final Summary                                        ║");
        println!("╠═══════════════════════════════════════════════════════╣");
        println!("║ Total Suites:     {:>34} ║", self.suites.len());
        println!("║ Total Tests:      {:>34} ║", total);
        println!("║ Passed:           {:>34} ║", passed);
        println!("║ Failed:           {:>34} ║", failed);
        println!("║ Pass Rate:        {:>33}% ║", (passed * 100 / total.max(1)));
        println!("║ Total Duration:   {:>33} ns ║", total_duration);
        println!("╚═══════════════════════════════════════════════════════╝\n");

        RegistrySummary {
            suites: self.suites.len(),
            total,
            passed,
            failed,
            duration: total_duration,
        }
    }

    /// Run a specific test suite by name
    pub fn run_suite(&self, name: &str) -> Option<SuiteSummary> {
        if let Some(suite) = self.suites.get(name) {
            Some(suite.run())
        } else {
            println!("Test suite '{}' not found", name);
            println!("Available suites:");
            for suite_name in self.suites.keys() {
                println!("  - {}", suite_name);
            }
            None
        }
    }

    /// List all registered test suites
    pub fn list(&self) {
        println!("\nAvailable Test Suites:");
        println!("======================");
        for (name, suite) in &self.suites {
            println!("  {} - {} tests", name, suite.tests.len());
            println!("    {}", suite.description);
        }
        println!();
    }

    /// Get current time in nanoseconds
    fn time_now(&self) -> u64 {
        #[cfg(feature = "kernel")]
        unsafe {
            crate::arch::Arch::now_monotonic()
        }
        #[cfg(not(feature = "kernel"))]
        0
    }
}

/// Test registry summary
#[derive(Debug)]
pub struct RegistrySummary {
    /// Number of test suites
    pub suites: usize,
    /// Total tests
    pub total: usize,
    /// Passed tests
    pub passed: usize,
    /// Failed tests
    pub failed: usize,
    /// Total duration in nanoseconds
    pub duration: u64,
}

impl RegistrySummary {
    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Get pass rate as percentage
    pub fn pass_rate(&self) -> usize {
        if self.total == 0 {
            return 100;
        }
        (self.passed * 100) / self.total
    }
}

/// Global test registry (lazy_static would be ideal, but using a simpler approach)
static mut GLOBAL_REGISTRY: Option<TestRegistry> = None;

/// Get the global test registry
pub fn global_registry() -> &'static mut TestRegistry {
    unsafe {
        if GLOBAL_REGISTRY.is_none() {
            GLOBAL_REGISTRY = Some(TestRegistry::new());
        }
        GLOBAL_REGISTRY.as_mut().unwrap()
    }
}

/// Register a test suite with the global registry
pub fn register_suite(suite: TestSuite) {
    global_registry().register(suite);
}

/// Run all tests
pub fn run_all_tests() -> RegistrySummary {
    global_registry().run_all()
}

/// Run a specific test suite
pub fn run_test_suite(name: &str) -> Option<SuiteSummary> {
    global_registry().run_suite(name)
}

/// List all test suites
pub fn list_test_suites() {
    global_registry().list()
}

// ============================================================================
// Macros
// ============================================================================

/// Mark a function as a test case
///
/// # Usage
///
/// ```rust
/// #[test_case]
/// fn test_example() -> TestResult {
///     assert_eq!(1 + 1, 2);
///     Ok(())
/// }
/// ```
pub use rustux_macros::test_case;

/// Assertion macros
#[macro_export]
macro_rules! assert_true {
    ($expr:expr) => {
        if !$expr {
            return Err(format!("assertion failed: {} is not true", stringify!($expr)));
        }
    };
    ($expr:expr, $msg:expr) => {
        if !$expr {
            return Err(format!("assertion failed: {} ({} is not true)", $msg, stringify!($expr)));
        }
    };
}

#[macro_export]
macro_rules! assert_false {
    ($expr:expr) => {
        if $expr {
            return Err(format!("assertion failed: {} is not false", stringify!($expr)));
        }
    };
}

#[macro_export]
macro_rules! assert_eq {
    ($left:expr, $right:expr) => {
        {
            let left_val = &$left;
            let right_val = &$right;
            if left_val != right_val {
                return Err(format!(
                    "assertion failed: {} == {}\n  left: {:?}\n  right: {:?}",
                    stringify!($left),
                    stringify!($right),
                    left_val,
                    right_val
                ));
            }
        }
    };
    ($left:expr, $right:expr, $msg:expr) => {
        {
            let left_val = &$left;
            let right_val = &$right;
            if left_val != right_val {
                return Err(format!(
                    "assertion failed: {} ({} == {})\n  left: {:?}\n  right: {:?}",
                    $msg,
                    stringify!($left),
                    stringify!($right),
                    left_val,
                    right_val
                ));
            }
        }
    };
}

#[macro_export]
macro_rules! assert_ne {
    ($left:expr, $right:expr) => {
        {
            let left_val = &$left;
            let right_val = &$right;
            if left_val == right_val {
                return Err(format!(
                    "assertion failed: {} != {}\n  both are: {:?}",
                    stringify!($left),
                    stringify!($right),
                    left_val
                ));
            }
        }
    };
}

#[macro_export]
macro_rules! assert_gt {
    ($left:expr, $right:expr) => {
        {
            let left_val = &$left;
            let right_val = &$right;
            if left_val <= right_val {
                return Err(format!(
                    "assertion failed: {} > {}\n  left: {:?}\n  right: {:?}",
                    stringify!($left),
                    stringify!($right),
                    left_val,
                    right_val
                ));
            }
        }
    };
}

#[macro_export]
macro_rules! assert_ge {
    ($left:expr, $right:expr) => {
        {
            let left_val = &$left;
            let right_val = &$right;
            if left_val < right_val {
                return Err(format!(
                    "assertion failed: {} >= {}\n  left: {:?}\n  right: {:?}",
                    stringify!($left),
                    stringify!($right),
                    left_val,
                    right_val
                ));
            }
        }
    };
}

#[macro_export]
macro_rules! assert_lt {
    ($left:expr, $right:expr) => {
        {
            let left_val = &$left;
            let right_val = &$right;
            if left_val >= right_val {
                return Err(format!(
                    "assertion failed: {} < {}\n  left: {:?}\n  right: {:?}",
                    stringify!($left),
                    stringify!($right),
                    left_val,
                    right_val
                ));
            }
        }
    };
}

#[macro_export]
macro_rules! assert_le {
    ($left:expr, $right:expr) => {
        {
            let left_val = &$left;
            let right_val = &$right;
            if left_val > right_val {
                return Err(format!(
                    "assertion failed: {} <= {}\n  left: {:?}\n  right: {:?}",
                    stringify!($left),
                    stringify!($right),
                    left_val,
                    right_val
                ));
            }
        }
    };
}

/// Panic with a test error
#[macro_export]
macro_rules! test_fail {
    ($msg:expr) => {
        return Err(format!("test failed: {}", $msg));
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err(format!("test failed: {}", format!($fmt, $($arg)*)));
    };
}

/// Skip a test
#[macro_export]
macro_rules! test_skip {
    ($msg:expr) => {
        println!("  ⏭️  SKIP: {}", $msg);
        return Ok(());
    };
}

// ============================================================================
// Module Initialization
// ============================================================================

/// Initialize the test framework
pub fn init() {
    debug::log_info!("Test framework initialized");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_case_creation() {
        let tc = TestCase::new("test", "description", || Ok(()));
        assert_eq!(tc.name, "test");
        assert_eq!(tc.description, "description");
    }

    #[test]
    fn test_test_outcome() {
        let outcome = TestOutcome::Passed { duration: 100 };
        assert!(outcome.is_passed());
        assert_eq!(outcome.duration(), 100);

        let outcome = TestOutcome::Failed {
            duration: 200,
            error: String::from("error"),
        };
        assert!(!outcome.is_passed());
        assert_eq!(outcome.duration(), 200);
    }

    #[test]
    fn test_suite_summary() {
        let summary = SuiteSummary {
            name: "test",
            total: 10,
            passed: 8,
            failed: 2,
            duration: 1000,
        };
        assert!(!summary.all_passed());
        assert_eq!(summary.pass_rate(), 80);

        let summary = SuiteSummary {
            name: "test",
            total: 5,
            passed: 5,
            failed: 0,
            duration: 500,
        };
        assert!(summary.all_passed());
        assert_eq!(summary.pass_rate(), 100);
    }
}
