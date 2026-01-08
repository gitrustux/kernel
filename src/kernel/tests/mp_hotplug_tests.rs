// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! MP Hotplug Tests
//!
//! Tests for CPU hotplug functionality.


use crate::kernel::tests::runner::*;
use crate::kernel::mp;
use crate::kernel::thread;
use crate::debug;

/// Test getting CPU count
fn cpu_count_test() -> TestResult {
    let cpu_count = mp::cpu_count();

    assert_ge!(cpu_count, 1, "Should have at least 1 CPU");
    assert_le!(cpu_count, 1024, "CPU count unreasonably high");

    debug::log_info!("CPU count: {}", cpu_count);
    Ok(())
}

/// Test getting online CPU mask
fn online_mask_test() -> TestResult {
    let mask = mp::get_online_mask();

    assert_ne!(mask, 0, "At least one CPU should be online");

    // Count bits in mask
    let count = mask.count_ones();
    debug::log_info!("Online CPUs: {}", count);

    Ok(())
}

/// Test current CPU ID
fn current_cpu_test() -> TestResult {
    let current = mp::current_cpu();
    let online_mask = mp::get_online_mask();

    assert!(online_mask & (1 << current) != 0, "Current CPU should be online");

    debug::log_info!("Current CPU: {}", current);
    Ok(())
}

/// Test CPU affinity
fn cpu_affinity_test() -> TestResult {
    let current = thread::current_thread();
    let original_affinity = current.cpu_affinity();

    // Try setting affinity to current CPU
    current.set_cpu_affinity(1 << mp::current_cpu())?;

    // Restore original affinity
    current.set_cpu_affinity(original_affinity)?;

    debug::log_info!("CPU affinity test passed");
    Ok(())
}

/// Test CPU hotplug (x86-64 only)
#[cfg(target_arch = "x86_64")]
fn cpu_hotplug_test() -> TestResult {
    use crate::kernel::mp;
    use crate::kernel::timer;

    let cpu_count = mp::cpu_count();

    if cpu_count < 2 {
        test_skip!("Need at least 2 CPUs");
    }

    debug::log_info!("CPU hotplug test (requires SMP support)");
    debug::log_info!("Note: Full hotplug testing requires hardware support");

    // Basic test: just verify we can query hotplug state
    for cpu_id in 0..cpu_count {
        let online = mp::is_cpu_online(cpu_id);
        debug::log_debug!("CPU {} online: {}", cpu_id, online);
    }

    Ok(())
}

/// Test CPU hotplug (other architectures - skip)
#[cfg(not(target_arch = "x86_64"))]
fn cpu_hotplug_test() -> TestResult {
    test_skip!("CPU hotplug only supported on x86-64");
}

/// Test per-CPU timer access
fn percpu_timer_test() -> TestResult {
    use crate::arch::ArchTimer;

    let cpu_count = mp::cpu_count();
    let mut frequencies = alloc::vec::Vec::new();

    for cpu_id in 0..cpu_count {
        if !mp::is_cpu_online(cpu_id) {
            continue;
        }

        // Set affinity to this CPU
        let current = thread::current_thread();
        let original_affinity = current.cpu_affinity();
        current.set_cpu_affinity(1 << cpu_id)?;

        // Read timer frequency
        let freq = Arch::get_frequency();
        frequencies.push((cpu_id, freq));

        // Restore affinity
        current.set_cpu_affinity(original_affinity)?;
    }

    // All CPUs should have the same timer frequency
    if frequencies.len() > 1 {
        let first_freq = frequencies[0].1;
        for (cpu_id, freq) in &frequencies {
            assert_eq!(*freq, first_freq, "Timer frequency should be consistent");
        }
    }

    debug::log_info!("Per-CPU timer test passed");
    Ok(())
}

/// Create the MP hotplug test suite
pub fn create_mp_hotplug_suite() -> TestSuite {
    TestSuite::new(
        "mp_hotplug",
        "Multi-processor and hotplug tests",
        alloc::vec::Vec::from([
            TestCase::new("cpu_count", "CPU count", cpu_count_test),
            TestCase::new("online_mask", "Online CPU mask", online_mask_test),
            TestCase::new("current_cpu", "Current CPU ID", current_cpu_test),
            TestCase::new("affinity", "CPU affinity", cpu_affinity_test),
            TestCase::new("hotplug", "CPU hotplug", cpu_hotplug_test),
            TestCase::new("percpu_timer", "Per-CPU timer", percpu_timer_test),
        ]),
    )
}

/// Register MP hotplug tests
pub fn register() {
    register_suite(create_mp_hotplug_suite());
}
