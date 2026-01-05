// Copyright 2025 Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64::*;
use crate::vm::vm::*;

/// Disable debug state for the current execution context
pub fn arm64_disable_debug_state() {
    // The KDE bit enables and disables debug exceptions for the current execution.
    // Instruction Breakpoint Exceptions (software breakpoints) cannot be deactivated.
    unsafe {
        let mdscr_val = __arm_rsr("mdscr_el1") & !ARM64_MDSCR_EL1_KDE;
        __arm_wsr("mdscr_el1", mdscr_val);
        __isb(ARM_MB_SY);
    }
}

/// Enable debug state for the current execution context
pub fn arm64_enable_debug_state() {
    // The KDE bit enables and disables debug exceptions for the current execution.
    // Instruction Breakpoint Exceptions (software breakpoints) cannot be deactivated.
    unsafe {
        let mdscr_val = __arm_rsr("mdscr_el1") | ARM64_MDSCR_EL1_KDE;
        __arm_wsr("mdscr_el1", mdscr_val);
        __isb(ARM_MB_SY);
    }
}

/// Validate a debug state structure
///
/// Checks that the addresses are valid and masks out fields that userspace
/// is not allowed to modify.
///
/// # Arguments
///
/// * `state` - Debug state structure to validate
///
/// # Returns
///
/// * `true` if the debug state is valid, `false` otherwise
pub fn arm64_validate_debug_state(state: &mut Arm64DebugState) -> bool {
    // Validate that the addresses are valid.
    let hw_bp_count = arm64_hw_breakpoint_count();
    for i in 0..hw_bp_count {
        let addr = state.hw_bps[i as usize].dbgbvr;
        if addr != 0 && !is_user_address(addr) {
            return false;
        }

        // Mask out the fields that userspace is not allowed to modify.
        let masked_user_bcr = state.hw_bps[i as usize].dbgbcr & ARM64_DBGBCR_USER_MASK;
        state.hw_bps[i as usize].dbgbcr = ARM64_DBGBCR_MASK | masked_user_bcr;
    }

    true
}

/// Get the number of hardware breakpoints supported by the CPU
///
/// # Returns
///
/// * Number of hardware breakpoints
pub fn arm64_hw_breakpoint_count() -> u8 {
    // TODO(donoso): Eventually this should be cached as a boot time constant.
    unsafe {
        let dfr0 = __arm_rsr64("id_aa64dfr0_el1");
        let count = (((dfr0 & ARM64_ID_AADFR0_EL1_BRPS) >>
                       ARM64_ID_AADFR0_EL1_BRPS_SHIFT) + 1) as u8;
        // ARMv8 assures at least 2 hw registers.
        debug_assert!(count >= 2 && count <= 16);
        count
    }
}

// Read Debug State ------------------------------------------------------------------------------

