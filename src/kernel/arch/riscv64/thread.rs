// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit thread context management

use crate::arch;
use crate::arch::riscv64;
use crate::kernel::thread;
use crate::rustux::types::*;

/// RISC-V 64-bit thread context
///
/// This structure represents the saved state of a thread on RISC-V.
/// The register layout must match the assembly in asm.S.
#[repr(C)]
pub struct RiscvThreadContext {
    // Stack pointer (saved separately)
    pub sp: u64,      // x2 - stack pointer

    // Callee-saved registers (saved during context switch)
    pub s0: u64,      // x8 / fp - frame pointer
    pub s1: u64,      // x9
    pub s2: u64,      // x18
    pub s3: u64,      // x19
    pub s4: u64,      // x20
    pub s5: u64,      // x21
    pub s6: u64,      // x22
    pub s7: u64,      // x23
    pub s8: u64,      // x24
    pub s9: u64,      // x25
    pub s10: u64,     // x26
    pub s11: u64,     // x27
    pub ra: u64,      // x1 - return address
}

impl RiscvThreadContext {
    /// Create a new zero-initialized thread context
    pub const fn new() -> Self {
        Self {
            sp: 0,
            s0: 0, s1: 0,
            s2: 0, s3: 0, s4: 0, s5: 0, s6: 0, s7: 0,
            s8: 0, s9: 0, s10: 0, s11: 0,
            ra: 0,
        }
    }
}

/// Interrupt frame structure for exceptions/syscalls
///
/// This is the structure saved by the exception entry code.
#[repr(C)]
pub struct RiscvIframe {
    // General purpose registers
    pub ra: u64,      // x1
    pub sp: u64,      // x2
    pub gp: u64,      // x3
    pub tp: u64,      // x4
    pub t0: u64,      // x5
    pub t1: u64,      // x6
    pub t2: u64,      // x7
    pub s0: u64,      // x8
    pub s1: u64,      // x9
    pub a0: u64,      // x10
    pub a1: u64,      // x11
    pub a2: u64,      // x12
    pub a3: u64,      // x13
    pub a4: u64,      // x14
    pub a5: u64,      // x15
    pub a6: u64,      // x16
    pub a7: u64,      // x17
    pub s2: u64,      // x18
    pub s3: u64,      // x19
    pub s4: u64,      // x20
    pub s5: u64,      // x21
    pub s6: u64,      // x22
    pub s7: u64,      // x23
    pub s8: u64,      // x24
    pub s9: u64,      // x25
    pub s10: u64,     // x26
    pub s11: u64,     // x27
    pub t3: u64,      // x28
    pub t4: u64,      // x29
    pub t5: u64,      // x30
    pub t6: u64,      // x31

    // Special registers
    pub pc: u64,      // Program counter
    pub status: u64,  // sstatus register
}

/// Initialize a thread context for entry into a function
pub fn arch_thread_initialize(
    thread: &mut thread::Thread,
    entry: extern "C" fn(usize) -> !,
    arg: usize,
) {
    let mut stack_top = thread.stack_top() as u64;

    // Align stack to 16 bytes
    stack_top &= !0xf;

    // Allocate space for the initial context frame on the stack
    // The context switch will save: ra + s0-s11 = 13 * 8 = 104 bytes
    // Round to 112 bytes for alignment
    stack_top -= 112;

    // Set up the initial thread context on the stack
    // The context should be at the bottom of the allocated space
    let ctx_ptr = stack_top as *mut RiscvThreadContext;
    unsafe {
        (*ctx_ptr).ra = entry as u64;
        (*ctx_ptr).s0 = 0;
        (*ctx_ptr).s1 = 0;
        (*ctx_ptr).s2 = 0;
        (*ctx_ptr).s3 = 0;
        (*ctx_ptr).s4 = 0;
        (*ctx_ptr).s5 = 0;
        (*ctx_ptr).s6 = 0;
        (*ctx_ptr).s7 = 0;
        (*ctx_ptr).s8 = 0;
        (*ctx_ptr).s9 = 0;
        (*ctx_ptr).s10 = 0;
        (*ctx_ptr).s11 = arg as u64; // Pass argument in s11 (callee-saved)
    }

    // Save the stack pointer in the thread's arch state
    // TODO: This needs to be properly integrated with the Thread's arch field
    let _ = thread;
    let _ = stack_top;
}

/// Perform a context switch between two threads
///
/// # Arguments
///
/// * `old_thread` - Thread to switch from
/// * `new_thread` - Thread to switch to
///
/// # Safety
///
/// Both threads must be valid and the function must be called
/// with interrupts disabled.
pub unsafe fn arch_context_switch(
    old_thread: &mut thread::Thread,
    new_thread: &mut thread::Thread,
) {
    extern "C" {
        fn riscv_context_switch(old_sp: *mut u64, new_sp: u64);
    }

    // TODO: This needs to be properly integrated with the Thread's arch field
    let _ = old_thread;
    let _ = new_thread;
    // let old_sp = &mut (old_thread.arch.suspended_general_regs as *mut _ as *mut u64);
    // let new_sp = new_thread.arch.suspended_general_regs as u64;
    // riscv_context_switch(old_sp, new_sp);
}

/// Get the current stack pointer
#[inline(always)]
pub fn arch_get_sp() -> usize {
    extern "C" {
        fn riscv_get_sp() -> usize;
    }
    unsafe { riscv_get_sp() }
}

/// Set the stack pointer
#[inline(always)]
pub fn arch_set_sp(sp: usize) {
    extern "C" {
        fn riscv_set_sp(sp: usize);
    }
    unsafe { riscv_set_sp(sp) }
}

/// Get the current thread pointer value
#[inline(always)]
pub fn arch_thread_get_pointer() -> usize {
    let tp: usize;
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) tp);
    }
    tp
}

/// Set the thread pointer value
#[inline(always)]
pub fn arch_thread_set_pointer(tp: usize) {
    unsafe {
        core::arch::asm!("mv tp, {}", in(reg) tp);
    }
}

/// Halt the current CPU
#[inline(always)]
pub fn arch_halt() {
    unsafe {
        core::arch::asm!("wfi");
    }
}

/// Pause for spin-waiting
#[inline(always)]
pub fn arch_pause() {
    unsafe {
        core::arch::asm!("fence");
    }
}
