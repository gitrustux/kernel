// Copyright 2025 The Rustux Authors
// Copyright (c) 2016 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 syscall support
//!
//! This module provides helper functions for syscall handling.
//! The main syscall entry point is in syscall.S for performance
//! and precise stack/register management.

#![no_std]

use crate::kernel::arch::amd64::debugger;
use crate::kernel::arch::amd64::debugger::X86SyscallGeneralRegs;
use crate::kernel::arch::amd64::X86Iframe;
use crate::kernel::thread;
use crate::rustux::types::*;

/// Syscall return values
const ZX_OK: i32 = 0;

/// Maximum number of syscalls
pub const ZX_SYS_COUNT: u64 = 1000; // Adjust based on actual syscall count

/// Process pending signals for a thread in syscall
///
/// This is called from the syscall path when a thread has a pending signal.
///
/// # Arguments
///
/// * `regs` - Pointer to the syscall general registers
///
/// # Returns
///
/// The new syscall result value to return to user space
///
/// # Safety
///
/// regs must point to valid memory
#[no_mangle]
pub unsafe extern "C" fn x86_syscall_process_pending_signals(
    regs: *mut X86SyscallGeneralRegs,
) -> i32 {
    let current_thread = thread::get_current_thread();

    // Check if we have a signal handler
    // TODO: Implement proper signal handling
    // For now, just return the original syscall result

    // Preserve the syscall result in rax
    let result = (*regs).rax as i32;

    // TODO: Call signal handler if one exists
    // This would involve:
    // 1. Saving current state
    // 2. Setting up signal handler stack frame
    // 3. Returning to user space to run handler
    // 4. Resuming syscall after handler completes

    result
}

/// Handle unknown syscall number
///
/// # Arguments
///
/// * `syscall_num` - The invalid syscall number
///
/// # Returns
///
/// Error code for unknown syscall
#[no_mangle]
pub extern "C" fn unknown_syscall(syscall_num: u64) -> i32 {
    // Log the unknown syscall
    // TODO: println!("Unknown syscall: {}", syscall_num);

    // Return error
    -1 // ZX_ERR_BAD_SYSCALL
}

/// Convert iframe to syscall general registers
///
/// # Arguments
///
/// * `out` - Output syscall general registers
/// * `frame` - Input interrupt frame
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn x86_fill_in_syscall_regs_from_iframe(
    out: *mut X86SyscallGeneralRegs,
    frame: &X86Iframe,
) {
    // On syscall entry, the registers are:
    // rax = syscall number
    // rdi, rsi, rdx, r10, r8, r9 = syscall arguments
    // r12 = arg 7, r13 = arg 8 (for extended syscalls)
    // rcx = user RIP
    // r11 = user RFLAGS

    (*out).rax = frame.rax;
    (*out).rdi = frame.rdi;
    (*out).rsi = frame.rsi;
    (*out).rdx = frame.rdx;
    (*out).r10 = frame.r10;
    (*out).r8 = frame.r8;
    (*out).r9 = frame.r9;
    (*out).r11 = frame.r11;
    (*out).rcx = frame.rcx;
    (*out).rbx = frame.rbx;
    (*out).rbp = frame.rbp;
    (*out).r12 = frame.r12;
    (*out).r13 = frame.r13;
    (*out).r14 = frame.r14;
    (*out).r15 = frame.r15;
    (*out).rsp = frame.user_sp;
    (*out).rip = frame.ip;
    (*out).rflags = frame.flags;
}

/// Convert syscall general registers to iframe
///
/// # Arguments
///
/// * `frame` - Output interrupt frame
/// * `regs` - Input syscall general registers
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn x86_fill_in_iframe_from_syscall_regs(
    frame: &mut X86Iframe,
    regs: &X86SyscallGeneralRegs,
) {
    frame.rax = regs.rax;
    frame.rdi = regs.rdi;
    frame.rsi = regs.rsi;
    frame.rdx = regs.rdx;
    frame.r10 = regs.r10;
    frame.r8 = regs.r8;
    frame.r9 = regs.r9;
    frame.r11 = regs.r11;
    frame.rcx = regs.rcx;
    frame.rbx = regs.rbx;
    frame.rbp = regs.rbp;
    frame.r12 = regs.r12;
    frame.r13 = regs.r13;
    frame.r14 = regs.r14;
    frame.r15 = regs.r15;
    frame.user_sp = regs.rsp;
    frame.ip = regs.rip;

    // Don't allow overriding privileged fields of rflags
    const X86_FLAGS_USER: u64 = 0x3F7FF;
    frame.flags = (frame.flags & !X86_FLAGS_USER) | (regs.rflags & X86_FLAGS_USER);
}

