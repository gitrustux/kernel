// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

// MDSCR_EL1
// Monitor Debug System Control Register. It's the main control register for the debug
// implementation.

pub const ARM64_MDSCR_EL1_SS: u32 = 1 << 0;
pub const ARM64_MDSCR_EL1_ERR: u32 = 1 << 6;
pub const ARM64_MDSCR_EL1_TDCC: u32 = 1 << 12;
pub const ARM64_MDSCR_EL1_KDE: u32 = 1 << 13;
pub const ARM64_MDSCR_EL1_HDE: u32 = 1 << 14;
pub const ARM64_MDSCR_EL1_MDE: u32 = 1 << 15;
pub const ARM64_MDSCR_EL1_RAZ_WI: u32 = 0x000e0000;
pub const ARM64_MDSCR_EL1_RAZ_WI_SHIFT: u32 = 16;
pub const ARM64_MDSCR_EL1_TDA: u32 = 1 << 21;
pub const ARM64_MDSCR_EL1_INTDIS: u32 = 0x000c0000;
pub const ARM64_MDSCR_EL1_INTDIS_SHIFT: u32 = 22;
pub const ARM64_MDSCR_EL1_TXU: u32 = 1 << 26;
pub const ARM64_MDSCR_EL1_RXO: u32 = 1 << 27;
pub const ARM64_MDSCR_EL1_TXfull: u32 = 1 << 29;
pub const ARM64_MDSCR_EL1_RXfull: u32 = 1 << 30;

// ID_AA64DFR0
// Debug Feature Register 0. This register is used to query the system for the debug
// capabilities present within the chip.

pub const ARM64_ID_AADFR0_EL1_DEBUG_VER: u64 = 0x000000000000000F;
pub const ARM64_ID_AADFR0_EL1_TRACE_VER: u64 = 0x00000000000000F0;
pub const ARM64_ID_AADFR0_EL1_PMU_VER: u64 = 0x0000000000000F00;
// Defines the amount of HW breakpoints.
pub const ARM64_ID_AADFR0_EL1_BRPS: u64 = 0x000000000000F000;
pub const ARM64_ID_AADFR0_EL1_BRPS_SHIFT: u64 = 12;
// Defines the amount of HW data watchpoints.
pub const ARM64_ID_AADFR0_EL1_WRPS: u64 = 0x0000000000F00000;
pub const ARM64_ID_AADFR0_EL1_WRPS_SHIFT: u64 = 20;
pub const ARM64_ID_AADFR0_EL1_CTX_CMP: u64 = 0x000000F0000000;
pub const ARM64_ID_AADFR0_EL1_PMS_VER: u64 = 0x00000F00000000;

// DBGBCR<n>
// Control register for HW breakpoints. There is one foreach HW breakpoint present within the
// system. They go numbering by DBGBCR0, DBGBCR1, ... until the value defined in ID_AADFR0_EL1.

pub const ARM64_DBGBCR_E: u32 = 1 << 0;
pub const ARM64_DBGBCR_PMC: u32 = 0b11 << 1;  // Bits 1-2.
pub const ARM64_DBGBCR_PMC_SHIFT: u32 = 1;
pub const ARM64_DBGBCR_BAS: u32 = 0b1111 << 5;  // Bits 5-8.
pub const ARM64_DBGGCR_BAS_SHIFT: u32 = 5;
pub const ARM64_DBGBCR_HMC: u32 = 1 << 13;
pub const ARM64_DBGBCR_HMC_SHIFT: u32 = 13;
pub const ARM64_DBGBCR_SSC: u32 = 0b111 << 14; // Bits 14-15.
pub const ARM64_DBGBCR_SSC_SHIFT: u32 = 14;
pub const ARM64_DBGBCR_LBN: u32 = 0b1111 << 16; // Bits 16-19.
pub const ARM64_DBGBCR_LBN_SHIFT: u32 = 16;
pub const ARM64_DBGBCR_BT: u32 = 0b1111 << 20; // Bits 20-23.
pub const ARM64_DBGBCR_BY_SHIFT: u32 = 20;

// The user can only activate/deactivate breakpoints.
pub const ARM64_DBGBCR_USER_MASK: u32 = ARM64_DBGBCR_E;

// This is the mask that we validate for a breakpoint control.
// PMC [0b10]
// BAS [0b1111]: Match on complete address.
// HMC [0]
// SSC [0]
// LBN [0]: No breakpoint linking.
// BT [0]: Unliked instruction address match.
//
// The PMC, HMC, SSC values configured here enable debug exceptions to be thrown in EL0.
pub const ARM64_DBGBCR_MASK: u32 = (0b10 << ARM64_DBGBCR_PMC_SHIFT) | ARM64_DBGBCR_BAS;

pub const ARM64_MAX_HW_BREAKPOINTS: usize = 16;

use crate::rustux::compiler::*;
use crate::sys::types::*;

/// Kernel tracking of the current state of the debug registers for a particular thread.
/// ARMv8 can have from 2 to 16 HW breakpoints and 2 to 16 HW watchpoints.
///
/// This struct can potentially hold all of them. If the platform has fewer of those
/// breakpoints available, it will fill from the lower index up to correct amount.
/// The other indices should never be accessed.
#[repr(C)]
pub struct arm64_debug_state {
    pub hw_bps: [HwBreakpoint; ARM64_MAX_HW_BREAKPOINTS],
    // TODO(donosoc): Do watchpoint integration.
}

#[repr(C)]
pub struct HwBreakpoint {
    pub dbgbcr: u32,
    pub dbgbvr: u64,
}

pub type arm64_debug_state_t = arm64_debug_state;

/// Enable/disable the HW debug functionalities for the current thread.
pub unsafe fn arm64_disable_debug_state();
pub unsafe fn arm64_enable_debug_state();

/// Checks whether the given state is valid to install on a running thread.
/// Will mask out reserved values on DBGBCR<n>. This is for the caller convenience, considering
/// that we don't have a good mechanism to communicate back to the user what went wrong with the
/// call.
pub unsafe fn arm64_validate_debug_state(debug_state: *mut arm64_debug_state_t) -> bool;

/// Returns the amount of HW breakpoints present in this CPU.
pub unsafe fn arm64_hw_breakpoint_count() -> u8;

/// Read from the CPU registers into |debug_state|.
pub unsafe fn arm64_read_hw_debug_regs(debug_state: *mut arm64_debug_state_t);

/// Write from the |debug_state| into the CPU registers.
///
/// IMPORTANT: This function is used in the context switch, so no validation is done, just writing.
///            In any other context (eg. setting debug values from a syscall), you *MUST* call
///            arm64_validate_debug_state first.
pub unsafe fn arm64_write_hw_debug_regs(debug_state: *const arm64_debug_state_t);

/// Handles the context switch for debug HW functionality.
/// Will only copy over state if it's enabled (non-zero) for |new_thread|.
pub unsafe fn arm64_debug_state_context_switch(old_thread: *mut thread, new_thread: *mut thread);