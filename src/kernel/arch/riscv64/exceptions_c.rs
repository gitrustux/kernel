// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit exception handlers
//!
//! This module provides exception handling for RISC-V, including
//! page faults, illegal instructions, and system calls.

#![no_std]

use crate::arch::riscv64::registers::csr;
use crate::arch::riscv64::registers::scause;
use crate::debug;
use crate::kernel::thread;
use crate::rustux::types::*;

/// RISC-V interrupt frame
///
/// Captures the processor state at exception time
#[repr(C)]
pub struct RiscvIframe {
    /// General-purpose registers
    pub ra: u64,   // x1 (return address)
    pub sp: u64,   // x2 (stack pointer)
    pub gp: u64,   // x3 (global pointer)
    pub tp: u64,   // x4 (thread pointer)
    pub t0: u64,   // x5
    pub t1: u64,   // x6
    pub t2: u64,   // x7
    pub s0: u64,   // x8 / fp
    pub s1: u64,   // x9
    pub a0: u64,   // x10 (argument/return value)
    pub a1: u64,   // x11
    pub a2: u64,   // x12
    pub a3: u64,   // x13
    pub a4: u64,   // x14
    pub a5: u64,   // x15
    pub a6: u64,   // x16
    pub a7: u64,   // x17
    pub s2: u64,   // x18
    pub s3: u64,   // x19
    pub s4: u64,   // x20
    pub s5: u64,   // x21
    pub s6: u64,   // x22
    pub s7: u64,   // x23
    pub s8: u64,   // x24
    pub s9: u64,   // x25
    pub s10: u64,  // x26
    pub s11: u64,  // x27
    pub t3: u64,   // x28
    pub t4: u64,   // x29
    pub t5: u64,   // x30
    pub t6: u64,   // x31
    /// Exception-specific registers
    pub pc: u64,   // Program counter (saved from EPC)
    pub status: u64, // Status register (saved from SSTATUS)
    pub cause: u64, // Exception cause (SCAUSE)
    pub tval: u64,  // Trap value (STVAL)
}

/// Exception dispatch context
#[repr(C)]
pub struct ExceptionContext {
    pub iframe: *mut RiscvIframe,
    pub cause: u64,
    pub tval: u64,
}

/// Exception type for user-space dispatch
#[repr(u32)]
pub enum ExceptionType {
    Unknown = 0,
    IllegalInstruction = 1,
    Breakpoint = 2,
    PageFault = 3,
    UserCopy = 4,
    General = 8,
}

/// Dump the exception frame for debugging
fn dump_iframe(iframe: &RiscvIframe) {
    println!("RISC-V Exception Frame:");
    println!("  PC     = {:#18x}", iframe.pc);
    println!("  SSTATUS = {:#18x}", iframe.status);
    println!("  SCAUSE  = {:#18x}", iframe.cause);
    println!("  STVAL   = {:#18x}", iframe.tval);
    println!();
    println!("  RA  = {:#18x}  SP  = {:#18x}  GP  = {:#18x}", iframe.ra, iframe.sp, iframe.gp);
    println!("  TP  = {:#18x}  T0  = {:#18x}  T1  = {:#18x}", iframe.tp, iframe.t0, iframe.t1);
    println!("  T2  = {:#18x}  S0  = {:#18x}  S1  = {:#18x}", iframe.t2, iframe.s0, iframe.s1);
    println!("  A0  = {:#18x}  A1  = {:#18x}  A2  = {:#18x}", iframe.a0, iframe.a1, iframe.a2);
    println!("  A3  = {:#18x}  A4  = {:#18x}  A5  = {:#18x}", iframe.a3, iframe.a4, iframe.a5);
    println!("  A6  = {:#18x}  A7  = {:#18x}", iframe.a6, iframe.a7);
    println!("  S2  = {:#18x}  S3  = {:#18x}  S4  = {:#18x}", iframe.s2, iframe.s3, iframe.s4);
    println!("  S5  = {:#18x}  S6  = {:#18x}  S7  = {:#18x}", iframe.s5, iframe.s6, iframe.s7);
    println!("  S8  = {:#18x}  S9  = {:#18x}  S10 = {:#18x}", iframe.s8, iframe.s9, iframe.s10);
    println!("  S11 = {:#18x}  T3  = {:#18x}  T4  = {:#18x}", iframe.s11, iframe.t3, iframe.t4);
    println!("  T5  = {:#18x}  T6  = {:#18x}", iframe.t5, iframe.t6);
}

