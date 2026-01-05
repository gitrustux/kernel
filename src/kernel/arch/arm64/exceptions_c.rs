// Copyright 2025 The Rustux Authors
// Copyright (c) 2014 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arch_ops;
use crate::arch::arm64;
use crate::arch::arm64::exceptions;
use crate::arch::exception;
use crate::arch::user_copy;

use crate::bits;
use crate::debug;
use core::fmt::Write;

use crate::kernel::interrupt;
use crate::kernel::thread;

use crate::platform;
use crate::trace;
use crate::vm::fault;
use crate::vm;

use crate::lib::counters;
use crate::lib::crashlog;

use crate::rustux::syscalls::exception::*;
use crate::rustux::types::*;

const LOCAL_TRACE: bool = false;

const DFSC_ALIGNMENT_FAULT: u32 = 0b100001;

fn dump_iframe(iframe: &arm64::arm64_iframe_long) {
    println!("iframe {:p}:", iframe);
    println!("x0  {:#18x} x1  {:#18x} x2  {:#18x} x3  {:#18x}", iframe.r[0], iframe.r[1], iframe.r[2], iframe.r[3]);
    println!("x4  {:#18x} x5  {:#18x} x6  {:#18x} x7  {:#18x}", iframe.r[4], iframe.r[5], iframe.r[6], iframe.r[7]);
    println!("x8  {:#18x} x9  {:#18x} x10 {:#18x} x11 {:#18x}", iframe.r[8], iframe.r[9], iframe.r[10], iframe.r[11]);
    println!("x12 {:#18x} x13 {:#18x} x14 {:#18x} x15 {:#18x}", iframe.r[12], iframe.r[13], iframe.r[14], iframe.r[15]);
    println!("x16 {:#18x} x17 {:#18x} x18 {:#18x} x19 {:#18x}", iframe.r[16], iframe.r[17], iframe.r[18], iframe.r[19]);
    println!("x20 {:#18x} x21 {:#18x} x22 {:#18x} x23 {:#18x}", iframe.r[20], iframe.r[21], iframe.r[22], iframe.r[23]);
    println!("x24 {:#18x} x25 {:#18x} x26 {:#18x} x27 {:#18x}", iframe.r[24], iframe.r[25], iframe.r[26], iframe.r[27]);
    println!("x28 {:#18x} x29 {:#18x} lr  {:#18x} usp {:#18x}", iframe.r[28], iframe.r[29], iframe.lr, iframe.usp);
    println!("elr  {:#18x}", iframe.elr);
    println!("spsr {:#18x}", iframe.spsr);
}

counters::KCOUNTER!(
    EXCEPTIONS_BRKPT, "kernel.exceptions.breakpoint");
counters::KCOUNTER!(
    EXCEPTIONS_FPU, "kernel.exceptions.fpu");
counters::KCOUNTER!(
    EXCEPTIONS_PAGE, "kernel.exceptions.page_fault");
counters::KCOUNTER!(
    EXCEPTIONS_IRQ, "kernel.exceptions.irq");
counters::KCOUNTER!(
    EXCEPTIONS_UNHANDLED, "kernel.exceptions.unhandled");
counters::KCOUNTER!(
    EXCEPTIONS_USER, "kernel.exceptions.user");
counters::KCOUNTER!(
    EXCEPTIONS_UNKNOWN, "kernel.exceptions.unknown");

fn try_dispatch_user_data_fault_exception(
    type_: rx_excp_type_t, 
    iframe: &mut arm64::arm64_iframe_long,
    esr: u32, 
    far: u64
) -> rx_status_t {
    let thread = thread::get_current_thread();
    let mut context = exception::arch_exception_context_t {
        frame: iframe as *mut arm64::arm64_iframe_long,
        esr,
        far,
    };
    
    arch_ops::arch_enable_ints();
    debug_assert!(thread.arch.suspended_general_regs.is_null());
    thread.arch.suspended_general_regs = iframe as *mut arm64::arm64_iframe_long;
    let status = exception::dispatch_user_exception(type_, &mut context);
    thread.arch.suspended_general_regs = core::ptr::null_mut();
    arch_ops::arch_disable_ints();
    status
}

