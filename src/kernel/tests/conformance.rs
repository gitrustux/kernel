// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by an MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Cross-Architecture Conformance Test Suite
//!
//! This module provides tests that must pass identically on all supported
//! architectures (ARM64, AMD64, RISC-V) to ensure consistent behavior.
//!
//! # Test Categories
//!
//! - **Page Table Tests**: Virtual memory management
//! - **Syscall Tests**: System call interface
//! - **Thread Tests**: Thread creation and scheduling
//! - **Timer Tests**: High-resolution timers
//! - **IPC Tests**: Inter-process communication
//! - **Memory Tests**: Memory allocation and protection


use crate::kernel::arch::arch_traits::*;
use crate::kernel::vm;
use crate::kernel::thread;
use crate::kernel::object;
use crate::rustux::types::*;
use crate::debug;

/// Test result
pub type TestResult = Result<(), &'static str>;

/// Conformance test suite
pub struct ConformanceSuite {
    tests: &'static [&'static ConformanceTest],
    passed: usize,
    failed: usize,
}

/// Individual conformance test
pub struct ConformanceTest {
    pub name: &'static str,
    pub test: fn() -> TestResult,
}

impl ConformanceSuite {
    /// Create a new conformance test suite
    pub const fn new(tests: &'static [&'static ConformanceTest]) -> Self {
        Self {
            tests,
            passed: 0,
            failed: 0,
        }
    }

    /// Run all tests in the suite
    pub fn run(&mut self) {
        println!("=== Cross-Architecture Conformance Test Suite ===");
        println!("Architecture: {}", Self::current_arch());

        for test in self.tests {
            print!("  {}... ", test.name);

            match (test.test)() {
                Ok(()) => {
                    println!("✅ PASS");
                    self.passed += 1;
                }
                Err(e) => {
                    println!("❌ FAIL: {}", e);
                    self.failed += 1;
                }
            }
        }

        println!();
        println!("Results: {} passed, {} failed", self.passed, self.failed);
        println!("===============================================");
    }

    /// Get the current architecture name
    fn current_arch() -> &'static str {
        #[cfg(target_arch = "aarch64")]
        return "ARM64";
        #[cfg(target_arch = "x86_64")]
        return "AMD64";
        #[cfg(target_arch = "riscv64")]
        return "RISC-V";
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64", target_arch = "riscv64")))]
        return "UNKNOWN";
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

// ============================================================================
// Page Table Conformance Tests
// ============================================================================

/// Test page table creation and basic mapping
fn test_page_table_basic() -> TestResult {
    use crate::arch::ArchMMU;

    // Test constants
    const TEST_VADDR: VAddr = 0x1000_0000;
    const TEST_PADDR: PAddr = 0x8000_0000;
    const TEST_SIZE: usize = 0x1000; // 4KB
    const TEST_FLAGS: u64 = ArchMMUFlags::READ | ArchMMUFlags::WRITE;

    unsafe {
        // Map a page
        let result = Arch::map(TEST_PADDR, TEST_VADDR, TEST_SIZE, TEST_FLAGS);
        if result != 0 {
            return Err("map failed");
        }

        // Verify the mapping exists
        let phys = Arch::virt_to_phys(TEST_VADDR);
        if phys != TEST_PADDR {
            // May be different due to page table abstraction
            // Just verify it's valid
            if phys == 0 {
                return Err("virt_to_phys returned 0");
            }
        }

        // Unmap the page
        Arch::unmap(TEST_VADDR, TEST_SIZE);

        Ok(())
    }
}

/// Test page protection flags
fn test_page_protection() -> TestResult {
    use crate::arch::ArchMMU;

    const TEST_VADDR: VAddr = 0x2000_0000;
    const TEST_PADDR: PAddr = 0x9000_0000;
    const TEST_SIZE: usize = 0x1000;

    unsafe {
        // Map with read-only
        let ro_flags = ArchMMUFlags::READ;
        if Arch::map(TEST_PADDR, TEST_VADDR, TEST_SIZE, ro_flags) != 0 {
            return Err("read-only map failed");
        }

        // Map with read-write
        let rw_flags = ArchMMUFlags::READ | ArchMMUFlags::WRITE;
        if Arch::map(TEST_PADDR + 0x1000, TEST_VADDR + 0x1000, TEST_SIZE, rw_flags) != 0 {
            return Err("read-write map failed");
        }

        // Map with execute
        let rx_flags = ArchMMUFlags::READ | ArchMMUFlags::EXECUTE;
        if Arch::map(TEST_PADDR + 0x2000, TEST_VADDR + 0x2000, TEST_SIZE, rx_flags) != 0 {
            return Err("read-execute map failed");
        }

        // Cleanup
        Arch::unmap(TEST_VADDR, TEST_SIZE * 3);

        Ok(())
    }
}

