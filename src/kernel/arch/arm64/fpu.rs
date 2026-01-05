// Copyright 2025 The Rustux Authors
// Copyright (c) 2015 Google Inc. All rights reserved
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64;
use crate::bits;
use crate::kernel::thread::{self, Thread};
use crate::trace::*;

const LOCAL_TRACE: bool = false;

/* FPEN bits in the cpacr register
 * 0 means all fpu instructions fault
 * 3 means no faulting at all EL levels
 * other values are not useful to us
 */
const FPU_ENABLE_MASK: u64 = 3 << 20;

#[inline]
fn is_fpu_enabled(cpacr: u32) -> bool {
    bits::BITS(cpacr, 21, 20) != 0
}

fn arm64_fpu_load_state(t: &Thread) {
    let fpstate = &t.arch.fpstate;

    LTRACEF!("cpu {}, thread {}, load fpstate {:p}\n", arch_curr_cpu_num(), t.name, fpstate);

    // Static assertion to ensure size is correct
    assert_eq!(core::mem::size_of_val(&fpstate.regs), 16 * 32);

    unsafe {
        core::arch::asm!(
            "ldp     q0, q1, [{0}, #(0 * 32)]",
            "ldp     q2, q3, [{0}, #(1 * 32)]",
            "ldp     q4, q5, [{0}, #(2 * 32)]",
            "ldp     q6, q7, [{0}, #(3 * 32)]",
            "ldp     q8, q9, [{0}, #(4 * 32)]",
            "ldp     q10, q11, [{0}, #(5 * 32)]",
            "ldp     q12, q13, [{0}, #(6 * 32)]",
            "ldp     q14, q15, [{0}, #(7 * 32)]",
            "ldp     q16, q17, [{0}, #(8 * 32)]",
            "ldp     q18, q19, [{0}, #(9 * 32)]",
            "ldp     q20, q21, [{0}, #(10 * 32)]",
            "ldp     q22, q23, [{0}, #(11 * 32)]",
            "ldp     q24, q25, [{0}, #(12 * 32)]",
            "ldp     q26, q27, [{0}, #(13 * 32)]",
            "ldp     q28, q29, [{0}, #(14 * 32)]",
            "ldp     q30, q31, [{0}, #(15 * 32)]",
            "msr     fpcr, {1}",
            "msr     fpsr, {2}",
            in(reg) &fpstate.regs[0],
            in(reg) fpstate.fpcr as u64,
            in(reg) fpstate.fpsr as u64,
        );
    }
}

#[no_sanitize(address, memory, thread)]
fn arm64_fpu_save_state(t: &mut Thread) {
    let fpstate = &mut t.arch.fpstate;

    LTRACEF!("cpu {}, thread {}, save fpstate {:p}\n", arch_curr_cpu_num(), t.name, fpstate);

    unsafe {
        core::arch::asm!(
            "stp     q0, q1, [{0}, #(0 * 32)]",
            "stp     q2, q3, [{0}, #(1 * 32)]",
            "stp     q4, q5, [{0}, #(2 * 32)]",
            "stp     q6, q7, [{0}, #(3 * 32)]",
            "stp     q8, q9, [{0}, #(4 * 32)]",
            "stp     q10, q11, [{0}, #(5 * 32)]",
            "stp     q12, q13, [{0}, #(6 * 32)]",
            "stp     q14, q15, [{0}, #(7 * 32)]",
            "stp     q16, q17, [{0}, #(8 * 32)]",
            "stp     q18, q19, [{0}, #(9 * 32)]",
            "stp     q20, q21, [{0}, #(10 * 32)]",
            "stp     q22, q23, [{0}, #(11 * 32)]",
            "stp     q24, q25, [{0}, #(12 * 32)]",
            "stp     q26, q27, [{0}, #(13 * 32)]",
            "stp     q28, q29, [{0}, #(14 * 32)]",
            "stp     q30, q31, [{0}, #(15 * 32)]",
            in(reg) &mut fpstate.regs[0],
        );

        // These are 32-bit values, but the mrs instruction always uses a
        // 64-bit source register.
        let mut fpcr: u64 = 0;
        let mut fpsr: u64 = 0;
        
        core::arch::asm!("mrs {}, fpcr", out(reg) fpcr);
        core::arch::asm!("mrs {}, fpsr", out(reg) fpsr);
        
        fpstate.fpcr = fpcr as u32;
        fpstate.fpsr = fpsr as u32;
    }

    LTRACEF!("thread {}, fpcr {:x}, fpsr {:x}\n", t.name, fpstate.fpcr, fpstate.fpsr);
}

/// Save fpu state if the thread had dirtied it and disable the fpu
#[no_mangle]
#[no_sanitize(address, memory, thread)]
pub extern "C" fn arm64_fpu_context_switch(oldthread: *mut Thread, newthread: *mut Thread) {
    unsafe {
        let mut cpacr: u64;
        core::arch::asm!("mrs {}, cpacr_el1", out(reg) cpacr);
        
        if is_fpu_enabled(cpacr as u32) {
            let oldthread = &mut *oldthread;
            LTRACEF!("saving state on thread {}\n", oldthread.name);

            // save the state
            arm64_fpu_save_state(oldthread);

            // disable the fpu again
            cpacr &= !FPU_ENABLE_MASK;
            core::arch::asm!("msr cpacr_el1, {}", in(reg) cpacr);
            core::arch::asm!("isb sy");
        }
    }
}

/// Called because of a fpu instruction used exception
#[no_mangle]
pub extern "C" fn arm64_fpu_exception(iframe: *mut arm64::arm64_iframe_long, exception_flags: u32) {
    let t = thread::get_current_thread();
    LTRACEF!("cpu {}, thread {}, flags 0x{:x}\n", arch_curr_cpu_num(), t.name, exception_flags);

    // only valid to be called if exception came from lower level
    debug_assert!((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) != 0);

    unsafe {
        let mut cpacr: u64;
        core::arch::asm!("mrs {}, cpacr_el1", out(reg) cpacr);
        
        debug_assert!(!is_fpu_enabled(cpacr as u32));

        // enable the fpu
        cpacr |= FPU_ENABLE_MASK;
        core::arch::asm!("msr cpacr_el1, {}", in(reg) cpacr);
        core::arch::asm!("isb sy");

        // load the state from the current cpu
        if likely(t) != core::ptr::null_mut() {
            arm64_fpu_load_state(&*t);
        }
    }
}

#[inline(always)]
fn arch_curr_cpu_num() -> u32 {
    unsafe { arm64::arch_curr_cpu_num() }
}

// Helper for macros
#[inline(always)]
fn likely(ptr: *const Thread) -> *const Thread {
    ptr
}