fn try_dispatch_user_exception(
    type_: rx_excp_type_t, 
    iframe: &mut arm64::arm64_iframe_long, 
    esr: u32
) -> rx_status_t {
    try_dispatch_user_data_fault_exception(type_, iframe, esr, 0)
}

#[no_return]
fn exception_die(iframe: &mut arm64::arm64_iframe_long, esr: u32) {
    platform::platform_panic_start();

    let ec = bits::BITS_SHIFT(esr, 31, 26);
    let il = bits::BIT(esr, 25);
    let iss = bits::BITS(esr, 24, 0);

    /* fatal exception, die here */
    println!("ESR 0x{:x}: ec 0x{:x}, il 0x{:x}, iss 0x{:x}", esr, ec, il, iss);
    dump_iframe(iframe);
    crashlog::crashlog.iframe = iframe as *mut arm64::arm64_iframe_long;

    platform::platform_halt(platform::HALT_ACTION_HALT, platform::HALT_REASON_SW_PANIC);
    // This never returns
    loop {}
}

fn arm64_unknown_handler(iframe: &mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    /* this is for a lot of reasons, but most of them are undefined instructions */
    if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0) {
        /* trapped inside the kernel, this is bad */
        println!("unknown exception in kernel: PC at {:#x}", iframe.elr);
        exception_die(iframe, esr);
    }
    let _ = try_dispatch_user_exception(RX_EXCP_UNDEFINED_INSTRUCTION, iframe, esr);
}

fn arm64_brk_handler(iframe: &mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0) {
        /* trapped inside the kernel, this is bad */
        println!("BRK in kernel: PC at {:#x}", iframe.elr);
        exception_die(iframe, esr);
    }
    let _ = try_dispatch_user_exception(RX_EXCP_SW_BREAKPOINT, iframe, esr);
}

fn arm64_hw_breakpoint_handler(iframe: &mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0) {
        /* trapped inside the kernel, this is bad */
        println!("HW breakpoint in kernel: PC at {:#x}", iframe.elr);
        exception_die(iframe, esr);
    }

    // We don't need to save the debug state because it doesn't change by an exception. The only
    // way to change the debug state is through the thread write syscall.

    // NOTE: ARM64 Doesn't provide a good way to communicate exception status (without exposing ESR
    //       to userspace). This means a debugger will have to compare the registers with the PC
    //       on the exceptions to find out which breakpoint triggered the exception.
    let _ = try_dispatch_user_exception(RX_EXCP_HW_BREAKPOINT, iframe, esr);
}

fn arm64_step_handler(iframe: &mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0) {
        /* trapped inside the kernel, this is bad */
        println!("software step in kernel: PC at {:#x}", iframe.elr);
        exception_die(iframe, esr);
    }
    // TODO(RX-3037): Is it worth separating this into two separate exceptions?
    let _ = try_dispatch_user_exception(RX_EXCP_HW_BREAKPOINT, iframe, esr);
}

fn arm64_fpu_handler(iframe: &mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0) {
        /* we trapped a floating point instruction inside our own EL, this is bad */
        println!("invalid fpu use in kernel: PC at {:#x}", iframe.elr);
        exception_die(iframe, esr);
    }
    unsafe {
        arm64::arm64_fpu_exception(iframe, exception_flags);
    }
}

fn arm64_instruction_abort_handler(iframe: &mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    /* read the FAR register */
    let far = unsafe {
        let far: u64;
        core::arch::asm!("mrs {}, far_el1", out(reg) far);
        far
    };
    
    let ec = bits::BITS_SHIFT(esr, 31, 26);
    let iss = bits::BITS(esr, 24, 0);
    let is_user = !bits::BIT(ec, 0);

    let mut pf_flags = vm::VMM_PF_FLAG_INSTRUCTION;
    pf_flags |= if is_user { vm::VMM_PF_FLAG_USER } else { 0 };
    
    /* Check if this was not permission fault */
    if (iss & 0b111100) != 0b001100 {
        pf_flags |= vm::VMM_PF_FLAG_NOT_PRESENT;
    }

    trace::LTRACEF!("instruction abort: PC at {:#x}, is_user {}, FAR {:#x}, esr 0x{:x}, iss 0x{:x}",
            iframe.elr, is_user, far, esr, iss);

    arch_ops::arch_enable_ints();
    EXCEPTIONS_PAGE.add(1);
    interrupt::CPU_STATS_INC!(page_faults);
    
    let err = fault::vmm_page_fault_handler(far, pf_flags);
    
    arch_ops::arch_disable_ints();
    
    if err >= 0 {
        return;
    }

    // If this is from user space, let the user exception handler
    // get a shot at it.
    if is_user {
        EXCEPTIONS_USER.add(1);
        if try_dispatch_user_data_fault_exception(RX_EXCP_FATAL_PAGE_FAULT, iframe, esr, far) == RX_OK {
            return;
        }
    }

    println!("instruction abort: PC at {:#x}, is_user {}, FAR {:#x}", iframe.elr, is_user, far);
    exception_die(iframe, esr);
}

