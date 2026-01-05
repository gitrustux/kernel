// Copyright 2025 The RISC-V Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V Floating Point Unit (FPU) support
//!
//! This module provides functions for saving and restoring
//! floating point state during context switches.

#![no_std]

use crate::arch::riscv64::registers;
use crate::arch::riscv64::registers::csr;
use crate::rustux::types::*;

/// FPU register state
///
/// RISC-V supports single-precision (32-bit) and double-precision (64-bit)
/// floating point via the F extension. Each register is 64 bits (F/D registers).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FpuState {
    /// 32 floating point registers (f0-f31)
    /// Each is 64 bits to accommodate both single and double precision
    pub fpregs: [u64; 32],
    /// FCSR (Floating-point Control and Status Register)
    pub fcsr: u32,
    /// Padding to align to 16 bytes
    _padding: u32,
}

impl FpuState {
    /// Create a new zero-initialized FPU state
    pub const fn new() -> Self {
        Self {
            fpregs: [0; 32],
            fcsr: 0,
            _padding: 0,
        }
    }
}

impl Default for FpuState {
    fn default() -> Self {
        Self::new()
    }
}

/// FS (Floating-point Status) bits in SSTATUS
pub const FS_OFF: u64 = 0x0 << 13;
pub const FS_INITIAL: u64 = 0x1 << 13;
pub const FS_CLEAN: u64 = 0x2 << 13;
pub const FS_DIRTY: u64 = 0x3 << 13;

/// Initialize the FPU for the current hart
///
/// Enables the FPU by setting the FS field to Initial in SSTATUS
pub fn riscv_fpu_init() {
    unsafe {
        // Set FS to Initial to enable FPU access
        let mut sstatus = registers::read_csr(csr::SSTATUS);
        sstatus &= !(0x3 << 13); // Clear FS field
        sstatus |= FS_INITIAL;   // Set to Initial
        registers::write_csr(csr::SSTATUS, sstatus);
    }
}

/// Save the current FPU state
///
/// # Arguments
///
/// * `state` - Pointer to FPU state to save to
///
/// # Safety
///
/// state must point to valid memory
pub unsafe fn riscv_fpu_save(state: *mut FpuState) {
    // Save all 32 FP registers
    // Using assembly to ensure we get the right registers
    core::arch::asm!(
        // Save f0-f7
        "fsd f0, 0({0})",
        "fsd f1, 8({0})",
        "fsd f2, 16({0})",
        "fsd f3, 24({0})",
        "fsd f4, 32({0})",
        "fsd f5, 40({0})",
        "fsd f6, 48({0})",
        "fsd f7, 56({0})",
        // Save f8-f15
        "fsd f8, 64({0})",
        "fsd f9, 72({0})",
        "fsd f10, 80({0})",
        "fsd f11, 88({0})",
        "fsd f12, 96({0})",
        "fsd f13, 104({0})",
        "fsd f14, 112({0})",
        "fsd f15, 120({0})",
        // Save f16-f23
        "fsd f16, 128({0})",
        "fsd f17, 136({0})",
        "fsd f18, 144({0})",
        "fsd f19, 152({0})",
        "fsd f20, 160({0})",
        "fsd f21, 168({0})",
        "fsd f22, 176({0})",
        "fsd f23, 184({0})",
        // Save f24-f31
        "fsd f24, 192({0})",
        "fsd f25, 200({0})",
        "fsd f26, 208({0})",
        "fsd f27, 216({0})",
        "fsd f28, 224({0})",
        "fsd f29, 232({0})",
        "fsd f30, 240({0})",
        "fsd f31, 248({0})",
        in(reg) state,
        options(nostack)
    );

    // Save FCSR
    let fcsr: u32;
    core::arch::asm!(
        "frcsr {0}",
        out(reg) fcsr,
        options(nostack)
    );
    (*state).fcsr = fcsr;

    // Mark FPU as clean in SSTATUS
    let mut sstatus = registers::read_csr(csr::SSTATUS);
    sstatus &= !(0x3 << 13); // Clear FS field
    sstatus |= FS_CLEAN;     // Set to Clean
    registers::write_csr(csr::SSTATUS, sstatus);
}

