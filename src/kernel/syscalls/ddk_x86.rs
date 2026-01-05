// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 DDK System Calls
//!
//! This module implements x86-specific DDK system calls.
//!
//! # Design
//!
//! x86 doesn't support ARM SMC (Secure Monitor Call), so these syscalls
//! return NOT_SUPPORTED.

#![no_std]

use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;

// Import logging macros
use crate::{log_debug, log_info, log_warn};

/// ============================================================================
/// SMC Parameters
/// ============================================================================

/// SMC parameters structure
///
/// This structure is used to pass parameters to SMC calls.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SmcParameters {
    /// Function ID
    pub func_id: u32,

    /// Reserved
    pub reserved: u32,

    /// Arguments
    pub args: [u64; 6],
}

impl Default for SmcParameters {
    fn default() -> Self {
        Self {
            func_id: 0,
            reserved: 0,
            args: [0; 6],
        }
    }
}

/// ============================================================================
/// SMC Result
/// ============================================================================

/// SMC result structure
///
/// This structure is used to return results from SMC calls.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SmcResult {
    /// Function ID
    pub func_id: u32,

    /// Reserved
    pub reserved: u32,

    /// Return values
    pub ret_vals: [u64; 6],
}

impl Default for SmcResult {
    fn default() -> Self {
        Self {
            func_id: 0,
            reserved: 0,
            ret_vals: [0; 6],
        }
    }
}

/// ============================================================================
/// Syscall: SMC Call (x86)
/// ============================================================================

/// SMC call syscall handler (x86)
///
/// x86 doesn't support ARM SMC (Secure Monitor Call), so this always
/// returns NOT_SUPPORTED.
///
/// # Arguments
///
/// * `params` - User pointer to SMC parameters
/// * `result` - User pointer to store SMC result
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code (always RX_ERR_NOT_SUPPORTED on x86)
pub fn arch_smc_call_impl(params: usize, result: usize) -> SyscallRet {
    log_debug!(
        "arch_smc_call_x86: params={:#x} result={:#x}",
        params,
        result
    );

    log_warn!("arch_smc_call_x86: SMC not supported on x86");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get x86 DDK statistics
pub fn get_stats() -> DdkX86Stats {
    DdkX86Stats {
        total_smc_calls: 0, // SMC not supported on x86
    }
}

/// x86 DDK statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DdkX86Stats {
    /// Total SMC calls (always 0, not supported)
    pub total_smc_calls: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the x86 DDK subsystem
pub fn init() {
    log_info!("x86 DDK subsystem initialized");
    log_warn!("  SMC calls not supported on x86 architecture");
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_smc_call_not_supported() {
        let result = arch_smc_call_impl(0, 0);
        assert_eq!(result, err_to_ret(RX_ERR_NOT_SUPPORTED));
    }

    #[test]
    fn test_smc_parameters_default() {
        let params = SmcParameters::default();
        assert_eq!(params.func_id, 0);
        assert_eq!(params.reserved, 0);
        assert_eq!(params.args, [0; 6]);
    }

    #[test]
    fn test_smc_result_default() {
        let result = SmcResult::default();
        assert_eq!(result.func_id, 0);
        assert_eq!(result.reserved, 0);
        assert_eq!(result.ret_vals, [0; 6]);
    }
}
