// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64;
use crate::arch::arm64::registers;
// use crate::arch::debugger;  // Removed - this is the current module
use crate::err;
use crate::kernel::thread::{self, Thread};
use crate::kernel::thread_lock::{self, ThreadLock, Guard, IrqSave};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use crate::rustux::syscalls::debug::*;
use crate::sys::{
    rx_thread_state_general_regs_t,
    rx_thread_state_vector_regs_t,
    rx_thread_state_fp_regs_t,
    rx_thread_state_debug_regs_t,
    rx_excp_type_t,
    rx_vaddr_t,
};

// Only the NZCV flags (bits 31 to 28 respectively) of the CPSR are
// readable and writable by userland on ARM64.
const USER_VISIBLE_FLAGS: u32 = 0xf0000000;

// SS (="Single Step") is bit 0 in MDSCR_EL1.
const MDSCR_SS_MASK: u64 = 1;

// Single Step for PSTATE, see ARMv8 Manual C5.2.18, enable Single step for Process
const SS_MASK_SPSR: u64 = 1 << 21;

pub fn arch_get_general_regs(thread: &Thread, out: &mut rx_thread_state_general_regs_t) -> rx_status_t {
    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());

    // Punt if registers aren't available. E.g.,
    // RX-563 (registers aren't available in synthetic exceptions)
    if thread.arch.suspended_general_regs.is_null() {
        return RX_ERR_NOT_SUPPORTED;
    }

    let input = unsafe { &*(thread.arch.suspended_general_regs) };
    debug_assert!(!thread.arch.suspended_general_regs.is_null());

    // Copy register values
    unsafe {
        core::ptr::copy_nonoverlapping(
            input.r.as_ptr(),
            out.r.as_mut_ptr(),
            input.r.len()
        );
    }
    
    out.lr = input.lr;
    out.sp = input.usp;
    out.pc = input.elr;
    out.cpsr = (input.spsr as u32) & USER_VISIBLE_FLAGS;

    RX_OK
}

pub fn arch_set_general_regs(thread: &mut Thread, input: &rx_thread_state_general_regs_t) -> rx_status_t {
    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());

    // Punt if registers aren't available. E.g.,
    // RX-563 (registers aren't available in synthetic exceptions)
    if thread.arch.suspended_general_regs.is_null() {
        return RX_ERR_NOT_SUPPORTED;
    }

    let output = unsafe { &mut *(thread.arch.suspended_general_regs) };
    debug_assert!(!thread.arch.suspended_general_regs.is_null());

    // Copy register values
    unsafe {
        core::ptr::copy_nonoverlapping(
            input.r.as_ptr(),
            output.r.as_mut_ptr(),
            input.r.len()
        );
    }
    
    output.lr = input.lr;
    output.usp = input.sp;
    output.elr = input.pc;
    output.spsr = (output.spsr & !(USER_VISIBLE_FLAGS as u64)) | ((input.cpsr & USER_VISIBLE_FLAGS) as u64);

    RX_OK
}

pub fn arch_get_single_step(thread: &Thread, single_step: &mut bool) -> rx_status_t {
    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());

    // Punt if registers aren't available. E.g.,
    // RX-563 (registers aren't available in synthetic exceptions)
    if thread.arch.suspended_general_regs.is_null() {
        return RX_ERR_NOT_SUPPORTED;
    }
    
    let regs = unsafe { &*(thread.arch.suspended_general_regs) };

    let mdscr_ss_enable = (regs.mdscr & MDSCR_SS_MASK) != 0;
    let spsr_ss_enable = (regs.spsr & SS_MASK_SPSR) != 0;

    *single_step = mdscr_ss_enable && spsr_ss_enable;
    RX_OK
}

pub fn arch_set_single_step(thread: &mut Thread, single_step: bool) -> rx_status_t {
    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());

    // Punt if registers aren't available. E.g.,
    // RX-563 (registers aren't available in synthetic exceptions)
    if thread.arch.suspended_general_regs.is_null() {
        return RX_ERR_NOT_SUPPORTED;
    }
    
    let regs = unsafe { &mut *(thread.arch.suspended_general_regs) };
    
    if single_step {
        regs.mdscr |= MDSCR_SS_MASK;
        regs.spsr |= SS_MASK_SPSR;
    } else {
        regs.mdscr &= !MDSCR_SS_MASK;
        regs.spsr &= !SS_MASK_SPSR;
    }
    
    RX_OK
}

pub fn arch_get_fp_regs(_thread: &Thread, _out: &mut rx_thread_state_fp_regs_t) -> rx_status_t {
    // There are no ARM fp regs.
    RX_OK
}