/// Restore FPU state
///
/// # Arguments
///
/// * `state` - Pointer to FPU state to restore from
///
/// # Safety
///
/// state must point to valid memory
pub unsafe fn riscv_fpu_restore(state: *const FpuState) {
    // Restore all 32 FP registers
    core::arch::asm!(
        // Restore f0-f7
        "fld f0, 0({0})",
        "fld f1, 8({0})",
        "fld f2, 16({0})",
        "fld f3, 24({0})",
        "fld f4, 32({0})",
        "fld f5, 40({0})",
        "fld f6, 48({0})",
        "fld f7, 56({0})",
        // Restore f8-f15
        "fld f8, 64({0})",
        "fld f9, 72({0})",
        "fld f10, 80({0})",
        "fld f11, 88({0})",
        "fld f12, 96({0})",
        "fld f13, 104({0})",
        "fld f14, 112({0})",
        "fld f15, 120({0})",
        // Restore f16-f23
        "fld f16, 128({0})",
        "fld f17, 136({0})",
        "fld f18, 144({0})",
        "fld f19, 152({0})",
        "fld f20, 160({0})",
        "fld f21, 168({0})",
        "fld f22, 176({0})",
        "fld f23, 184({0})",
        // Restore f24-f31
        "fld f24, 192({0})",
        "fld f25, 200({0})",
        "fld f26, 208({0})",
        "fld f27, 216({0})",
        "fld f28, 224({0})",
        "fld f29, 232({0})",
        "fld f30, 240({0})",
        "fld f31, 248({0})",
        in(reg) state,
        options(nostack)
    );

    // Restore FCSR
    core::arch::asm!(
        "fscsr {0}",
        in(reg) (*state).fcsr,
        options(nostack)
    );

    // Mark FPU as clean in SSTATUS
    let mut sstatus = registers::read_csr(csr::SSTATUS);
    sstatus &= !(0x3 << 13); // Clear FS field
    sstatus |= FS_CLEAN;     // Set to Clean
    registers::write_csr(csr::SSTATUS, sstatus);
}

/// Zero the FPU state
///
/// # Arguments
///
/// * `state` - Pointer to FPU state to zero
///
/// # Safety
///
/// state must point to valid memory
pub unsafe fn riscv_fpu_zero(state: *mut FpuState) {
    (*state) = FpuState::new();
}

/// Check if FPU is enabled
///
/// # Returns
///
/// true if FPU access is enabled
pub fn riscv_fpu_enabled() -> bool {
    unsafe {
        let sstatus = registers::read_csr(csr::SSTATUS);
        let fs = (sstatus >> 13) & 0x3;
        fs != 0 // FS != OFF means FPU is enabled
    }
}

/// Disable the FPU
///
/// Sets FS field to OFF in SSTATUS
pub fn riscv_fpu_disable() {
    unsafe {
        let mut sstatus = registers::read_csr(csr::SSTATUS);
        sstatus &= !(0x3 << 13); // Clear FS field (sets to OFF)
        registers::write_csr(csr::SSTATUS, sstatus);
    }
}

/// Get current FPU state from SSTATUS
///
/// # Returns
///
/// Current FS field value (OFF, INITIAL, CLEAN, or DIRTY)
pub fn riscv_fpu_get_state() -> u64 {
    unsafe {
        let sstatus = registers::read_csr(csr::SSTATUS);
        (sstatus >> 13) & 0x3
    }
}

/// Context switch helper: save FPU if dirty, skip if clean
///
/// This is a lightweight check that avoids saving FPU state
/// if it hasn't been used since the last save.
///
/// # Arguments
///
/// * `state` - Pointer to FPU state to save to
///
/// # Returns
///
/// true if FPU state was saved, false if skipped
///
/// # Safety
///
/// state must point to valid memory
pub unsafe fn riscv_fpu_context_switch_save(state: *mut FpuState) -> bool {
    let fs = riscv_fpu_get_state();

    if fs == (FS_DIRTY >> 13) {
        // FPU is dirty, need to save
        riscv_fpu_save(state);
        true
    } else {
        // FPU is clean or off, no need to save
        false
    }
}

/// Initialize FPU state for a new thread
///
/// # Arguments
///
/// * `state` - Pointer to FPU state to initialize
///
/// # Safety
///
/// state must point to valid memory
pub unsafe fn riscv_fpu_init_thread(state: *mut FpuState) {
    // Zero the FPU state
    riscv_fpu_zero(state);

    // Mark FPU as clean in SSTATUS
    let mut sstatus = registers::read_csr(csr::SSTATUS);
    sstatus &= !(0x3 << 13); // Clear FS field
    sstatus |= FS_CLEAN;     // Set to Clean
    registers::write_csr(csr::SSTATUS, sstatus);
}

/// FPU exception codes
pub mod fpu_exceptions {
    pub const INVALID_OPERATION: u32 = 1 << 0;
    pub const DIVISION_BY_ZERO: u32 = 1 << 3;
    pub const OVERFLOW: u32 = 1 << 4;
    pub const UNDERFLOW: u32 = 1 << 5;
    pub const INEXACT: u32 = 1 << 6;
}

/// Get FPU exception flags from FCSR
///
/// # Returns
///
/// Bitmask of pending FPU exceptions
pub fn riscv_fpu_get_exceptions() -> u32 {
    unsafe {
        let fcsr: u32;
        core::arch::asm!("frcsr {0}", out(reg) fcsr, options(nostack));
        fcsr & 0x1F
    }
}

/// Clear FPU exception flags
pub fn riscv_fpu_clear_exceptions() {
    unsafe {
        let fcsr: u32;
        core::arch::asm!("frcsr {0}", out(reg) fcsr, options(nostack));
        // Clear exception flags (bits 0-4)
        let new_fcsr = fcsr & !0x1F;
        core::arch::asm!("fscsr {0}", in(reg) new_fcsr, options(nostack));
    }
}

/// Assert that FpuState is the correct size
const _: () = assert!(core::mem::size_of::<FpuState>() == 264);