/// Check if exception came from user mode
fn is_from_user(iframe: &RiscvIframe) -> bool {
    // SPP bit in SSTATUS indicates previous privilege mode
    // 0 = user mode, 1 = supervisor mode
    (iframe.status & (1 << 8)) == 0
}

/// Fatal exception handler - prints diagnostic and halts
#[cold]
fn exception_die(iframe: &RiscvIframe, msg: &str) -> ! {
    println!("{}", msg);
    println!("Exception cause: {:#x}", iframe.cause);
    if iframe.tval != 0 {
        println!("Trap value: {:#x}", iframe.tval);
    }
    dump_iframe(iframe);

    // TODO: platform_halt(HALT_ACTION_HALT, HALT_REASON_SW_PANIC);

    loop {
        unsafe { core::arch::asm!("wfi") };
    }
}

/// Page fault handler
fn riscv_page_fault_handler(iframe: &mut RiscvIframe) {
    let is_load = match iframe.cause {
        scause::LOAD_PAGE_FAULT => true,
        scause::STORE_AMO_PAGE_FAULT => false,
        scause::INSTRUCTION_PAGE_FAULT => {
            // Instruction page fault
            exception_die(iframe, "Instruction page fault\n");
        }
        _ => {
            exception_die(iframe, "Invalid page fault cause\n");
        }
    };

    let fault_addr = iframe.tval as VAddr;

    println!(
        "Page fault: {} address {:#x} at PC {:#x}",
        if is_load { "load" } else { "store" },
        fault_addr,
        iframe.pc
    );

    // Check if user mode
    if is_from_user(iframe) {
        // TODO: Dispatch user exception
        // For now, fatal error
        exception_die(iframe, "User page fault (unimplemented)\n");
    } else {
        // Kernel page fault - always fatal
        exception_die(iframe, "Kernel page fault\n");
    }
}

/// Illegal instruction handler
fn riscv_illegal_instruction_handler(iframe: &mut RiscvIframe) {
    if is_from_user(iframe) {
        // TODO: Try to dispatch to user-space exception handler
        exception_die(iframe, "User illegal instruction (unimplemented)\n");
    } else {
        exception_die(iframe, "Kernel illegal instruction\n");
    }
}

/// Breakpoint handler
fn riscv_breakpoint_handler(iframe: &mut RiscvIframe) {
    if is_from_user(iframe) {
        // TODO: Try to dispatch to user-space exception handler
        exception_die(iframe, "User breakpoint (unimplemented)\n");
    } else {
        exception_die(iframe, "Kernel breakpoint\n");
    }
}

/// Supervisor software interrupt (IPI from other harts)
fn riscv_software_interrupt_handler(_iframe: &mut RiscvIframe) {
    // TODO: Handle inter-processor interrupt
    // Typically used for TLB shootdowns, rescheduling, etc.
    println!("Software interrupt received");
}

/// Supervisor timer interrupt
fn riscv_timer_interrupt_handler(_iframe: &mut RiscvIframe) {
    // TODO: Handle timer interrupt
    // Typically used for preemption, timeout, etc.
    println!("Timer interrupt received");
}

/// Supervisor external interrupt (PLIC)
fn riscv_external_interrupt_handler(_iframe: &mut RiscvIframe) {
    // TODO: Read from PLIC to determine which device interrupted
    println!("External interrupt received");
}

/// Environment call from user mode (syscall)
fn riscv_syscall_handler(iframe: &mut RiscvIframe) {
    // Syscall number is in a7
    // Arguments are in a0-a6
    // Return value goes in a0

    let syscall_num = iframe.a7 as u64;
    let _arg0 = iframe.a0;
    let _arg1 = iframe.a1;
    let _arg2 = iframe.a2;
    let _arg3 = iframe.a3;
    let _arg4 = iframe.a4;
    let _arg5 = iframe.a5;
    let _arg6 = iframe.a6;

    // TODO: Dispatch to syscall table
    // For now, just return an error
    println!("Syscall {} (unimplemented)", syscall_num);
    iframe.a0 = (-1i64) as u64; // Error
}

