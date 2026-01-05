// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V debugger support
//!
//! This module provides functions for reading and writing thread state
//! for debugging purposes, including general-purpose registers.

#![no_std]

use crate::arch::riscv64::RiscvIframe;
use crate::kernel::thread;
use crate::rustux::types::*;

/// User-accessible flags in SSTATUS
const X86_FLAGS_USER: u64 = 0x3F7FF; // Note: This is named for x86 compatibility

/// Thread general register state (for userspace)
#[repr(C)]
pub struct RiscvThreadStateGeneralRegs {
    pub ra: u64,   // x1
    pub sp: u64,   // x2
    pub gp: u64,   // x3
    pub tp: u64,   // x4
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
    pub pc: u64,
    pub status: u64,
}

/// Convert iframe to thread general registers
///
/// # Arguments
///
/// * `out` - Output thread general registers
/// * `input` - Input interrupt frame
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn riscv_fill_in_gregs_from_iframe(
    out: *mut RiscvThreadStateGeneralRegs,
    input: &RiscvIframe,
) {
    (*out).ra = input.ra;
    (*out).sp = input.sp;
    (*out).gp = input.gp;
    (*out).tp = input.tp;
    (*out).t0 = input.t0;
    (*out).t1 = input.t1;
    (*out).t2 = input.t2;
    (*out).s0 = input.s0;
    (*out).s1 = input.s1;
    (*out).a0 = input.a0;
    (*out).a1 = input.a1;
    (*out).a2 = input.a2;
    (*out).a3 = input.a3;
    (*out).a4 = input.a4;
    (*out).a5 = input.a5;
    (*out).a6 = input.a6;
    (*out).a7 = input.a7;
    (*out).s2 = input.s2;
    (*out).s3 = input.s3;
    (*out).s4 = input.s4;
    (*out).s5 = input.s5;
    (*out).s6 = input.s6;
    (*out).s7 = input.s7;
    (*out).s8 = input.s8;
    (*out).s9 = input.s9;
    (*out).s10 = input.s10;
    (*out).s11 = input.s11;
    (*out).t3 = input.t3;
    (*out).t4 = input.t4;
    (*out).t5 = input.t5;
    (*out).t6 = input.t6;
    (*out).pc = input.pc;
    (*out).status = input.status;
}

/// Convert thread general registers to iframe
///
/// # Arguments
///
/// * `out` - Output interrupt frame
/// * `input` - Input thread general registers
///
/// # Safety
///
/// Both pointers must be valid
pub unsafe fn riscv_fill_in_iframe_from_gregs(
    out: &mut RiscvIframe,
    input: &RiscvThreadStateGeneralRegs,
) {
    out.ra = input.ra;
    out.sp = input.sp;
    out.gp = input.gp;
    out.tp = input.tp;
    out.t0 = input.t0;
    out.t1 = input.t1;
    out.t2 = input.t2;
    out.s0 = input.s0;
    out.s1 = input.s1;
    out.a0 = input.a0;
    out.a1 = input.a1;
    out.a2 = input.a2;
    out.a3 = input.a3;
    out.a4 = input.a4;
    out.a5 = input.a5;
    out.a6 = input.a6;
    out.a7 = input.a7;
    out.s2 = input.s2;
    out.s3 = input.s3;
    out.s4 = input.s4;
    out.s5 = input.s5;
    out.s6 = input.s6;
    out.s7 = input.s7;
    out.s8 = input.s8;
    out.s9 = input.s9;
    out.s10 = input.s10;
    out.s11 = input.s11;
    out.t3 = input.t3;
    out.t4 = input.t4;
    out.t5 = input.t5;
    out.t6 = input.t6;
    out.pc = input.pc;

    // Don't allow overriding privileged fields of status
    // Keep SPIE and SPP bits from original
    const STATUS_PRIV_BITS: u64 = (1 << 5) | (1 << 8); // SPIE | SPP
    out.status = (out.status & STATUS_PRIV_BITS) | (input.status & !STATUS_PRIV_BITS);
}

/// Thread vector register state
///
/// RISC-V vector extension is optional, so this is a placeholder
/// for future support when the V extension is more widely available.
#[repr(C)]
pub struct RiscvThreadStateVectorRegs {
    pub vlen: u32,  // Vector length in bits
    pub vl: u32,    // Current vector length
    pub vtype: u32, // Current vector type
    pub vstart: u64, // Vector start index
    pub vdata: [u8; 8192], // Vector register data (placeholder size)
}

/// Get or set vector registers
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
/// Thread must be valid and regs must point to valid memory
pub unsafe fn riscv_get_set_vector_regs(
    _thread: &mut Thread,
    _regs: *mut RiscvThreadStateVectorRegs,
    _access: RegAccess,
) -> i32 {
    // TODO: Implement vector register support when V extension is available
    // For now, return error as not supported
    -1
}

/// Register access direction
#[repr(i32)]
pub enum RegAccess {
    Get = 0,
    Set = 1,
}

/// Read debug state (currently stub for RISC-V)
///
/// RISC-V doesn't have the same debug register architecture as x86.
/// Debugging is typically done through external debuggers (OpenOCD, GDB)
/// using the JTAG/Debug module.
///
/// # Arguments
///
/// * `debug_state` - Debug state to read into
///
/// # Safety
///
/// debug_state must be valid
pub unsafe fn riscv_read_debug_state(debug_state: &mut RiscvDebugState) {
    debug_state.dpc = riscv_read_dpc();
    debug_state.dcsr = riscv_read_dcsr();
}

/// Write debug state (currently stub for RISC-V)
///
/// # Arguments
///
/// * `debug_state` - Debug state to write
///
/// # Safety
///
/// debug_state must be valid
pub unsafe fn riscv_write_debug_state(debug_state: &RiscvDebugState) {
    riscv_write_dpc(debug_state.dpc);
    riscv_write_dcsr(debug_state.dcsr);
}

/// Disable debug state
///
/// # Safety
///
/// Disables hardware breakpoints by clearing DCSR
pub unsafe fn riscv_disable_debug_state() {
    // Clear DCSR to disable debugging
    riscv_write_dcsr(0);
}

/// RISC-V debug state
#[repr(C)]
pub struct RiscvDebugState {
    pub dpc: u64,   // Debug PC
    pub dcsr: u64,  // Debug Control and Status
}

/// Read DPC (Debug Program Counter) CSR
#[inline]
pub unsafe fn riscv_read_dpc() -> u64 {
    let value: u64;
    core::arch::asm!("csrr {0}, 0x7b1", out(reg) value); // dpc = 0x7b1
    value
}

/// Write DPC CSR
#[inline]
pub unsafe fn riscv_write_dpc(value: u64) {
    core::arch::asm!("csrw 0x7b1, {0}", in(reg) value);
}

/// Read DCSR (Debug Control and Status) CSR
#[inline]
pub unsafe fn riscv_read_dcsr() -> u64 {
    let value: u64;
    core::arch::asm!("csrr {0}, 0x7b0", out(reg) value); // dcsr = 0x7b0
    value
}

/// Write DCSR CSR
#[inline]
pub unsafe fn riscv_write_dcsr(value: u64) {
    core::arch::asm!("csrw 0x7b0, {0}", in(reg) value);
}

// Type aliases for compatibility
pub type zx_thread_state_general_regs_t = RiscvThreadStateGeneralRegs;
pub type zx_thread_state_vector_regs_t = RiscvThreadStateVectorRegs;

// Compile-time checks for structure compatibility
const _: () = assert!(
    core::mem::size_of::<RiscvThreadStateGeneralRegs>() == 272,
    "general regs must be 272 bytes"
);
