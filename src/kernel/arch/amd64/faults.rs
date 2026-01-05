// Copyright 2025 The Rustux Authors
// Copyright (c) 2009 Corey Tabaka
// Copyright (c) 2015 Intel Corporation
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Exception and fault handlers
//!
//! This module handles all x86 exceptions including page faults,
//! general protection faults, and debug exceptions.

#![no_std]

use crate::kernel::arch::amd64;
use crate::kernel::arch::amd64::apic;
use crate::kernel::arch::amd64::descriptor;
use crate::kernel::arch::amd64::feature;
use crate::kernel::arch::amd64::mmu;
use crate::kernel::arch::amd64::registers::*;
use crate::kernel::arch::amd64::X86Iframe;
use crate::kernel::debug;
use crate::print;
use crate::println;
use crate::kernel::thread;
use crate::rustux::types::*;

/// Page fault error code flags
pub const PFEX_P: u64 = 1 << 0;   // Page present
pub const PFEX_W: u64 = 1 << 1;   // Write access
pub const PFEX_U: u64 = 1 << 2;   // User mode
pub const PFEX_RSV: u64 = 1 << 3; // Reserved bit set
pub const PFEX_I: u64 = 1 << 4;   // Instruction fetch
pub const PFEX_PK: u64 = 1 << 5;  // Protection key violation
pub const PFEX_SGX: u64 = 1 << 15; // SGX violation

/// X86 exception vectors
pub const X86_INT_DEBUG: u64 = 1;
pub const X86_INT_NMI: u64 = 2;
pub const X86_INT_BREAKPOINT: u64 = 3;
pub const X86_INT_INVALID_OP: u64 = 6;
pub const X86_INT_DEVICE_NA: u64 = 7;
pub const X86_INT_DOUBLE_FAULT: u64 = 8;
pub const X86_INT_FPU_FP_ERROR: u64 = 16;
pub const X86_INT_SIMD_FP_ERROR: u64 = 19;
pub const X86_INT_GP_FAULT: u64 = 13;
pub const X86_INT_PAGE_FAULT: u64 = 14;
pub const X86_INT_APIC_SPURIOUS: u64 = 255;
pub const X86_INT_APIC_ERROR: u64 = 0xfe;
pub const X86_INT_APIC_TIMER: u64 = 0xfd;

/// Exception types for user-space dispatch
pub const ZX_EXCP_HW_BREAKPOINT: u32 = 0x1;
pub const ZX_EXCP_SW_BREAKPOINT: u32 = 0x2;
pub const ZX_EXCP_GENERAL: u32 = 0x8;
pub const ZX_EXCP_UNDEFINED_INSTRUCTION: u32 = 0x4;
pub const ZX_EXCP_FATAL_PAGE_FAULT: u32 = 0x20;

/// Page fault flags for VM
pub const VMM_PF_FLAG_WRITE: u32 = 1 << 0;
pub const VMM_PF_FLAG_USER: u32 = 1 << 1;
pub const VMM_PF_FLAG_INSTRUCTION: u32 = 1 << 2;
pub const VMM_PF_FLAG_NOT_PRESENT: u32 = 1 << 3;

/// Check if the exception came from user mode
fn is_from_user(frame: &X86Iframe) -> bool {
    descriptor::SELECTOR_PL(frame.user_cs) != 0
}

/// Dump the fault frame for debugging
fn dump_fault_frame(frame: &X86Iframe) {
    let cr2 = unsafe { x86_get_cr2() };

    println!(
        " CS:  {:#18x} RIP: {:#18x} EFL: {:#18x} CR2: {:#18x}",
        frame.user_cs, frame.rip, frame.rflags, cr2
    );
    println!(
        " RAX: {:#18x} RBX: {:#18x} RCX: {:#18x} RDX: {:#18x}",
        frame.rax, frame.rbx, frame.rcx, frame.rdx
    );
    println!(
        " RSI: {:#18x} RDI: {:#18x} RBP: {:#18x} RSP: {:#18x}",
        frame.rsi, frame.rdi, frame.rbp, frame.rsp
    );
    println!(
        "  R8: {:#18x}  R9: {:#18x} R10: {:#18x} R11: {:#18x}",
        frame.r8, frame.r9, frame.r10, frame.r11
    );
    println!(
        " R12: {:#18x} R13: {:#18x} R14: {:#18x} R15: {:#18x}",
        frame.r12, frame.r13, frame.r14, frame.r15
    );
    println!("errc: {:#18x}", 0); // TODO: error code needs to be passed as parameter
}

