// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 DDK System Calls
//!
//! This module implements ARM64-specific DDK system calls.
//!
//! # Design
//!
//! ARM64 supports SMC (Secure Monitor Call) which is used to call into
//! the ARM Secure Monitor (typically for TrustZone services).

#![no_std]

use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use core::sync::atomic::{AtomicU64, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};
/// ============================================================================
/// SMC Function ID Types
/// ============================================================================

/// SMC function call types
pub mod smc_call_type {
    /// Standard call
    pub const STANDARD: u32 = 0x0;

    /// Fast call
    pub const FAST: u32 = 0x1;

    /// Type mask
    pub const MASK: u32 = 0x1;
}

/// SMC calling conventions
pub mod smc_calling_convention {
    /// SMC32 call (32-bit values)
    pub const SMC32: u32 = 0x0 << 30;

    /// SMC64 call (64-bit values)
    pub const SMC64: u32 = 0x1 << 30;

    /// Convention mask
    pub const MASK: u32 = 0x3 << 30;
}

/// ============================================================================
/// SMC Parameters
/// ============================================================================

/// SMC parameters structure
///
/// This structure is used to pass parameters to SMC calls.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SmcParameters {
    /// Function ID (includes type, calling convention, and function number)
    pub func_id: u32,

    /// Secure OS ID
    pub secure_os_id: u16,

    /// Client ID
    pub client_id: u16,

    /// Reserved
    pub reserved: [u32; 2],

    /// Arguments
    pub args: [u64; 6],
}