pub fn arch_set_fp_regs(_thread: &mut Thread, _input: &rx_thread_state_fp_regs_t) -> rx_status_t {
    // There are no ARM fp regs.
    RX_OK
}

pub fn arch_get_vector_regs(thread: &Thread, out: &mut rx_thread_state_vector_regs_t) -> rx_status_t {
    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());

    // ARM64 FP state access not yet implemented
    // fpstate is a raw pointer (*mut c_void) - need proper FP state structure
    if thread.arch.fpstate.is_null() {
        // Initialize with zeros
        out.fpcr = 0;
        out.fpsr = 0;
        for i in 0..32 {
            out.v[i].low = 0;
            out.v[i].high = 0;
        }
    } else {
        // TODO: Implement proper FP state access
        return RX_ERR_NOT_SUPPORTED;
    }

    RX_OK
}

pub fn arch_set_vector_regs(thread: &mut Thread, input: &rx_thread_state_vector_regs_t) -> rx_status_t {
    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());

    // ARM64 FP state access not yet implemented
    if thread.arch.fpstate.is_null() {
        // TODO: Implement proper FP state setting
        return RX_ERR_NOT_SUPPORTED;
    }

    RX_OK
}

pub fn arch_get_debug_regs(thread: &Thread, out: &mut rx_thread_state_debug_regs_t) -> rx_status_t {
    out.hw_bps_count = arm64::registers::arm64_hw_breakpoint_count() as u32;
    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());

    // The kernel ensures that this state is being kept up to date, so we can safely copy the
    // information over.
    for i in 0..out.hw_bps_count as usize {
        out.hw_bps[i].dbgbcr = thread.arch.debug_state.hw_bps[i].dbgbcr;
        out.hw_bps[i].dbgbvr = thread.arch.debug_state.hw_bps[i].dbgbvr;
    }

    // Hacked through the last debug registers for now for development.
    // THIS CODE WILL GO AWAY!
    // This normally doesn't affect functionality as normally a CPU implementation has around six
    // debug registers.
    // TODO(RX-3038): This should be exposed through a standard interface.
    //                Either the sysinfo fidl, the vDSO info mapping or some other mechanism.
    unsafe {
        let id_aa64dfr0_el1: u64;
        let mdscr_el1: u64;
        
        core::arch::asm!(
            "mrs {}, id_aa64dfr0_el1",
            out(reg) id_aa64dfr0_el1
        );
        
        core::arch::asm!(
            "mrs {}, mdscr_el1",
            out(reg) mdscr_el1
        );
        
        out.hw_bps[arm64::ARM64_MAX_HW_BREAKPOINTS - 1].dbgbvr = id_aa64dfr0_el1;
        out.hw_bps[arm64::ARM64_MAX_HW_BREAKPOINTS - 2].dbgbvr = mdscr_el1;
    }

    RX_OK
}

pub fn arch_set_debug_regs(thread: &mut Thread, input: &rx_thread_state_debug_regs_t) -> rx_status_t {
    let mut state = arm64::thread::Arm64DebugState::default();

    // We copy over the state from the input.
    let bp_count = arm64::thread::ARM64_MAX_HW_BREAKPOINTS.min(input.hw_bps_count as usize);
    for i in 0..bp_count {
        state.hw_bps[i].dbgbcr = input.hw_bps[i].dbgbcr;
        state.hw_bps[i].dbgbvr = input.hw_bps[i].dbgbvr;
    }

    // TODO: Validate debug state using proper ARM64 validation
    // For now, skip validation as the registers module uses a different type

    let thread_lock_guard = Guard::<_, IrqSave>::new(ThreadLock::get());
    thread.arch.track_debug_state = true;
    thread.arch.debug_state = state;

    RX_OK
}

pub fn arch_get_x86_register_fs(_thread: &Thread, _out: &mut u64) -> rx_status_t {
    // There are no FS register on ARM.
    RX_ERR_NOT_SUPPORTED
}

pub fn arch_set_x86_register_fs(_thread: &mut Thread, _input: &u64) -> rx_status_t {
    // There are no FS register on ARM.
    RX_ERR_NOT_SUPPORTED
}

pub fn arch_get_x86_register_gs(_thread: &Thread, _out: &mut u64) -> rx_status_t {
    // There are no GS register on ARM.
    RX_ERR_NOT_SUPPORTED
}

pub fn arch_set_x86_register_gs(_thread: &mut Thread, _input: &u64) -> rx_status_t {
    // There are no GS register on ARM.
    RX_ERR_NOT_SUPPORTED
}