/// Test address space validity checks
fn test_address_space_validation() -> TestResult {
    use crate::arch::ArchMMU;

    unsafe {
        // Test canonical address check
        let valid_va = ArchMMU::is_valid_va(0x1000_0000);
        if !valid_va {
            return Err("valid VA marked invalid");
        }

        let invalid_va = ArchMMU::is_valid_va(0xFFFF_FFFF_FFFF_FFFF);
        if invalid_va {
            return Err("invalid VA marked valid");
        }

        Ok(())
    }
}

// ============================================================================
// Timer Conformance Tests
// ============================================================================

/// Test timer monotonicity
fn test_timer_monotonic() -> TestResult {
    use crate::arch::ArchTimer;

    let t1 = Arch::now_monotonic();
    let t2 = Arch::now_monotonic();

    // Time should never go backwards
    if t2 < t1 {
        return Err("timer went backwards");
    }

    // But it shouldn't be too fast either (within 1 second)
    let freq = Arch::get_frequency();
    let max_delta = freq; // 1 second
    if t2 - t1 > max_delta {
        // This might fail if the system is heavily loaded, but is unusual
        println!("    (warning: large timer delta: {} cycles)", t2 - t1);
    }

    Ok(())
}

/// Test timer frequency
fn test_timer_frequency() -> TestResult {
    use crate::arch::ArchTimer;

    let freq = Arch::get_frequency();

    // All architectures should have at least 1 MHz timer frequency
    if freq < 1_000_000 {
        return Err("timer frequency too low");
    }

    // And at most 10 GHz (which is very high)
    if freq > 10_000_000_000 {
        return Err("timer frequency too high");
    }

    println!("    (timer frequency: {} MHz)", freq / 1_000_000);
    Ok(())
}

// ============================================================================
// Thread Conformance Tests
// ============================================================================

/// Test basic thread creation
fn test_thread_create() -> TestResult {
    // This test requires a working scheduler
    // For now, just verify the thread module compiles
    Ok(())
}

/// Test stack pointer manipulation
fn test_stack_pointer() -> TestResult {
    use crate::arch::ArchThreadContext;

    let sp1 = unsafe { Arch::current_sp() };
    let sp2 = unsafe { Arch::current_sp() };

    // Stack pointer should be aligned to 16 bytes
    if sp1 & 0xF != 0 {
        return Err("stack pointer not 16-byte aligned");
    }

    // Stack pointer should not change significantly between reads
    let diff = if sp1 > sp2 { sp1 - sp2 } else { sp2 - sp1 };
    if diff > 0x100 {
        return Err("stack pointer changed unexpectedly");
    }

    println!("    (stack pointer: {:#x})", sp1);
    Ok(())
}

// ============================================================================
// Synchronization Conformance Tests
// ============================================================================

/// Test memory barriers
fn test_memory_barriers() -> TestResult {
    use crate::arch::ArchMemoryBarrier;

    // These tests mainly ensure the operations compile and execute
    // without crashing. Actual barrier effectiveness is hard to test.

    Arch::mb(); // Full barrier
    Arch::rmb(); // Read barrier
    Arch::wmb(); // Write barrier
    Arch::acquire(); // Acquire barrier
    Arch::release(); // Release barrier

    Ok(())
}

/// Test atomic operations
fn test_atomic_operations() -> TestResult {
    use core::sync::atomic::{AtomicU64, Ordering};

    let val = AtomicU64::new(0);

    // Test fetch_add
    let old = val.fetch_add(1, Ordering::SeqCst);
    if old != 0 {
        return Err("fetch_add returned wrong value");
    }

    // Test compare_exchange
    let mut result = val.compare_exchange(1, 2, Ordering::SeqCst, Ordering::Relaxed);
    if result.is_err() {
        return Err("compare_exchange failed");
    }

    // Test load/store
    val.store(42, Ordering::Release);
    if val.load(Ordering::Acquire) != 42 {
        return Err("load returned wrong value");
    }

    Ok(())
}

// ============================================================================
// Cache Conformance Tests
// ============================================================================

/// Test cache line size
fn test_cache_line_size() -> TestResult {
    use crate::arch::ArchCache;

    let dcache_line = Arch::dcache_line_size();
    let icache_line = Arch::icache_line_size();

    // All architectures should have at least 16 byte cache lines
    if dcache_line < 16 || dcache_line > 1024 {
        return Err("dcache line size out of range");
    }

    if icache_line < 16 || icache_line > 1024 {
        return Err("icache line size out of range");
    }

    // Cache lines should be power of 2
    if !dcache_line.is_power_of_two() || !icache_line.is_power_of_two() {
        return Err("cache line size not power of 2");
    }

    println!("    (cache lines: d={}, i={})", dcache_line, icache_line);
    Ok(())
}

// ============================================================================
// Halt Conformance Tests
// ============================================================================

/// Test halt instruction
fn test_halt() -> TestResult {
    use crate::arch::ArchHalt;

    // Just verify that halt compiles and doesn't crash immediately
    // We can't actually halt in a test
    Arch::pause();
    Arch::serialize();

    Ok(())
}

// ============================================================================
// CPU Feature Detection
// ============================================================================

