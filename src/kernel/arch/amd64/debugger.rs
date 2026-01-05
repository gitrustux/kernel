// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Debugger support
//!
//! This module provides functions for reading and writing thread state
//! for debugging purposes, including general-purpose registers and
//! vector registers (SSE, AVX, etc.).

#![no_std]

use crate::kernel::arch::amd64::registers::*;
use crate::kernel::arch::amd64::X86Iframe;
use crate::kernel::thread::Thread;
use crate::rustux::types::*;
use core::mem;

/// User-accessible flags in RFLAGS
const X86_FLAGS_USER: u64 = 0x3F7FF;

/// Convert syscall general registers to thread general registers
///
/// # Arguments
///
/// * `out` - Output thread general registers
/// * `in` - Input syscall general registers
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn x86_fill_in_gregs_from_syscall(
    out: *mut X86SyscallGeneralRegs,
    input: *const X86SyscallGeneralRegs,
) {
    core::ptr::copy_nonoverlapping(input, out, 1);
}

/// Convert thread general registers to syscall general registers
///
/// # Arguments
///
/// * `out` - Output syscall general registers
/// * `in` - Input thread general registers
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn x86_fill_in_syscall_from_gregs(
    out: *mut X86SyscallGeneralRegs,
    input: *const X86SyscallGeneralRegs,
) {
    // Don't allow overriding privileged fields of rflags
    let orig_rflags = (*out).rflags;
    core::ptr::copy_nonoverlapping(input, out, 1);
    (*out).rflags = (orig_rflags & !X86_FLAGS_USER) | ((*input).rflags & X86_FLAGS_USER);
}

/// Convert iframe to thread general registers
///
/// # Arguments
///
/// * `out` - Output thread general registers
/// * `in` - Input interrupt frame
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn x86_fill_in_gregs_from_iframe(
    out: *mut X86ThreadStateGeneralRegs,
    input: &X86Iframe,
) {
    (*out).rax = input.rax;
    (*out).rbx = input.rbx;
    (*out).rcx = input.rcx;
    (*out).rdx = input.rdx;
    (*out).rsi = input.rsi;
    (*out).rdi = input.rdi;
    (*out).rbp = input.rbp;
    (*out).r8 = input.r8;
    (*out).r9 = input.r9;
    (*out).r10 = input.r10;
    (*out).r11 = input.r11;
    (*out).r12 = input.r12;
    (*out).r13 = input.r13;
    (*out).r14 = input.r14;
    (*out).r15 = input.r15;
    (*out).rsp = input.rsp;
    (*out).rip = input.rip;
    (*out).rflags = input.rflags;
}

/// Convert thread general registers to iframe
///
/// # Arguments
///
/// * `out` - Output interrupt frame
/// * `in` - Input thread general registers
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn x86_fill_in_iframe_from_gregs(
    out: &mut X86Iframe,
    input: &X86ThreadStateGeneralRegs,
) {
    out.rax = input.rax;
    out.rbx = input.rbx;
    out.rcx = input.rcx;
    out.rdx = input.rdx;
    out.rsi = input.rsi;
    out.rdi = input.rdi;
    out.rbp = input.rbp;
    out.r8 = input.r8;
    out.r9 = input.r9;
    out.r10 = input.r10;
    out.r11 = input.r11;
    out.r12 = input.r12;
    out.r13 = input.r13;
    out.r14 = input.r14;
    out.r15 = input.r15;
    out.rsp = input.rsp;
    out.rip = input.rip;

    // Don't allow overriding privileged fields of rflags
    out.rflags = (out.rflags & !X86_FLAGS_USER) | (input.rflags & X86_FLAGS_USER);
}

/// XSAVE state indices
const X86_XSAVE_STATE_INDEX_SSE: u32 = 0;
const X86_XSAVE_STATE_INDEX_AVX: u32 = 2;
const X86_XSAVE_STATE_INDEX_MPX: u32 = 3;  // Memory Protection Keys
const X86_XSAVE_STATE_INDEX_AVX512: u32 = 5;  // AVX-512
const X86_XSAVE_STATE_INDEX_PKRU: u32 = 9;   // PKRU register

