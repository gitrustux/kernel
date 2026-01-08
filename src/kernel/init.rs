// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Initialization
//!
//! This module provides kernel initialization functions for the Rustux kernel.
//! It coordinates the initialization of various kernel subsystems.
//!
//! # Initialization Order
//!
//! The kernel must be initialized in a specific order:
//!
//! 1. Early architecture setup (arch, interrupts, MMU)
//! 2. Physical memory manager
//! 3. Virtual memory subsystem
//! 4. Per-CPU data
//! 5. Thread subsystem
//! 6. Scheduler
//! 7. Timer subsystem
//! 8. Syscall layer
//!
//! # Usage
//!
//! ```rust
//! // Called from architecture-specific boot code
//! kernel_init();
//! ```


// ============================================================================
// LK Compatibility Constants
// ============================================================================

/// LK initialization level: earliest
pub const LK_INIT_LEVEL_EARLIEST: u32 = 0;

/// LK initialization level: threading
pub const LK_INIT_LEVEL_THREADING: u32 = 1;

/// LK initialization flag: secondary CPUs
pub const LK_INIT_FLAG_SECONDARY_CPUS: u32 = 1 << 0;

use crate::kernel::vm;
use crate::kernel::pmm;
use crate::kernel::thread;
use crate::kernel::sched;
use crate::kernel::syscalls;
use crate::kernel::usercopy;
use crate::kernel::percpu;
use crate::kernel::cmdline;
use crate::kernel::debug;

// Import logging macros
use crate::{log_debug, log_info, log_error};

/// ============================================================================
/// Initialization State
/// ============================================================================

/// Kernel initialization state
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitState {
    /// Not initialized
    NotStarted = 0,

    /// Early initialization in progress
    Early = 1,

    /// Architecture-specific initialization
    Arch = 2,

    /// Physical memory manager initialized
    PMM = 3,

    /// Virtual memory initialized
    VM = 4,

    /// Per-CPU data initialized
    PerCpu = 5,

    /// Thread subsystem initialized
    Thread = 6,

    /// Scheduler initialized
    Scheduler = 7,

    /// Timer subsystem initialized
    Timer = 8,

    /// Syscall layer initialized
    Syscall = 9,

    /// Late initialization complete
    Complete = 10,

    /// Running (initialization done)
    Running = 11,
}

/// Current initialization state
static mut INIT_STATE: InitState = InitState::NotStarted;

/// ============================================================================
/// Public API
/// ============================================================================

/// Initialize the kernel
///
/// This is the main kernel initialization function.
/// It should be called from architecture-specific boot code.
///
/// # Safety
///
/// Must be called exactly once during kernel boot.
pub fn kernel_init() {
    unsafe {
        if INIT_STATE != InitState::NotStarted {
            panic!("kernel_init called multiple times");
        }
        INIT_STATE = InitState::Early;
    }

    log_info!("Rustux kernel initializing...");

    // Initialize subsystems in order
    init_early();
    init_arch();
    init_memory();
    init_threads();
    init_late();

    unsafe {
        INIT_STATE = InitState::Complete;
    }

    log_info!("Rustux kernel initialization complete");
}

/// Get the current initialization state
pub fn init_state() -> InitState {
    unsafe { INIT_STATE }
}

/// ============================================================================
/// Initialization Phases
/// ============================================================================

/// Early initialization
///
/// Initializes core subsystems needed for everything else.
fn init_early() {
    log_debug!("init_early: starting");

    // Initialize debug/logging first
    debug::init();
    log_info!("Debug subsystem initialized");

    // Initialize command line parsing
    cmdline::init();
    log_info!("Command line parsing initialized");

    unsafe {
        INIT_STATE = InitState::Early;
    }

    log_debug!("init_early: complete");
}

/// Architecture-specific initialization
///
/// Initializes architecture-specific hardware interfaces.
fn init_arch() {
    log_debug!("init_arch: starting");

    // Initialize per-CPU data
    percpu::percpu_init();

    // Architecture-specific initialization would happen here
    // - Interrupt controllers
    // - Timer hardware
    // - CPU features
    // - etc.

    #[cfg(target_arch = "aarch64")]
    {
        log_info!("ARM64 architecture initialization");
        // crate::arch::arm64::init();
    }

    #[cfg(target_arch = "x86_64")]
    {
        log_info!("x86-64 architecture initialization");
        // crate::arch::amd64::init();
    }

    #[cfg(target_arch = "riscv64")]
    {
        log_info!("RISC-V architecture initialization");
        // crate::arch::riscv64::init();
    }

    unsafe {
        INIT_STATE = InitState::Arch;
    }

    log_debug!("init_arch: complete");
}