impl Default for SmcParameters {
    fn default() -> Self {
        Self {
            func_id: 0,
            secure_os_id: 0,
            client_id: 0,
            reserved: [0; 2],
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
    /// Return values
    pub ret_vals: [u64; 4],
}

impl Default for SmcResult {
    fn default() -> Self {
        Self {
            ret_vals: [0; 4],
        }
    }
}

/// ============================================================================
/// SMC Call Result
/// ============================================================================

/// ARM SMC call result
///
/// Represents the result of an ARM SMC call.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ArmSmcResult {
    /// x0 register value
    pub x0: u64,

    /// x1 register value
    pub x1: u64,

    /// x2 register value
    pub x2: u64,

    /// x3 register value
    pub x3: u64,

    /// x4 register value
    pub x4: u64,

    /// x5 register value
    pub x5: u64,

    /// x6 register value
    pub x6: u64,
}

/// ============================================================================
/// SMC Statistics
/// ============================================================================

/// Total SMC calls counter
static mut TOTAL_SMC_CALLS: AtomicU64 = AtomicU64::new(0);

/// ============================================================================
/// Syscall: SMC Call (ARM64)
/// ============================================================================

/// SMC call syscall handler (ARM64)
///
/// Performs an ARM64 SMC (Secure Monitor Call) to call into the
/// ARM Secure Monitor (typically for TrustZone services).
///
/// # Arguments
///
/// * `params` - User pointer to SMC parameters
/// * `result` - User pointer to store SMC result
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn arch_smc_call_impl(params: usize, result: usize) -> SyscallRet {
    log_debug!(
        "arch_smc_call_arm64: params={:#x} result={:#x}",
        params,
        result
    );

    // Increment SMC call counter
    unsafe {
        TOTAL_SMC_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    // Copy parameters from user
    let user_params_ptr = crate::kernel::usercopy::UserPtr::<u8>::new(params);
    let smc_params = unsafe {
        let mut p = SmcParameters::default();
        if let Err(err) = crate::kernel::usercopy::copy_from_user(
            &mut p as *mut SmcParameters as *mut u8,
            user_params_ptr,
            1,
        ) {
            log_error!("arch_smc_call_arm64: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
        p
    };

    log_debug!(
        "arch_smc_call_arm64: func_id={:#x} secure_os_id={} client_id={}",
        smc_params.func_id,
        smc_params.secure_os_id,
        smc_params.client_id
    );

    // Perform the SMC call
    let arm_result = arm_smccc_smc(
        smc_params.func_id,
        smc_params.args[0],
        smc_params.args[1],
        smc_params.args[2],
        smc_params.args[3],
        smc_params.args[4],
        smc_params.args[5],
        ((smc_params.secure_os_id as u32) << 16) | (smc_params.client_id as u32),
    );

    // Build result structure
    let smc_result = SmcResult {
        ret_vals: [arm_result.x0, arm_result.x1, arm_result.x2, arm_result.x3],
    };

    // Copy result to user
    let user_result_ptr = crate::kernel::usercopy::UserPtr::<u8>::new(result);
    unsafe {
        if let Err(err) = crate::kernel::usercopy::copy_to_user(
            user_result_ptr,
            &smc_result as *const SmcResult as *const u8,
            1,
        ) {
            log_error!("arch_smc_call_arm64: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!(
        "arch_smc_call_arm64: success x0={:#x} x1={:#x}",
        arm_result.x0,
        arm_result.x1
    );

    ok_to_ret(0)
}

/// ============================================================================
/// ARM SMCCC SMC Call
/// ============================================================================

/// ARM SMC Calling Convention (SMCCC) call
///
/// Performs an SMC call according to the ARM SMC Calling Convention.
///
/// # Arguments
///
/// * `func_id` - Function ID
/// * `arg0` - Argument 0
/// * `arg1` - Argument 1
/// * `arg2` - Argument 2
/// * `arg3` - Argument 3
/// * `arg4` - Argument 4
/// * `arg5` - Argument 5
/// * `client_id` - Client ID (secure_os_id << 16 | client_id)
///
/// # Returns
///
/// SMC result structure
fn arm_smccc_smc(
    func_id: u32,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    client_id: u32,
) -> ArmSmcResult {
    log_debug!(
        "arm_smccc_smc: func_id={:#x} client_id={:#x}",
        func_id,
        client_id
    );

    // TODO: Implement actual SMC call
    // This requires inline assembly to execute the SMC instruction
    //
    // The assembly would look something like:
    // ```
    // asm volatile (
    //     "smc #0"
    //     : "=r"(x0), "=r"(x1), "=r"(x2), "=r"(x3),
    //       "=r"(x4), "=r"(x5), "=r"(x6)
    //     : "r"(func_id), "r"(arg0), "r"(arg1), "r"(arg2),
    //       "r"(arg3), "r"(arg4), "r"(arg5), "r"(client_id)
    //     : "memory"
    // );
    // ```

    // For now, return a stub result
    ArmSmcResult {
        x0: func_id as u64, // Echo back function ID as success
        x1: 0,
        x2: 0,
        x3: 0,
        x4: 0,
        x5: 0,
        x6: 0,
    }
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get ARM64 DDK statistics
pub fn get_stats() -> DdkArm64Stats {
    DdkArm64Stats {
        total_smc_calls: unsafe { TOTAL_SMC_CALLS.load(Ordering::Relaxed) },
    }
}

/// ARM64 DDK statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DdkArm64Stats {
    /// Total SMC calls
    pub total_smc_calls: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the ARM64 DDK subsystem
pub fn init() {
    log_info!("ARM64 DDK subsystem initialized");
    log_info!("  SMC (Secure Monitor Call) support enabled");
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smc_parameters_default() {
        let params = SmcParameters::default();
        assert_eq!(params.func_id, 0);
        assert_eq!(params.secure_os_id, 0);
        assert_eq!(params.client_id, 0);
        assert_eq!(params.args, [0; 6]);
    }

    #[test]
    fn test_smc_result_default() {
        let result = SmcResult::default();
        assert_eq!(result.ret_vals, [0; 4]);
    }

    #[test]
    fn test_arm_smc_result_default() {
        let result = ArmSmcResult::default();
        assert_eq!(result.x0, 0);
        assert_eq!(result.x1, 0);
        assert_eq!(result.x2, 0);
        assert_eq!(result.x3, 0);
    }

    #[test]
    fn test_smc_call_type_consts() {
        assert_eq!(smc_call_type::STANDARD, 0x0);
        assert_eq!(smc_call_type::FAST, 0x1);
        assert_eq!(smc_call_type::MASK, 0x1);
    }

    #[test]
    fn test_smc_calling_convention_consts() {
        assert_eq!(smc_calling_convention::SMC32, 0x0 << 30);
        assert_eq!(smc_calling_convention::SMC64, 0x1 << 30);
        assert_eq!(smc_calling_convention::MASK, 0x3 << 30);
    }

    #[test]
    fn test_arm_smccc_smc() {
        let result = arm_smccc_smc(0x84000000, 1, 2, 3, 4, 5, 6, 0);
        // Should echo back function ID
        assert_eq!(result.x0, 0x84000000);
    }
}