/// Dump page fault error information
fn x86_dump_pfe(frame: &X86Iframe, cr2: u64, err_code: u64) {
    println!("<PAGE FAULT> Instruction Pointer   = 0x{:x}:0x{:x}",
        frame.user_cs & 0xFFFF, frame.rip);
    println!("<PAGE FAULT> Stack Pointer         = 0x{:x}:0x{:x}",
        frame.user_ss & 0xFFFF, frame.rsp);
    println!("<PAGE FAULT> Fault Linear Address  = 0x{:x}", cr2);
    println!("<PAGE FAULT> Error Code Value      = 0x{:x}", err_code);

    let access_type = if err_code & PFEX_W != 0 { "write" } else { "read" };
    let mode = if err_code & PFEX_U != 0 { "user" } else { "supervisor" };
    let fetch_type = if err_code & PFEX_I != 0 { "instruction" } else { "data" };
    let rsv = if err_code & PFEX_RSV != 0 { " rsv" } else { "" };
    let present = if err_code & PFEX_P != 0 {
        "protection violation"
    } else {
        "page not present"
    };

    println!(
        "<PAGE FAULT> Error Code Type       = {} {} {}{}, {}",
        mode, access_type, fetch_type, rsv, present
    );
}

/// Fatal page fault handler - halts the system
fn x86_fatal_pfe_handler(frame: &X86Iframe, cr2: u64, err_code: u64) -> ! {
    x86_dump_pfe(frame, cr2, err_code);

    // TODO: Dump thread during panic
    // dump_thread_during_panic(get_current_thread(), true);

    let error_code = err_code;

    if error_code & PFEX_U != 0 {
        // User mode page fault
        match error_code {
            4..=7 => {
                exception_die(frame, "User Page Fault exception, halting\n");
            }
            _ => {}
        }
    } else {
        // Supervisor mode page fault
        match error_code {
            0..=3 => {
                exception_die(frame, "Supervisor Page Fault exception, halting\n");
            }
            _ => {}
        }
    }

    exception_die(frame, "unhandled page fault, halting\n");
}