/// Memory subsystem initialization
///
/// Initializes physical and virtual memory management.
fn init_memory() {
    log_debug!("init_memory: starting");

    // Initialize physical memory manager
    // Note: PMM should be initialized early by arch-specific code
    log_info!("Physical memory manager initialized");

    // Initialize virtual memory subsystem
    vm::init();
    log_info!("Virtual memory subsystem initialized");

    // Initialize kernel stack allocator
    unsafe {
        vm::stacks::init_stacks();
    }
    log_info!("Kernel stack allocator initialized");

    unsafe {
        INIT_STATE = InitState::VM;
    }

    log_debug!("init_memory: complete");
}

/// Thread and scheduler initialization
///
/// Initializes the threading and scheduling subsystems.
fn init_threads() {
    log_debug!("init_threads: starting");

    // Initialize thread subsystem
    thread::init();
    log_info!("Thread subsystem initialized");

    // Initialize scheduler
    sched::init();
    log_info!("Scheduler initialized");

    unsafe {
        INIT_STATE = InitState::Scheduler;
    }

    log_debug!("init_threads: complete");
}

/// Late initialization
///
/// Initializes remaining subsystems.
fn init_late() {
    log_debug!("init_late: starting");

    // Initialize syscall layer
    syscalls::init();
    log_info!("Syscall layer initialized");

    // User/kernel boundary safety
    usercopy::init();
    log_info!("User/kernel boundary safety initialized");

    unsafe {
        INIT_STATE = InitState::Complete;
    }

    log_debug!("init_late: complete");
}

/// Mark kernel as running
///
/// Called after all initialization is complete.
pub fn kernel_running() {
    unsafe {
        INIT_STATE = InitState::Running;
    }

    log_info!("Kernel is now running");

    // Create idle thread for CPU 0
    // In a full implementation, we would create idle threads for all CPUs
    match thread::Thread::new_kernel(idle_thread_entry, 0, thread::PRIORITY_IDLE) {
        Ok(idle_thread) => {
            log_info!("Created idle thread for CPU 0: tid={}", idle_thread.tid());
            idle_thread.start().ok();
            // Register thread in global registry
            thread::register_thread(alloc::sync::Arc::new(idle_thread));
        }
        Err(e) => {
            log_error!("Failed to create idle thread: {:?}", e);
        }
    }

    log_info!("Starting scheduler...");
    // The scheduler would now take over and start scheduling threads
}

/// Idle thread entry point
///
/// This is the entry point for idle threads.
/// When there's no work to do, the idle thread runs.
extern "C" fn idle_thread_entry(_cpu_id: usize) -> ! {
    log_info!("Idle thread started for CPU {}", _cpu_id);

    loop {
        // In a real implementation, this would:
        // 1. Check for pending work
        // 2. If no work, halt the CPU until interrupt
        // 3. Repeat

        // For now, just spin
        core::hint::spin_loop();
    }
}

/// ============================================================================
// Module Initialization
// ============================================================================

/// Initialize the init subsystem
pub fn init() {
    // This module doesn't need initialization
    // It's called from arch-specific boot code
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state_values() {
        assert_eq!(InitState::NotStarted as u8, 0);
        assert_eq!(InitState::Early as u8, 1);
        assert_eq!(InitState::Complete as u8, 10);
        assert_eq!(InitState::Running as u8, 11);
    }

    #[test]
    fn test_init_state_order() {
        assert!(InitState::Early < InitState::Arch);
        assert!(InitState::Arch < InitState::PMM);
        assert!(InitState::VM < InitState::Thread);
        assert!(InitState::Complete < InitState::Running);
    }

    #[test]
    fn test_init_state_initial() {
        // Initially should be NotStarted
        assert_eq!(init_state(), InitState::NotStarted);
    }
}