/// Validate syscall number is in range
///
/// # Arguments
///
/// * `syscall_num` - Syscall number to validate
///
/// # Returns
///
/// true if the syscall number is valid
#[inline]
pub fn x86_is_valid_syscall(syscall_num: u64) -> bool {
    syscall_num < ZX_SYS_COUNT
}

/// Get the syscall wrapper function pointer for a syscall number
///
/// # Arguments
///
/// * `syscall_num` - Syscall number
///
/// # Returns
///
/// Function pointer to the syscall wrapper, or None if invalid
///
/// # Safety
///
/// The returned function pointer must only be called with proper
/// syscall argument setup
pub unsafe fn x86_get_syscall_wrapper(syscall_num: u64) -> Option<unsafe extern "C" fn() -> i32> {
    if !x86_is_valid_syscall(syscall_num) {
        return None;
    }

    // The actual syscall wrapper table is defined in assembly
    // in the syscall-kernel-branches.S include
    extern "C" {
        #[link_name = "syscall_wrapper_table"]
        static SYSCALL_WRAPPER_TABLE: [*const (); 1000];
    }

    let func_ptr = SYSCALL_WRAPPER_TABLE.get(syscall_num as usize)?;
    Some(core::mem::transmute(*func_ptr))
}

/// Syscall statistics (for debugging/monitoring)
#[repr(C)]
pub struct SyscallStats {
    pub count: u64,
    pub total_time: u64,
    pub max_time: u64,
}

/// Per-syscall statistics
static mut SYSCALL_STATS: [SyscallStats; 1000] = [SyscallStats {
    count: 0,
    total_time: 0,
    max_time: 0,
}; 1000];

/// Record syscall entry time
///
/// # Returns
///
/// The current TSC value
#[inline]
pub fn x86_syscall_enter() -> u64 {
    crate::arch::amd64::asm::rdtsc()
}

/// Record syscall completion and update statistics
///
/// # Arguments
///
/// * `syscall_num` - The syscall number
/// * `start_time` - The TSC value from syscall entry
#[inline]
pub unsafe fn x86_syscall_exit(syscall_num: u64, start_time: u64) {
    let end_time = crate::arch::amd64::asm::rdtsc();
    let elapsed = end_time.wrapping_sub(start_time);

    if (syscall_num as usize) < SYSCALL_STATS.len() {
        let stats = &mut SYSCALL_STATS[syscall_num as usize];
        stats.count += 1;
        stats.total_time += elapsed;
        if elapsed > stats.max_time {
            stats.max_time = elapsed;
        }
    }
}

/// Get syscall statistics for a syscall
///
/// # Arguments
///
/// * `syscall_num` - The syscall number
///
/// # Returns
///
/// Reference to the syscall stats, or None if invalid
pub unsafe fn x86_get_syscall_stats(syscall_num: u64) -> Option<&'static SyscallStats> {
    if (syscall_num as usize) < SYSCALL_STATS.len() {
        Some(&SYSCALL_STATS[syscall_num as usize])
    } else {
        None
    }
}

/// Test if we're in a syscall
///
/// Checks the CS register to see if we're in kernel mode
/// and came from a syscall (vs an interrupt).
///
/// # Returns
///
/// true if we're processing a syscall
pub fn x86_in_syscall() -> bool {
    unsafe {
        let cs: u64;
        core::arch::asm!("mov {0}, cs", out(reg) cs, options(nostack));

        // Check if we're in kernel CS and came from user mode
        // This is a simplified check - a proper implementation would
        // need to track syscall state more carefully
        cs == 0x08 // Kernel CS
    }
}