/// Page fault handler
fn x86_pfe_handler(frame: &mut X86Iframe, error_code: u64) -> core::result::Result<(), i32> {
    let va = unsafe { x86_get_cr2() } as usize;

    // TODO: Re-enable interrupts, manage preemption
    // thread_preempt_reenable_no_resched();
    // arch_set_blocking_disallowed(false);
    // arch_enable_ints();

    // Auto-call to restore state on exit
    // let _guard = scopeguard::guard((), |_| {
    //     arch_disable_ints();
    //     arch_set_blocking_disallowed(true);
    //     thread_preempt_disable();
    // });

    // Check for flags we're not prepared to handle
    let unhandled_bits = error_code & !(PFEX_I | PFEX_U | PFEX_W | PFEX_P);
    if unhandled_bits != 0 {
        println!(
            "x86_pfe_handler: unhandled error code bits set, error code {:#x}",
            error_code
        );
        return Err(-1); // ZX_ERR_NOT_SUPPORTED
    }

    // Check for potential SMAP failure
    let supervisor_access = error_code & PFEX_U == 0;
    let page_present = error_code & PFEX_P != 0;
    let ac_clear = (frame.rflags & X86_FLAGS_AC) == 0;
    let smap_enabled = feature::x86_feature_test(feature::X86_FEATURE_SMAP);
    let user_addr = crate::kernel::arch::amd64::arch::is_user_address(va);

    if supervisor_access && page_present && smap_enabled && ac_clear && user_addr {
        println!(
            "x86_pfe_handler: potential SMAP failure, supervisor access at address {:#x}",
            va
        );
        return Err(-2); // ZX_ERR_ACCESS_DENIED
    }

    // Convert PF error codes to page fault flags
    let mut flags = 0u32;
    if error_code & PFEX_W != 0 {
        flags |= VMM_PF_FLAG_WRITE;
    }
    if error_code & PFEX_U != 0 {
        flags |= VMM_PF_FLAG_USER;
    }
    if error_code & PFEX_I != 0 {
        flags |= VMM_PF_FLAG_INSTRUCTION;
    }
    if error_code & PFEX_P == 0 {
        flags |= VMM_PF_FLAG_NOT_PRESENT;
    }

    // Call the high level page fault handler
    // TODO: Implement vmm_page_fault_handler
    // let pf_err = vmm_page_fault_handler(va, flags);
    // if pf_err == ZX_OK {
    //     return Ok(());
    // }

    // Check if a resume address is specified
    // let current_thread = get_current_thread();
    // if current_thread.arch.page_fault_resume != null_mut() {
    //     frame.ip = current_thread.arch.page_fault_resume as u64;
    //     return Ok(());
    // }

    // Let high level code deal with user space faults
    if is_from_user(frame) {
        // TODO: Dispatch user exception
        // kcounter_add(exceptions_user, 1);
        // return call_dispatch_user_exception(ZX_EXCP_FATAL_PAGE_FAULT, frame);
    }

    // Fall through to fatal path
    Err(-1)
}

/// Debug exception handler
fn x86_debug_handler(frame: &mut X86Iframe) {
    let thread = unsafe { thread::get_current_thread() };

    // Read debug status register DR6
    if let Some(t) = thread {
        unsafe { x86_read_debug_status(&mut t.arch.debug_state) };
    }

    // Try to dispatch to user-space handler
    if try_dispatch_user_exception(frame, ZX_EXCP_HW_BREAKPOINT) {
        return;
    }

    exception_die(frame, "unhandled hw breakpoint, halting\n");
}

/// Breakpoint exception handler (INT 3)
fn x86_breakpoint_handler(frame: &mut X86Iframe) {
    if try_dispatch_user_exception(frame, ZX_EXCP_SW_BREAKPOINT) {
        return;
    }

    exception_die(frame, "unhandled sw breakpoint, halting\n");
}

/// General protection fault handler
fn x86_gpf_handler(frame: &mut X86Iframe) {
    assert!(crate::kernel::arch::amd64::arch::arch_ints_disabled());

    // Check if we were doing a GPF test (e.g., to check if an MSR exists)
    let percpu = unsafe { crate::kernel::arch::amd64::mp::x86_get_percpu() };
    if (*percpu).gpf_return_target != 0 {
        assert!(!is_from_user(frame));

        // Set up return to new address
        frame.rip = (*percpu).gpf_return_target as u64;
        // Note: Can't directly assign to pointer field, need to handle differently
        // For now, just skip the GPF return target mechanism
        return;
    }

    if try_dispatch_user_exception(frame, ZX_EXCP_GENERAL) {
        return;
    }

    exception_die(frame, "unhandled gpf, halting\n");
}

/// Invalid opcode handler
fn x86_invop_handler(frame: &mut X86Iframe) {
    if try_dispatch_user_exception(frame, ZX_EXCP_UNDEFINED_INSTRUCTION) {
        return;
    }

    exception_die(frame, "invalid opcode, halting\n");
}

/// Double fault handler
fn x86_df_handler(frame: &X86Iframe) {
    // Do not give the user exception handler the opportunity to handle double faults
    exception_die(frame, "double fault, halting\n");
}

/// NMI handler
fn x86_nmi_handler(_frame: &X86Iframe) {
    // NMI handler - typically used for watchdog or hardware diagnostics
}