fn arm64_data_abort_handler(iframe: &mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    /* read the FAR register */
    let far = unsafe {
        let far: u64;
        core::arch::asm!("mrs {}, far_el1", out(reg) far);
        far
    };
    
    let ec = bits::BITS_SHIFT(esr, 31, 26);
    let iss = bits::BITS(esr, 24, 0);
    let is_user = !bits::BIT(ec, 0);
    let WnR = bits::BIT(iss, 6); // Write not Read
    let CM = bits::BIT(iss, 8);  // cache maintenance op

    let mut pf_flags = 0;
    // if it was marked Write but the cache maintenance bit was set, treat it as read
    pf_flags |= if WnR && !CM { vm::VMM_PF_FLAG_WRITE } else { 0 };
    pf_flags |= if is_user { vm::VMM_PF_FLAG_USER } else { 0 };
    
    /* Check if this was not permission fault */
    if (iss & 0b111100) != 0b001100 {
        pf_flags |= vm::VMM_PF_FLAG_NOT_PRESENT;
    }

    trace::LTRACEF!("data fault: PC at {:#x}, is_user {}, FAR {:#x}, esr 0x{:x}, iss 0x{:x}",
            iframe.elr, is_user, far, esr, iss);

    let dfsc = bits::BITS(iss, 5, 0);
    
    if likely(dfsc != DFSC_ALIGNMENT_FAULT) {
        arch_ops::arch_enable_ints();
        EXCEPTIONS_PAGE.add(1);
        
        let err = fault::vmm_page_fault_handler(far, pf_flags);
        
        arch_ops::arch_disable_ints();
        
        if err >= 0 {
            return;
        }
    }

    // Check if the current thread was expecting a data fault and
    // we should return to its handler.
    let thr = thread::get_current_thread();
    if !thr.arch.data_fault_resume.is_null() && vm::is_user_address(far) {
        iframe.elr = thr.arch.data_fault_resume as usize;
        return;
    }

    // If this is from user space, let the user exception handler
    // get a shot at it.
    if is_user {
        EXCEPTIONS_USER.add(1);
        let excp_type = if unlikely(dfsc == DFSC_ALIGNMENT_FAULT) {
            RX_EXCP_UNALIGNED_ACCESS
        } else {
            RX_EXCP_FATAL_PAGE_FAULT
        };
        
        if try_dispatch_user_data_fault_exception(excp_type, iframe, esr, far) == RX_OK {
            return;
        }
    }

    /* decode the iss */
    if bits::BIT(iss, 24) != 0 { /* ISV bit */
        println!("data fault: PC at {:#x}, FAR {:#x}, iss {:#x} (DFSC {:#x})",
               iframe.elr, far, iss, bits::BITS(iss, 5, 0));
    } else {
        println!("data fault: PC at {:#x}, FAR {:#x}, iss 0x{:x}",
               iframe.elr, far, iss);
    }

    exception_die(iframe, esr);
}

#[inline]
unsafe fn arm64_restore_percpu_pointer() {
    arm64::arm64_write_percpu_ptr(thread::get_current_thread().arch.current_percpu_ptr);
}