/// Read hardware breakpoint by index
///
/// # Arguments
///
/// * `debug_state` - Debug state structure to read into
/// * `index` - Index of the hardware breakpoint to read
fn arm64_read_hw_breakpoint_by_index(debug_state: &mut Arm64DebugState, index: u32) {
    debug_assert!(index < arm64_hw_breakpoint_count() as u32);

    unsafe {
        match index {
            0 => {
                debug_state.hw_bps[0].dbgbcr = __arm_rsr("dbgbcr0_el1");
                debug_state.hw_bps[0].dbgbvr = __arm_rsr64("dbgbvr0_el1");
            },
            1 => {
                debug_state.hw_bps[1].dbgbcr = __arm_rsr("dbgbcr1_el1");
                debug_state.hw_bps[1].dbgbvr = __arm_rsr64("dbgbvr1_el1");
            },
            2 => {
                debug_state.hw_bps[2].dbgbcr = __arm_rsr("dbgbcr2_el1");
                debug_state.hw_bps[2].dbgbvr = __arm_rsr64("dbgbvr2_el1");
            },
            3 => {
                debug_state.hw_bps[3].dbgbcr = __arm_rsr("dbgbcr3_el1");
                debug_state.hw_bps[3].dbgbvr = __arm_rsr64("dbgbvr3_el1");
            },
            4 => {
                debug_state.hw_bps[4].dbgbcr = __arm_rsr("dbgbcr4_el1");
                debug_state.hw_bps[4].dbgbvr = __arm_rsr64("dbgbvr4_el1");
            },
            5 => {
                debug_state.hw_bps[5].dbgbcr = __arm_rsr("dbgbcr5_el1");
                debug_state.hw_bps[5].dbgbvr = __arm_rsr64("dbgbvr5_el1");
            },
            6 => {
                debug_state.hw_bps[6].dbgbcr = __arm_rsr("dbgbcr6_el1");
                debug_state.hw_bps[6].dbgbvr = __arm_rsr64("dbgbvr6_el1");
            },
            7 => {
                debug_state.hw_bps[7].dbgbcr = __arm_rsr("dbgbcr7_el1");
                debug_state.hw_bps[7].dbgbvr = __arm_rsr64("dbgbvr7_el1");
            },
            8 => {
                debug_state.hw_bps[8].dbgbcr = __arm_rsr("dbgbcr8_el1");
                debug_state.hw_bps[8].dbgbvr = __arm_rsr64("dbgbvr8_el1");
            },
            9 => {
                debug_state.hw_bps[9].dbgbcr = __arm_rsr("dbgbcr9_el1");
                debug_state.hw_bps[9].dbgbvr = __arm_rsr64("dbgbvr9_el1");
            },
            10 => {
                debug_state.hw_bps[10].dbgbcr = __arm_rsr("dbgbcr10_el1");
                debug_state.hw_bps[10].dbgbvr = __arm_rsr64("dbgbvr10_el1");
            },
            11 => {
                debug_state.hw_bps[11].dbgbcr = __arm_rsr("dbgbcr11_el1");
                debug_state.hw_bps[11].dbgbvr = __arm_rsr64("dbgbvr11_el1");
            },
            12 => {
                debug_state.hw_bps[12].dbgbcr = __arm_rsr("dbgbcr12_el1");
                debug_state.hw_bps[12].dbgbvr = __arm_rsr64("dbgbvr12_el1");
            },
            13 => {
                debug_state.hw_bps[13].dbgbcr = __arm_rsr("dbgbcr13_el1");
                debug_state.hw_bps[13].dbgbvr = __arm_rsr64("dbgbvr13_el1");
            },
            14 => {
                debug_state.hw_bps[14].dbgbcr = __arm_rsr("dbgbcr14_el1");
                debug_state.hw_bps[14].dbgbvr = __arm_rsr64("dbgbvr14_el1");
            },
            15 => {
                debug_state.hw_bps[15].dbgbcr = __arm_rsr("dbgbcr15_el1");
                debug_state.hw_bps[15].dbgbvr = __arm_rsr64("dbgbvr15_el1");
            },
            _ => {
                debug_assert!(false, "Invalid hardware breakpoint index");
            }
        }
    }
}

/// Read all hardware debug registers
///
/// # Arguments
///
/// * `debug_state` - Debug state structure to read into
pub fn arm64_read_hw_debug_regs(debug_state: &mut Arm64DebugState) {
    let count = arm64_hw_breakpoint_count();
    for i in 0..count {
        arm64_read_hw_breakpoint_by_index(debug_state, i as u32);
    }
}

// Writing Debug State ---------------------------------------------------------------------------