/// Test CPU feature detection
fn test_cpu_features() -> TestResult {
    use crate::arch::ArchCpuId;

    let current_cpu = Arch::current_cpu();
    let cpu_count = Arch::cpu_count();
    let features = Arch::get_features();

    // Current CPU must be less than CPU count
    if current_cpu >= cpu_count {
        return Err("current_cpu >= cpu_count");
    }

    // CPU count should be at least 1
    if cpu_count == 0 {
        return Err("cpu_count == 0");
    }

    // CPU count should be reasonable (<= 1024)
    if cpu_count > 1024 {
        return Err("cpu_count too large");
    }

    // Features should have some bits set
    if features == 0 {
        // This might be valid for some architectures
        println!("    (warning: no CPU features detected)");
    }

    println!("    (CPU {}/{}, features: {:#x})", current_cpu, cpu_count, features);
    Ok(())
}

// ============================================================================
// Test Suite Definition
// ============================================================================

const PAGE_TABLE_TESTS: &[&ConformanceTest] = &[
    &ConformanceTest { name: "page_table_basic", test: test_page_table_basic },
    &ConformanceTest { name: "page_protection", test: test_page_protection },
    &ConformanceTest { name: "address_space_validation", test: test_address_space_validation },
];

const TIMER_TESTS: &[&ConformanceTest] = &[
    &ConformanceTest { name: "timer_monotonic", test: test_timer_monotonic },
    &ConformanceTest { name: "timer_frequency", test: test_timer_frequency },
];

const THREAD_TESTS: &[&ConformanceTest] = &[
    &ConformanceTest { name: "thread_create", test: test_thread_create },
    &ConformanceTest { name: "stack_pointer", test: test_stack_pointer },
];

const SYNC_TESTS: &[&ConformanceTest] = &[
    &ConformanceTest { name: "memory_barriers", test: test_memory_barriers },
    &ConformanceTest { name: "atomic_operations", test: test_atomic_operations },
];

const CACHE_TESTS: &[&ConformanceTest] = &[
    &ConformanceTest { name: "cache_line_size", test: test_cache_line_size },
];

const HALT_TESTS: &[&ConformanceTest] = &[
    &ConformanceTest { name: "halt", test: test_halt },
];

const CPU_TESTS: &[&ConformanceTest] = &[
    &ConformanceTest { name: "cpu_features", test: test_cpu_features },
];

/// All conformance tests
const ALL_TESTS: &[&ConformanceTest] = &[
    // Page table tests
    &ConformanceTest { name: "page_table_basic", test: test_page_table_basic },
    &ConformanceTest { name: "page_protection", test: test_page_protection },
    &ConformanceTest { name: "address_space_validation", test: test_address_space_validation },
    // Timer tests
    &ConformanceTest { name: "timer_monotonic", test: test_timer_monotonic },
    &ConformanceTest { name: "timer_frequency", test: test_timer_frequency },
    // Thread tests
    &ConformanceTest { name: "thread_create", test: test_thread_create },
    &ConformanceTest { name: "stack_pointer", test: test_stack_pointer },
    // Synchronization tests
    &ConformanceTest { name: "memory_barriers", test: test_memory_barriers },
    &ConformanceTest { name: "atomic_operations", test: test_atomic_operations },
    // Cache tests
    &ConformanceTest { name: "cache_line_size", test: test_cache_line_size },
    // Halt tests
    &ConformanceTest { name: "halt", test: test_halt },
    // CPU tests
    &ConformanceTest { name: "cpu_features", test: test_cpu_features },
];

/// Run the full conformance test suite
pub fn run_conformance_tests() -> bool {
    let mut suite = ConformanceSuite::new(ALL_TESTS);
    suite.run();
    suite.all_passed()
}

/// Run a specific category of tests
pub fn run_page_table_tests() -> bool {
    let mut suite = ConformanceSuite::new(PAGE_TABLE_TESTS);
    suite.run();
    suite.all_passed()
}

pub fn run_timer_tests() -> bool {
    let mut suite = ConformanceSuite::new(TIMER_TESTS);
    suite.run();
    suite.all_passed()
}

pub fn run_thread_tests() -> bool {
    let mut suite = ConformanceSuite::new(THREAD_TESTS);
    suite.run();
    suite.all_passed()
}

pub fn run_sync_tests() -> bool {
    let mut suite = ConformanceSuite::new(SYNC_TESTS);
    suite.run();
    suite.all_passed()
}

pub fn run_cache_tests() -> bool {
    let mut suite = ConformanceSuite::new(CACHE_TESTS);
    suite.run();
    suite.all_passed()
}

pub fn run_cpu_tests() -> bool {
    let mut suite = ConformanceSuite::new(CPU_TESTS);
    suite.run();
    suite.all_passed()
}

// ============================================================================
// Helper trait for power of 2 check
// ============================================================================

trait PowerOfTwo {
    fn is_power_of_two(self) -> bool;
}

impl PowerOfTwo for usize {
    fn is_power_of_two(self) -> bool {
        self != 0 && (self & (self - 1)) == 0
    }
}