/* called from assembly */
#[no_mangle]
pub extern "C" fn arm64_sync_exception(
    iframe: *mut arm64::arm64_iframe_long, exception_flags: u32, esr: u32) {
    let iframe = unsafe { &mut *iframe };
    let ec = bits::BITS_SHIFT(esr, 31, 26);

    if (exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) != 0 {
        // if we came from a lower level, restore the per cpu pointer
        unsafe { arm64_restore_percpu_pointer(); }
    }

    match ec {
        0b000000 => { /* unknown reason */
            EXCEPTIONS_UNKNOWN.add(1);
            arm64_unknown_handler(iframe, exception_flags, esr);
        },
        0b111000 | 0b111100 => { /* BRK from arm32 or arm64 */
            EXCEPTIONS_BRKPT.add(1);
            arm64_brk_handler(iframe, exception_flags, esr);
        },
        0b000111 => { /* floating point */
            EXCEPTIONS_FPU.add(1);
            arm64_fpu_handler(iframe, exception_flags, esr);
        },
        0b010001 | 0b010101 => { /* syscall from arm32 or arm64 */
            println!("syscalls should be handled in assembly");
            exception_die(iframe, esr);
        },
        0b100000 | 0b100001 => { /* instruction abort from lower level or same level */
            arm64_instruction_abort_handler(iframe, exception_flags, esr);
        },
        0b100100 | 0b100101 => { /* data abort from lower level or same level */
            arm64_data_abort_handler(iframe, exception_flags, esr);
        },
        0b110000 | 0b110001 => { /* HW breakpoint from a lower level or same level */
            arm64_hw_breakpoint_handler(iframe, exception_flags, esr);
        },
        0b110010 | 0b110011 => { /* software step from lower level or same level */
            arm64_step_handler(iframe, exception_flags, esr);
        },
        _ => {
            /* TODO: properly decode more of these */
            if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0) {
                /* trapped inside the kernel, this is bad */
                println!("unhandled exception in kernel: PC at {:#x}", iframe.elr);
                exception_die(iframe, esr);
            }
            /* let the user exception handler get a shot at it */
            EXCEPTIONS_UNHANDLED.add(1);
            if try_dispatch_user_exception(RX_EXCP_GENERAL, iframe, esr) == RX_OK {
                // Handled successfully
            } else {
                println!("unhandled synchronous exception");
                exception_die(iframe, esr);
            }
        }
    }

    /* if we came from user space, check to see if we have any signals to handle */
    if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) != 0) {
        /* in the case of receiving a kill signal, this function may not return,
         * but the scheduler would have been invoked so it's fine.
         */
        unsafe {
            arm64_thread_process_pending_signals(iframe);
        }
    }

    /* if we're returning to kernel space, make sure we restore the correct x18 */
    if (exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0 {
        iframe.r[18] = unsafe { arm64::arm64_read_percpu_ptr() as u64 };
    }
}

/* called from assembly */
#[no_mangle]
pub extern "C" fn arm64_irq(iframe: *mut arm64::arm64_iframe_short, exception_flags: u32) -> u32 {
    if (exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) != 0 {
        // if we came from a lower level, restore the per cpu pointer
        unsafe { arm64_restore_percpu_pointer(); }
    }

    trace::LTRACEF!("iframe {:p}, flags 0x{:x}", iframe, exception_flags);

    let mut state = interrupt::int_handler_saved_state_t::default();
    interrupt::int_handler_start(&mut state);

    EXCEPTIONS_IRQ.add(1);
    unsafe {
        platform::platform_irq(iframe);
    }

    let do_preempt = interrupt::int_handler_finish(&state);

    /* if we came from user space, check to see if we have any signals to handle */
    if unlikely((exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) != 0) {
        let mut exit_flags = 0;
        if thread::thread_is_signaled(thread::get_current_thread()) {
            exit_flags |= arm64::ARM64_IRQ_EXIT_THREAD_SIGNALED;
        }
        if do_preempt {
            exit_flags |= arm64::ARM64_IRQ_EXIT_RESCHEDULE;
        }
        return exit_flags;
    }

    /* preempt the thread if the interrupt has signaled it */
    if do_preempt {
        thread::thread_preempt();
    }

    /* if we're returning to kernel space, make sure we restore the correct x18 */
    if (exception_flags & arm64::ARM64_EXCEPTION_FLAG_LOWER_EL) == 0 {
        unsafe {
            let iframe_ref = &mut *iframe;
            iframe_ref.r[18] = arm64::arm64_read_percpu_ptr() as u64;
        }
    }

    0
}

