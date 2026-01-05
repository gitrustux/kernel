// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Per-CPU Data
//!
//! This module provides per-CPU data storage for the Rustux kernel.
//! Each CPU has its own data area for performance and synchronization.
//!
//! # Design
//!
//! - **Per-CPU allocation**: Each CPU gets its own copy of data
//! - **Fast access**: No locking needed for per-CPU data
//! - **Cache-friendly**: Data is local to the CPU
//! - **Aligned**: Properly aligned for SMP access
//!
//! # Usage
//!
//! ```rust
//! // Get the current CPU's data
//! let cpu_data = get_local_percpu();
//!
//! // Access per-CPU fields
//! cpu_data.current_thread = some_tid;
//! ```

#![no_std]

use crate::kernel::thread::ThreadId;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

// Import logging macros
use crate::{log_debug, log_info};

/// ============================================================================
/// Constants
/// ============================================================================

/// Maximum number of CPUs supported
pub const SMP_MAX_CPUS: usize = 256;

/// CPU ID for the boot processor
pub const BOOT_CPU_ID: u32 = 0;

/// ============================================================================
/// Per-CPU Data
/// ============================================================================

/// Per-CPU data structure
///
/// Contains data that is specific to each CPU in the system.
#[repr(C, align(64))]
pub struct PerCpu {
    /// CPU ID
    pub cpu_id: u32,

    /// CPU number (0-based)
    pub cpu_num: u32,

    /// Current thread running on this CPU
    pub current_thread: AtomicU64,

    /// CPU state
    pub state: AtomicU8,

    /// Reserved for future use
    _reserved: [u8; 59],
}

/// CPU states
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuState {
    /// CPU is not present
    Offline = 0,

    /// CPU is present but not initialized
    Uninitialized = 1,

    /// CPU is initialized but not running
    Ready = 2,

    /// CPU is running
    Running = 3,

    /// CPU is being halted
    Halted = 4,

    /// CPU has encountered an error
    Error = 5,
}

impl PerCpu {
    /// Create zero-initialized per-CPU data
    pub const fn zeroed() -> Self {
        Self {
            cpu_id: 0,
            cpu_num: 0,
            current_thread: AtomicU64::new(0),
            state: AtomicU8::new(CpuState::Offline as u8),
            _reserved: [0; 59],
        }
    }

    /// Initialize per-CPU data
    pub fn init(&mut self, cpu_id: u32, cpu_num: u32) {
        self.cpu_id = cpu_id;
        self.cpu_num = cpu_num;
        self.current_thread.store(0, Ordering::Release);
        self.state.store(CpuState::Uninitialized as u8, Ordering::Release);
    }

    /// Get the current thread ID
    pub fn current_thread(&self) -> ThreadId {
        self.current_thread.load(Ordering::Acquire)
    }

    /// Set the current thread ID
    pub fn set_current_thread(&self, tid: ThreadId) {
        self.current_thread.store(tid, Ordering::Release);
    }

    /// Get the CPU state
    pub fn state(&self) -> CpuState {
        match self.state.load(Ordering::Acquire) {
            0 => CpuState::Offline,
            1 => CpuState::Uninitialized,
            2 => CpuState::Ready,
            3 => CpuState::Running,
            4 => CpuState::Halted,
            5 => CpuState::Error,
            _ => CpuState::Error,
        }
    }

    /// Set the CPU state
    pub fn set_state(&self, state: CpuState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Check if this CPU is online
    pub fn is_online(&self) -> bool {
        matches!(
            self.state(),
            CpuState::Ready | CpuState::Running
        )
    }

    /// Check if this CPU is the boot CPU
    pub fn is_boot_cpu(&self) -> bool {
        self.cpu_id == BOOT_CPU_ID
    }
}

/// ============================================================================
/// Global Per-CPU Data Array
/// ============================================================================

/// Wrapper for aligned per-CPU data array
#[repr(align(64))]
struct AlignedPerCpuArray {
    data: [PerCpu; SMP_MAX_CPUS],
}

/// Global per-CPU data array
///
/// This is aligned to cache line boundaries to prevent false sharing.
static mut PERCPU_DATA: AlignedPerCpuArray = AlignedPerCpuArray {
    data: [PerCpu::zeroed(); SMP_MAX_CPUS],
};

/// Number of initialized CPUs
static mut NUM_CPUS: u32 = 0;

/// ============================================================================
/// Public API
/// ============================================================================

/// Initialize the per-CPU subsystem
///
/// Must be called during kernel initialization.
pub fn percpu_init() {
    unsafe {
        // Initialize boot CPU
        PERCPU_DATA.data[0].init(BOOT_CPU_ID, 0);
        PERCPU_DATA.data[0].set_state(CpuState::Running);
        NUM_CPUS = 1;
    }

    log_info!("Per-CPU data initialized");
    log_info!("  Max CPUs: {}", SMP_MAX_CPUS);
    log_info!("  Boot CPU: {}", BOOT_CPU_ID);
}

/// Initialize a per-CPU data area
///
/// # Arguments
///
/// * `cpu_id` - ACPI/MP CPU ID
/// * `cpu_num` - Sequential CPU number (0-based)
///
/// # Safety
///
/// Must be called during SMP initialization before the CPU is started.
pub unsafe fn percpu_init_cpu(cpu_id: u32, cpu_num: u32) {
    if cpu_num as usize >= SMP_MAX_CPUS {
        panic!("CPU number {} exceeds max CPUs {}", cpu_num, SMP_MAX_CPUS);
    }

    PERCPU_DATA.data[cpu_num as usize].init(cpu_id, cpu_num);
    PERCPU_DATA.data[cpu_num as usize].set_state(CpuState::Ready);

    NUM_CPUS += 1;

    log_debug!("Initialized CPU {} (ID {})", cpu_num, cpu_id);
}

/// Get the local per-CPU data
///
/// Returns a reference to the current CPU's data structure.
///
/// # Safety
///
/// The caller must be running on a valid CPU with initialized per-CPU data.
pub fn get_local_percpu() -> &'static PerCpu {
    // In a real implementation, this would use:
    // - CPUID instruction on x86_64
    // - TPIDR_EL1 on ARM64
    // - tp register on RISC-V