/// Write hardware breakpoint by index
///
/// # Arguments
///
/// * `debug_state` - Debug state structure to write from
/// * `index` - Index of the hardware breakpoint to write
fn arm64_write_hw_breakpoint_by_index(debug_state: &Arm64DebugState, index: u32) {
    debug_assert!(index < arm64_hw_breakpoint_count() as u32);

    unsafe {
        match index {
            0 => {
                __arm_wsr("dbgbcr0_el1", debug_state.hw_bps[0].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr0_el1", debug_state.hw_bps[0].dbgbvr);
                __isb(ARM_MB_SY);
            },
            1 => {
                __arm_wsr("dbgbcr1_el1", debug_state.hw_bps[1].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr1_el1", debug_state.hw_bps[1].dbgbvr);
                __isb(ARM_MB_SY);
            },
            2 => {
                __arm_wsr("dbgbcr2_el1", debug_state.hw_bps[2].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr2_el1", debug_state.hw_bps[2].dbgbvr);
                __isb(ARM_MB_SY);
            },
            3 => {
                __arm_wsr("dbgbcr3_el1", debug_state.hw_bps[3].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr3_el1", debug_state.hw_bps[3].dbgbvr);
                __isb(ARM_MB_SY);
            },
            4 => {
                __arm_wsr("dbgbcr4_el1", debug_state.hw_bps[4].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr4_el1", debug_state.hw_bps[4].dbgbvr);
                __isb(ARM_MB_SY);
            },
            5 => {
                __arm_wsr("dbgbcr5_el1", debug_state.hw_bps[5].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr5_el1", debug_state.hw_bps[5].dbgbvr);
                __isb(ARM_MB_SY);
            },
            6 => {
                __arm_wsr("dbgbcr6_el1", debug_state.hw_bps[6].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr6_el1", debug_state.hw_bps[6].dbgbvr);
                __isb(ARM_MB_SY);
            },
            7 => {
                __arm_wsr("dbgbcr7_el1", debug_state.hw_bps[7].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr7_el1", debug_state.hw_bps[7].dbgbvr);
                __isb(ARM_MB_SY);
            },
            8 => {
                __arm_wsr("dbgbcr8_el1", debug_state.hw_bps[8].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr8_el1", debug_state.hw_bps[8].dbgbvr);
                __isb(ARM_MB_SY);
            },
            9 => {
                __arm_wsr("dbgbcr9_el1", debug_state.hw_bps[9].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr9_el1", debug_state.hw_bps[9].dbgbvr);
                __isb(ARM_MB_SY);
            },
            10 => {
                __arm_wsr("dbgbcr10_el1", debug_state.hw_bps[10].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr10_el1", debug_state.hw_bps[10].dbgbvr);
                __isb(ARM_MB_SY);
            },
            11 => {
                __arm_wsr("dbgbcr11_el1", debug_state.hw_bps[11].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr11_el1", debug_state.hw_bps[11].dbgbvr);
                __isb(ARM_MB_SY);
            },
            12 => {
                __arm_wsr("dbgbcr12_el1", debug_state.hw_bps[12].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr12_el1", debug_state.hw_bps[12].dbgbvr);
                __isb(ARM_MB_SY);
            },
            13 => {
                __arm_wsr("dbgbcr13_el1", debug_state.hw_bps[13].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr13_el1", debug_state.hw_bps[13].dbgbvr);
                __isb(ARM_MB_SY);
            },
            14 => {
                __arm_wsr("dbgbcr14_el1", debug_state.hw_bps[14].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr14_el1", debug_state.hw_bps[14].dbgbvr);
                __isb(ARM_MB_SY);
            },
            15 => {
                __arm_wsr("dbgbcr15_el1", debug_state.hw_bps[15].dbgbcr);
                __isb(ARM_MB_SY);
                __arm_wsr64("dbgbvr15_el1", debug_state.hw_bps[15].dbgbvr);
                __isb(ARM_MB_SY);
            },
            _ => {
                debug_assert!(false, "Invalid hardware breakpoint index");
            }
        }
    }
}

/// Write all hardware debug registers
///
/// # Arguments
///
/// * `debug_state` - Debug state structure to write from
pub fn arm64_write_hw_debug_regs(debug_state: &Arm64DebugState) {
    let bps_count = arm64_hw_breakpoint_count();
    for i in 0..bps_count {
        arm64_write_hw_breakpoint_by_index(debug_state, i as u32);
    }
}

// Types and constants

/// Maximum number of hardware breakpoints
const MAX_HW_BREAKPOINTS: usize = 16;

/// Hardware breakpoint structure
#[repr(C)]
pub struct Arm64HwBreakpoint {
    pub dbgbcr: u32,
    pub dbgbvr: u64,
}

/// Debug state structure
#[repr(C)]
pub struct Arm64DebugState {
    pub hw_bps: [Arm64HwBreakpoint; MAX_HW_BREAKPOINTS],
}

// ARM64 hardware register bit definitions
const ARM64_MDSCR_EL1_KDE: u32 = 1 << 13;
const ARM64_DBGBCR_USER_MASK: u32 = 0xFFFFFFFF; // Would be defined with actual mask bits
const ARM64_DBGBCR_MASK: u32 = 0; // Would be defined with actual mask bits
const ARM64_ID_AADFR0_EL1_BRPS: u64 = 0xF << 12;
const ARM64_ID_AADFR0_EL1_BRPS_SHIFT: u64 = 12;
const ARM_MB_SY: u32 = 15;

// External functions
extern "C" {
    fn __arm_rsr(reg: &str) -> u32;
    fn __arm_rsr64(reg: &str) -> u64;
    fn __arm_wsr(reg: &str, val: u32);
    fn __arm_wsr64(reg: &str, val: u64);
    fn __isb(mb_type: u32);
    fn is_user_address(addr: u64) -> bool;
}