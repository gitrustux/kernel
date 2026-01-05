// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::rustux::types::*;

// ARM Secure Monitor Call Calling Convention (SMCCC)
//
// http://infocenter.arm.com/help/index.jsp?topic=/com.arm.doc.den0028b/index.html

#[repr(C)]
pub struct arm_smccc_result {
    pub x0: u64,
    pub x1: u64,
    pub x2: u64,
    pub x3: u64,
}

pub type arm_smccc_result_t = arm_smccc_result;

/// Makes a secure monitor call (SMC)
///
/// # Parameters
/// * `w0` - Function Identifier
/// * `x1`, `x2` - Parameters
/// * `x3`, `x4` - Parameters
/// * `x5`, `x6` - Parameters
/// * `w7` - Client ID[15:0], Secure OS ID[31:16]
pub unsafe fn arm_smccc_smc(
    w0: u32,
    x1: u64, x2: u64,
    x3: u64, x4: u64,
    x5: u64, x6: u64,
    w7: u32,
) -> arm_smccc_result_t;

/// Makes a hypervisor call (HVC)
///
/// # Parameters
/// * `w0` - Function Identifier
/// * `x1`, `x2` - Parameters
/// * `x3`, `x4` - Parameters
/// * `x5`, `x6` - Parameters
/// * `w7` - Secure OS ID[31:16]
pub unsafe fn arm_smccc_hvc(
    w0: u32,
    x1: u64, x2: u64,
    x3: u64, x4: u64,
    x5: u64, x6: u64,
    w7: u32,
) -> arm_smccc_result_t;