/// Extended register state component for SSE (legacy area)
#[repr(C)]
pub struct X86XsaveLegacyArea {
    pub xmm: [X86Mmx; 16],  // XMM registers 0-15
    pub mxcsr: u32,           // MXCSR register
    pub mxcsr_mask: u32,      // MXCSR mask
}

/// 128-bit MMX/XMM register
#[repr(C)]
#[derive(Clone, Copy)]
pub struct X86Mmx {
    pub data: [u8; 16],
}

/// Get or set vector registers (SSE, AVX, AVX-512, MPX, etc.)
///
/// # Arguments
///
/// * `thread` - Thread to read/write
/// * `regs` - Vector register state
/// * `access` - Whether to get or set the registers
///
/// # Returns
///
/// Status code: 0 for success, negative for error
///
/// # Safety
///
/// Thread must be valid andregs must point to valid memory
pub unsafe fn x86_get_set_vector_regs(
    thread: &mut Thread,
    regs: *mut X86ThreadStateVectorRegs,
    access: RegAccess,
) -> i32 {
    // TODO: Implement full x86_get_set_vector_regs
    // For now, return success as a stub
    0
}

/// Register access direction
#[repr(i32)]
pub enum RegAccess {
    Get = 0,
    Set = 1,
}

/// Read debug register state
///
/// # Arguments
///
/// * `debug_state` - Debug state to read into
///
/// # Safety
///
/// debug_state must be valid
pub unsafe fn x86_read_debug_status(debug_state: &mut X86DebugState) {
    // DR6 is the debug status register
    debug_state.dr6 = x86_get_dr6();
}

/// Write hardware debug registers
///
/// # Arguments
///
/// * `debug_state` - Debug state to write
///
/// # Safety
///
/// debug_state must be valid
pub unsafe fn x86_write_hw_debug_regs(debug_state: &X86DebugState) {
    // Write DR0-DR3 (breakpoint registers)
    let dr_regs = [debug_state.dr0, debug_state.dr1, debug_state.dr2, debug_state.dr3];
    for i in 0..4usize {
        x86_set_dr(i as u32, dr_regs[i]);
    }

    // Write DR7 (debug control register)
    x86_set_dr7(debug_state.dr7);
}

/// Disable debug state
///
/// # Safety
///
/// Disables all hardware breakpoints by clearing DR7
pub unsafe fn x86_disable_debug_state() {
    // Clear DR7 to disable all breakpoints
    x86_set_dr7(0);
}

// External register access functions
extern "C" {
    fn x86_get_extended_register_state_component(
        state: *mut core::ffi::c_void,
        index: u32,
        get: bool,
        size_out: *mut u32,
    ) -> *mut core::ffi::c_void;

    fn x86_get_dr6() -> u64;
    fn x86_set_dr(index: u32, value: u64);
    fn x86_set_dr7(value: u64);
}

/// Thread general register state (syscalls)
#[repr(C)]
pub struct X86SyscallGeneralRegs {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

/// Thread general register state (for userspace)
#[repr(C)]
pub struct X86ThreadStateGeneralRegs {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

/// Thread vector register state
#[repr(C)]
pub struct X86ThreadStateVectorRegs {
    pub zmm: [X86Zmm; 32],  // ZMM registers (includes XMM, YMM)
    pub mxcsr: u32,
}

/// 512-bit ZMM register (includes XMM and YMM portions)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct X86Zmm {
    pub data: [u8; 64],
}

// Type aliases for compatibility
pub type zx_thread_state_general_regs_t = X86ThreadStateGeneralRegs;
pub type zx_thread_state_vector_regs_t = X86ThreadStateVectorRegs;
pub type x86_syscall_general_regs_t = X86SyscallGeneralRegs;

// Compile-time checks for structure compatibility
const _: () = assert!(
    mem::size_of::<X86SyscallGeneralRegs>() == mem::size_of::<X86ThreadStateGeneralRegs>(),
    "syscall and thread gregs must match"
);