/// Main exception dispatch handler
///
/// Called from assembly exception entry point in exceptions.S
///
/// # Arguments
///
/// * `iframe` - Pointer to the interrupt frame
///
/// # Safety
///
/// iframe must point to valid memory
#[no_mangle]
pub unsafe extern "C" fn riscv_exception_handler(iframe: *mut RiscvIframe) {
    let iframe = &mut *iframe;

    // Get the exception cause
    let cause = iframe.cause;

    // Check if it's an interrupt (high bit set)
    if cause & scause::INTERRUPT_BIT != 0 {
        // It's an interrupt
        let interrupt_code = cause & !scause::INTERRUPT_BIT;

        match interrupt_code {
            scause::SUPERVISOR_SOFTWARE_INTERRUPT => {
                riscv_software_interrupt_handler(iframe);
            }
            scause::SUPERVISOR_TIMER_INTERRUPT => {
                riscv_timer_interrupt_handler(iframe);
            }
            scause::SUPERVISOR_EXTERNAL_INTERRUPT => {
                riscv_external_interrupt_handler(iframe);
            }
            _ => {
                println!("Unknown interrupt: {:#x}", interrupt_code);
            }
        }
    } else {
        // It's an exception
        match cause {
            scause::INSTRUCTION_ADDRESS_MISALIGNED => {
                exception_die(iframe, "Instruction address misaligned\n");
            }
            scause::INSTRUCTION_ACCESS_FAULT => {
                exception_die(iframe, "Instruction access fault\n");
            }
            scause::ILLEGAL_INSTRUCTION => {
                riscv_illegal_instruction_handler(iframe);
            }
            scause::BREAKPOINT => {
                riscv_breakpoint_handler(iframe);
            }
            scause::LOAD_ADDRESS_MISALIGNED => {
                exception_die(iframe, "Load address misaligned\n");
            }
            scause::LOAD_ACCESS_FAULT => {
                exception_die(iframe, "Load access fault\n");
            }
            scause::STORE_AMO_ADDRESS_MISALIGNED => {
                exception_die(iframe, "Store/AMO address misaligned\n");
            }
            scause::STORE_AMO_ACCESS_FAULT => {
                exception_die(iframe, "Store/AMO access fault\n");
            }
            scause::ENV_CALL_FROM_U_MODE => {
                riscv_syscall_handler(iframe);
            }
            scause::ENV_CALL_FROM_S_MODE => {
                exception_die(iframe, "Environment call from S-mode\n");
            }
            scause::INSTRUCTION_PAGE_FAULT => {
                riscv_page_fault_handler(iframe);
            }
            scause::LOAD_PAGE_FAULT => {
                riscv_page_fault_handler(iframe);
            }
            scause::STORE_AMO_PAGE_FAULT => {
                riscv_page_fault_handler(iframe);
            }
            _ => {
                println!("Unknown exception: {:#x}", cause);
                exception_die(iframe, "Unknown exception\n");
            }
        }
    }
}

/// Get the SSTATUS register value
#[inline]
pub fn riscv_read_sstatus() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::SSTATUS) }
}

/// Get the SCAUSE register value
#[inline]
pub fn riscv_read_scause() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::SCAUSE) }
}

/// Get the STVAL register value
#[inline]
pub fn riscv_read_stval() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::STVAL) }
}

/// Get the SEPC register value
#[inline]
pub fn riscv_read_sepc() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::SEPC) }
}

/// Write to the SEPC register (to set where to return after exception)
#[inline]
pub fn riscv_write_sepc(value: u64) {
    unsafe { crate::arch::riscv64::registers::write_csr(csr::SEPC, value) }
}

/// End of interrupt - acknowledge interrupt handling
#[inline]
pub fn riscv_end_of_interrupt() {
    // For RISC-V, there's no explicit EOI like x86's APIC EOI
    // For PLIC (Platform-Level Interrupt Controller), we need
    // to claim and complete the interrupt
    // TODO: Implement PLIC handling
}

// External functions for exception dispatch
extern "C" {
    /// Dispatch exception to user-space handler
    fn dispatch_user_exception(
        kind: u32,
        context: *mut ExceptionContext,
    ) -> i32;
}