/// Unhandled exception handler
fn x86_unhandled_exception(frame: &mut X86Iframe) {
    if try_dispatch_user_exception(frame, ZX_EXCP_GENERAL) {
        return;
    }

    exception_die(frame, "unhandled exception, halting\n");
}

/// Try to dispatch exception to user-space handler
///
/// Returns true if the exception was handled by user-space
fn try_dispatch_user_exception(frame: &X86Iframe, kind: u32) -> bool {
    if !is_from_user(frame) {
        return false;
    }

    // TODO: Implement user exception dispatch
    // struct arch_exception_context context = {false, frame, 0};
    // thread_preempt_reenable_no_resched();
    // arch_set_blocking_disallowed(false);
    // arch_enable_ints();
    // let erc = dispatch_user_exception(kind, context);
    // arch_disable_ints();
    // arch_set_blocking_disallowed(true);
    // thread_preempt_disable();
    // if erc == ZX_OK {
    //     return true;
    // }

    false
}

/// Fatal exception handler - prints diagnostic and halts
fn exception_die(frame: &X86Iframe, msg: &str) -> ! {
    // TODO: platform_panic_start();
    // TODO: Get vector from somewhere (passed on stack before iframe)
    println!("exception at rip={:#x}", frame.rip);
    print!("{}", msg);
    dump_fault_frame(frame);

    // TODO: Try to dump user stack
    // crashlog.iframe = frame;
    // platform_halt(HALT_ACTION_HALT, HALT_REASON_SW_PANIC);

    loop {
        unsafe { crate::kernel::arch::amd64::registers::x86_hlt() };
    }
}

/// Main exception dispatch handler
///
/// Called from assembly exception entry point
/// The vector number is passed on the stack by the CPU before the iframe
#[no_mangle]
pub unsafe extern "C" fn x86_exception_handler(frame: *mut X86Iframe, vector: u64) {
    let frame = &mut *frame;

    // Process pending signals before handling exception
    // x86_iframe_process_pending_signals(frame);

    match vector {
        X86_INT_DEBUG => {
            x86_debug_handler(frame);
        }
        X86_INT_NMI => {
            x86_nmi_handler(frame);
        }
        X86_INT_BREAKPOINT => {
            x86_breakpoint_handler(frame);
        }
        X86_INT_INVALID_OP => {
            x86_invop_handler(frame);
        }
        X86_INT_DEVICE_NA => {
            exception_die(frame, "device na fault\n");
        }
        X86_INT_DOUBLE_FAULT => {
            x86_df_handler(frame);
        }
        X86_INT_FPU_FP_ERROR | X86_INT_SIMD_FP_ERROR => {
            x86_unhandled_exception(frame);
        }
        X86_INT_GP_FAULT => {
            x86_gpf_handler(frame);
        }
        X86_INT_PAGE_FAULT => {
            // TODO: Get actual error code from stack (pushed by CPU for PF)
            if x86_pfe_handler(frame, 0).is_err() {
                x86_fatal_pfe_handler(frame, x86_get_cr2() as u64, 0); // TODO: get actual error code from assembly
            }
        }
        X86_INT_APIC_SPURIOUS => {
            // Ignore spurious interrupts
        }
        X86_INT_APIC_ERROR => {
            apic::apic_error_interrupt_handler();
            apic::apic_issue_eoi();
        }
        X86_INT_APIC_TIMER => {
            apic::apic_timer_interrupt_handler();
            apic::apic_issue_eoi();
        }
        _ => {
            // Handle IRQs
            crate::kernel::arch::amd64::arch::platform_irq(frame);
        }
    }
}

// External functions
extern "C" {
    fn vmm_page_fault_handler(va: VAddr, flags: u32) -> i32;
    fn dispatch_user_exception(kind: u32, context: *const ArchExceptionContext) -> i32;
}

/// Architecture exception context for user-space dispatch
#[repr(C)]
pub struct ArchExceptionContext {
    pub is_page_fault: bool,
    pub frame: *const X86Iframe,
    pub cr2: u64,
}