/* called from assembly */
#[no_mangle]
pub extern "C" fn arm64_finish_user_irq(exit_flags: u32, iframe: *mut arm64::arm64_iframe_long) {
    // we came from a lower level, so restore the per cpu pointer
    unsafe { arm64_restore_percpu_pointer(); }

    /* in the case of receiving a kill signal, this function may not return,
     * but the scheduler would have been invoked so it's fine.
     */
    if unlikely((exit_flags & arm64::ARM64_IRQ_EXIT_THREAD_SIGNALED) != 0) {
        debug_assert!(!iframe.is_null());
        unsafe {
            arm64_thread_process_pending_signals(iframe);
        }
    }

    /* preempt the thread if the interrupt has signaled it */
    if (exit_flags & arm64::ARM64_IRQ_EXIT_RESCHEDULE) != 0 {
        thread::thread_preempt();
    }
}

/* called from assembly */
#[no_mangle]
pub extern "C" fn arm64_invalid_exception(iframe: *mut arm64::arm64_iframe_long, which: u32) {
    // restore the percpu pointer (x18) unconditionally
    unsafe { arm64_restore_percpu_pointer(); }

    println!("invalid exception, which 0x{:x}", which);
    unsafe {
        dump_iframe(&*iframe);
    }

    platform::platform_halt(platform::HALT_ACTION_HALT, platform::HALT_REASON_SW_PANIC);
}

/* called from assembly */
#[no_mangle]
pub extern "C" fn arm64_thread_process_pending_signals(iframe: *mut arm64::arm64_iframe_long) {
    let thread = thread::get_current_thread();
    debug_assert!(!iframe.is_null());
    debug_assert!(thread.arch.suspended_general_regs.is_null());

    thread.arch.suspended_general_regs = iframe;
    thread::thread_process_pending_signals();
    thread.arch.suspended_general_regs = core::ptr::null_mut();
}

pub fn arch_dump_exception_context(context: &exception::arch_exception_context_t) {
    let ec = bits::BITS_SHIFT(context.esr, 31, 26);
    let iss = bits::BITS(context.esr, 24, 0);
    let iframe = unsafe { &*context.frame };

    match ec {
        0b100000 | 0b100001 => { /* instruction abort from lower level or same level */
            println!("instruction abort: PC at {:#x}, address {:#x} IFSC {:#x} {}",
                   iframe.elr, context.far,
                   bits::BITS(context.esr, 5, 0),
                   if bits::BIT(ec, 0) != 0 { "" } else { "user " });
        },
        0b100100 | 0b100101 => { /* data abort from lower level or same level */
            println!("data abort: PC at {:#x}, address {:#x} {}{}",
                   iframe.elr, context.far,
                   if bits::BIT(ec, 0) != 0 { "" } else { "user " },
                   if bits::BIT(iss, 6) != 0 { "write" } else { "read" });
        },
        _ => {}
    }

    dump_iframe(iframe);

    // try to dump the user stack
    if vm::is_user_address(iframe.usp) {
        let mut buf = [0u8; 256];
        if unsafe { user_copy::arch_copy_from_user(buf.as_mut_ptr(), iframe.usp as *const u8, buf.len()) } == RX_OK {
            println!("bottom of user stack at 0x{:x}:", iframe.usp);
            debug::hexdump_ex(&buf, buf.len(), iframe.usp as usize);
        }
    }
}

pub fn arch_fill_in_exception_context(arch_context: &exception::arch_exception_context_t, report: &mut rx_exception_report_t) {
    let rx_context = &mut report.context;

    rx_context.arch.u.arm_64.esr = arch_context.esr;

    // If there was a fatal page fault, fill in the address that caused the fault.
    if RX_EXCP_FATAL_PAGE_FAULT == report.header.type_ {
        rx_context.arch.u.arm_64.far = arch_context.far;
    } else {
        rx_context.arch.u.arm_64.far = 0;
    }
}

pub fn arch_dispatch_user_policy_exception() -> rx_status_t {
    let mut frame = arm64::arm64_iframe_long::default();
    let mut context = exception::arch_exception_context_t {
        frame: &mut frame,
        esr: 0,
        far: 0,
    };
    exception::dispatch_user_exception(RX_EXCP_POLICY_ERROR, &mut context)
}

// Helper for macros
#[inline(always)]
fn unlikely(b: bool) -> bool {
    b
}

#[inline(always)]
fn likely(b: bool) -> bool {
    b
}