    // For now, return the boot CPU data
    unsafe {
        &PERCPU_DATA.data[0]
    }
}

/// Get per-CPU data by CPU number
///
/// # Arguments
///
/// * `cpu_num` - CPU number (0-based)
///
/// # Safety
///
/// The caller must ensure the CPU number is valid and the per-CPU data is initialized.
pub unsafe fn get_percpu(cpu_num: usize) -> &'static PerCpu {
    if cpu_num >= SMP_MAX_CPUS {
        panic!("Invalid CPU number: {}", cpu_num);
    }

    &PERCPU_DATA.data[cpu_num]
}

/// Get the number of CPUs in the system
pub fn num_cpus() -> u32 {
    unsafe { NUM_CPUS }
}

/// Get the current CPU number
///
/// Returns the 0-based CPU number of the calling CPU.
pub fn current_cpu_num() -> u32 {
    // In a real implementation, this would read from a CPU-specific register
    // For now, return 0 (boot CPU)
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let mut num: u32;
        core::arch::asm!(
            "mov {0}, fs:[0x8]", // Assume FS:0x8 stores CPU number
            out(reg) num,
            options(nostack, pure, readonly)
        );
        num
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        let mut num: u64;
        core::arch::asm!(
            "mrs {0}, tpidr_el1", // Assume TPIDR_EL1 stores pointer to per-CPU data
            out(reg) num,
            options(nostack, pure, readonly)
        );
        num as u32
    }

    #[cfg(target_arch = "riscv64")]
    {
        0 // Stub
    }
}

/// Set the current CPU number (for initialization)
///
/// # Arguments
///
/// * `cpu_num` - CPU number to set
///
/// # Safety
///
/// This should only be called during CPU initialization.
pub unsafe fn set_current_cpu_num(cpu_num: u32) {
    // In a real implementation, this would set the CPU-specific register
    // For now, this is a stub
    let _ = cpu_num;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percpu_zeroed() {
        let percpu = PerCpu::zeroed();
        assert_eq!(percpu.cpu_id, 0);
        assert_eq!(percpu.cpu_num, 0);
        assert_eq!(percpu.current_thread(), 0);
        assert_eq!(percpu.state(), CpuState::Offline);
    }

    #[test]
    fn test_percpu_init() {
        let mut percpu = PerCpu::zeroed();
        percpu.init(1, 0);

        assert_eq!(percpu.cpu_id, 1);
        assert_eq!(percpu.cpu_num, 0);
        assert_eq!(percpu.state(), CpuState::Uninitialized);
        assert!(!percpu.is_boot_cpu());
    }

    #[test]
    fn test_percpu_boot_cpu() {
        let mut percpu = PerCpu::zeroed();
        percpu.init(BOOT_CPU_ID, 0);

        assert!(percpu.is_boot_cpu());
    }

    #[test]
    fn test_percpu_thread() {
        let percpu = PerCpu::zeroed();

        assert_eq!(percpu.current_thread(), 0);
        percpu.set_current_thread(42);
        assert_eq!(percpu.current_thread(), 42);
    }

    #[test]
    fn test_cpu_state() {
        let percpu = PerCpu::zeroed();

        percpu.set_state(CpuState::Running);
        assert_eq!(percpu.state(), CpuState::Running);
        assert!(percpu.is_online());

        percpu.set_state(CpuState::Halted);
        assert!(!percpu.is_online());
    }
}
