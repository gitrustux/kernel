// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Syscall Wrapper Module
//!
//! This module provides wrapper functions for system call handling.
//! It manages syscall validation, statistics, and tracing.
//!
//! # Design
//!
//! - Validates syscall numbers
//! - Tracks syscall statistics
//! - Provides error handling for invalid syscalls
//! - Manages interrupt state during syscalls


use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_error, log_info};

/// ============================================================================
/// Syscall Statistics
/// ============================================================================

/// Per-syscall statistics
#[repr(C)]
#[derive(Debug)]
pub struct SyscallStatistics {
    /// Total number of syscalls
    pub total: AtomicU64,

    /// Number of syscalls by type
    pub by_type: [AtomicU64; 256],
}

impl SyscallStatistics {
    /// Create new syscall statistics
    pub const fn new() -> Self {
        const INIT: AtomicU64 = AtomicU64::new(0);
        Self {
            total: AtomicU64::new(0),
            by_type: [INIT; 256],
        }
    }

    /// Record a syscall
    pub fn record(&self, num: u32) {
        self.total.fetch_add(1, Ordering::Relaxed);
        if (num as usize) < 256 {
            self.by_type[num as usize].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get total syscall count
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Get syscall count for a specific type
    pub fn count(&self, num: u32) -> u64 {
        if (num as usize) < 256 {
            self.by_type[num as usize].load(Ordering::Relaxed)
        } else {
            0
        }
    }
}

/// ============================================================================
/// Global Syscall Statistics
/// ============================================================================

/// Global syscall statistics
static SYSCALL_STATS: SyscallStatistics = SyscallStatistics::new();

/// Get global syscall statistics
pub fn get_stats() -> &'static SyscallStatistics {
    &SYSCALL_STATS
}

/// Record a syscall invocation
pub fn record_syscall(num: u32) {
    SYSCALL_STATS.record(num);
}

/// ============================================================================
/// Syscall Validation
/// ============================================================================

/// Validate a syscall number
///
/// # Arguments
///
/// * `num` - Syscall number
///
/// # Returns
///
/// * true if syscall number is valid
/// * false otherwise
pub fn is_valid_syscall_number(num: u32) -> bool {
    // Check if syscall number is in valid range
    // The maximum syscall number is 0x43 for ABI v1
    num <= 0x43
}

/// Validate program counter for syscall
///
/// # Arguments
///
/// * `pc` - Program counter value
/// * `vdso_base` - VDSO base address
///
/// # Returns
///
/// * true if PC is valid
/// * false otherwise
pub fn is_valid_pc(pc: u64, vdso_base: u64) -> bool {
    // TODO: Implement proper PC validation
    // For now, just check if PC is not zero
    pc != 0
}

/// ============================================================================
/// Invalid Syscall Handler
/// ============================================================================

/// Handle invalid syscall
///
/// # Arguments
///
/// * `num` - Invalid syscall number
/// * `pc` - Program counter
///
/// # Returns
///
/// Negative error code
pub fn handle_invalid_syscall(num: u32, pc: u64) -> SyscallRet {
    log_error!(
        "Invalid syscall: num={} pc={:#x}",
        num,
        pc
    );

    // TODO: Signal policy exception
    // For now, just return error

    err_to_ret(RX_ERR_BAD_SYSCALL)
}

/// ============================================================================
/// Syscall Result
/// ============================================================================

/// Syscall result with signal flag
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallResult {
    /// Return value
    pub value: u64,

    /// Whether a signal is pending
    pub signal_pending: bool,
}

impl SyscallResult {
    /// Create a new syscall result
    pub const fn new(value: u64, signal_pending: bool) -> Self {
        Self {
            value,
            signal_pending,
        }
    }

    /// Create a success result
    pub const fn ok(value: u64) -> Self {
        Self::new(value, false)
    }

    /// Create an error result
    pub const fn err(err: rx_status_t) -> Self {
        Self::new(err as u64, false)
    }

    /// Create a result with pending signal
    pub const fn with_signal(value: u64) -> Self {
        Self::new(value, true)
    }
}

/// ============================================================================
/// Syscall Wrapper
/// ============================================================================

/// Wrap syscall execution with validation and statistics
///
/// # Arguments
///
/// * `num` - Syscall number
/// * `pc` - Program counter
/// * `vdso_base` - VDSO base address
/// * `f` - Function to call if validation passes
///
/// # Returns
///
/// Syscall result
pub fn wrap_syscall<F>(num: u32, pc: u64, vdso_base: u64, f: F) -> SyscallResult
where
    F: FnOnce() -> SyscallRet,
{
    // Record syscall entry
    record_syscall(num);

    // Validate PC
    if !is_valid_pc(pc, vdso_base) {
        let ret = handle_invalid_syscall(num, pc);
        return SyscallResult::new(ret as u64, false);
    }

    // Call the actual syscall
    let ret = f();

    // Check for signal pending
    // TODO: Implement proper signal checking
    let signal_pending = false;

    SyscallResult::new(ret as u64, signal_pending)
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the syscall wrapper subsystem
pub fn init() {
    log_info!("Syscall wrapper subsystem initialized");
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_validation() {
        // Valid syscall numbers
        assert!(is_valid_syscall_number(0x01));
        assert!(is_valid_syscall_number(0x43));

        // Invalid syscall numbers
        assert!(!is_valid_syscall_number(0x44));
        assert!(!is_valid_syscall_number(0xFFFF));
    }

    #[test]
    fn test_statistics() {
        let stats = get_stats();

        // Record some syscalls
        stats.record(0x01);
        stats.record(0x01);
        stats.record(0x02);

        assert_eq!(stats.total(), 3);
        assert_eq!(stats.count(0x01), 2);
        assert_eq!(stats.count(0x02), 1);
        assert_eq!(stats.count(0x03), 0);
    }

    #[test]
    fn test_syscall_result() {
        let result = SyscallResult::ok(42);
        assert_eq!(result.value, 42);
        assert!(!result.signal_pending);

        let result = SyscallResult::err(RX_ERR_NO_MEMORY);
        assert_eq!(result.value, RX_ERR_NO_MEMORY as u64);
        assert!(!result.signal_pending);

        let result = SyscallResult::with_signal(0);
        assert_eq!(result.value, 0);
        assert!(result.signal_pending);
    }

    #[test]
    fn test_wrap_syscall() {
        let result = wrap_syscall(0x01, 0x1000, 0, || {
            ok_to_ret(42)
        });

        assert_eq!(result.value, 42);
        assert!(!result.signal_pending);

        // Check that syscall was recorded
        assert_eq!(get_stats().count(0x01), 1);
    }

    #[test]
    fn test_wrap_invalid_syscall() {
        // Invalid PC
        let result = wrap_syscall(0x01, 0, 0, || {
            ok_to_ret(42)
        });

        // Should return error for invalid PC
        assert_eq!(result.value as i64, -(RX_ERR_BAD_SYSCALL as i64));
    }
}
