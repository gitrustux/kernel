// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86-64 Register Definitions
//!
//! This module provides MSR (Model Specific Register) and other
//! register definitions for x86-64 processors.

#![no_std]

/// MSR (Model Specific Register) indices
pub mod msr {
    /// IA32_GS_BASE - GS Segment Base Address
    pub const IA32_GS_BASE: u32 = 0xC000_0101;

    /// IA32_FS_BASE - FS Segment Base Address
    pub const IA32_FS_BASE: u32 = 0xC000_0100;

    /// IA32_KERNEL_GS_BASE - Swap GS Base for Kernel
    pub const IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;

    /// IA32_EFER - Extended Feature Enable Register
    pub const IA32_EFER: u32 = 0xC000_0080;

    /// IA32_APIC_BASE - Local APIC Base
    pub const IA32_APIC_BASE: u32 = 0x0000_001B;
}

/// Control register definitions
pub mod cr {
    /// CR0 - Control Register 0
    pub const CR0_PG: u64 = 1 << 31;  // Paging
    pub const CR0_CD: u64 = 1 << 30;  // Cache Disable
    pub const CR0_NW: u64 = 1 << 29;  // Not Write-through
    pub const CR0_AM: u64 = 1 << 18;  // Alignment Mask
    pub const CR0_WP: u64 = 1 << 16;  // Write Protect
    pub const CR0_NE: u64 = 1 << 5;   // Numeric Error

    /// CR4 - Control Register 4
    pub const CR4_PSE: u64 = 1 << 4;   // Page Size Extension
    pub const CR4_PAE: u64 = 1 << 5;   // Physical Address Extension
    pub const CR4_PGE: u64 = 1 << 7;   // Page Global Enable
    pub const CR4_OSFXSR: u64 = 1 << 9;  // OS FXSAVE/FXRSTOR Support
    pub const CR4_OSXMMEXCPT: u64 = 1 << 10;  // OS Exception Support
    pub const CR4_UMIP: u64 = 1 << 11; // User Mode Instruction Prevention
    pub const CR4_VMXE: u64 = 1 << 13; // VMX Enable
    pub const CR4_SMXE: u64 = 1 << 14; // SMX Enable
    pub const CR4_FSGSBASE: u64 = 1 << 16; // FSGSBASE Enable
    pub const CR4_PCIDE: u64 = 1 << 17; // Process-Context Identifiers
    pub const CR4_OSXSAVE: u64 = 1 << 18; // XSAVE and XRSTOR
    pub const CR4_SMEP: u64 = 1 << 20; // Supervisor Mode Execution Protection
    pub const CR4_SMAP: u64 = 1 << 21; // Supervisor Mode Access Prevention
    pub const CR4_CET: u64 = 1 << 23;  // Control-flow Enforcement Technology
}

/// RFLAGS register flags
pub mod rflags {
    pub const CF: u64 = 1 << 0;   // Carry Flag
    pub const PF: u64 = 1 << 2;   // Parity Flag
    pub const AF: u64 = 1 << 4;   // Auxiliary Carry Flag
    pub const ZF: u64 = 1 << 6;   // Zero Flag
    pub const SF: u64 = 1 << 7;   // Sign Flag
    pub const TF: u64 = 1 << 8;   // Trap Flag
    pub const IF: u64 = 1 << 9;   // Interrupt Enable Flag
    pub const DF: u64 = 1 << 10;  // Direction Flag
    pub const OF: u64 = 1 << 11;  // Overflow Flag
    pub const IOPL: u64 = 3 << 12; // I/O Privilege Level (2 bits)
    pub const NT: u64 = 1 << 14;  // Nested Task
    pub const RF: u64 = 1 << 16;  // Resume Flag
    pub const VM: u64 = 1 << 17;  // Virtual Mode
    pub const AC: u64 = 1 << 18;  // Alignment Check
    pub const VIF: u64 = 1 << 19; // Virtual Interrupt Flag
    pub const VIP: u64 = 1 << 20; // Virtual Interrupt Pending
    pub const ID: u64 = 1 << 21;  // ID Flag
}

/// Common RFLAGS constants (re-exported for convenience)
pub const X86_FLAGS_AC: u64 = rflags::AC;

/// EFER register flags
pub mod efer {
    pub const SCE: u64 = 1 << 0;   // System Call Extensions
    pub const LME: u64 = 1 << 8;   // Long Mode Enable
    pub const LMA: u64 = 1 << 10;  // Long Mode Active
    pub const NXE: u64 = 1 << 11;  // No-Execute Enable
    pub const SVME: u64 = 1 << 12; // Secure VM Enable
    pub const LMSLE: u64 = 1 << 13; // Long Mode Segment Limit Enable
    pub const FFXSR: u64 = 1 << 14; // Fast FXSAVE/FXRSTOR
    pub const TCE: u64 = 1 << 15;  // Translation Cache Extension
}

/// Maximum size of extended registers (AVX-512)
pub const X86_MAX_EXTENDED_REGISTER_SIZE: usize = 512;

/// System call general registers
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86SyscallGeneralRegs {
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub r10: u64,
    pub r8: u64,
    pub r9: u64,
}

/// Debug state (DR0-DR7)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86DebugState {
    pub dr0: u64,
    pub dr1: u64,
    pub dr2: u64,
    pub dr3: u64,
    pub dr6: u64,
    pub dr7: u64,
}

/// ============================================================================
/// MSR Access Functions
/// ============================================================================

/// Read a Model-Specific Register (MSR)
///
/// # Safety
///
/// The MSR number must be valid for the current CPU.
#[inline]
pub unsafe fn read_msr(msr: u32) -> u64 {
    let mut low: u32;
    let mut high: u32;
    core::arch::asm!(
        "rdmsr",
        out("eax") low,
        out("edx") high,
        in("ecx") msr,
        options(nomem, nostack)
    );
    (high as u64) << 32 | (low as u64)
}

/// Write to a Model-Specific Register (MSR)
///
/// # Safety
///
/// The MSR number must be valid for the current CPU.
#[inline]
pub unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nomem, nostack)
    );
}

/// ============================================================================
/// Control Register Access Functions
/// ============================================================================

/// Read CR0 register
///
/// # Safety
///
/// This function uses inline assembly to read CR0.
#[inline]
pub unsafe fn x86_get_cr0() -> u64 {
    let cr0: u64;
    core::arch::asm!(
        "mov {}, cr0",
        out(reg) cr0,
        options(nomem, nostack)
    );
    cr0
}

/// Read CR2 register (Page Fault Linear Address)
///
/// # Safety
///
/// This function uses inline assembly to read CR2.
#[inline]
pub unsafe fn x86_get_cr2() -> u64 {
    let cr2: u64;
    core::arch::asm!(
        "mov {}, cr2",
        out(reg) cr2,
        options(nomem, nostack)
    );
    cr2
}

/// Write CR0 register
///
/// # Safety
///
/// This function uses inline assembly to write CR0.
#[inline]
pub unsafe fn x86_set_cr0(value: u64) {
    core::arch::asm!(
        "mov cr0, {}",
        in(reg) value,
        options(nomem, nostack)
    );
}

/// ============================================================================
/// Interrupt Control Functions
/// ============================================================================

/// Disable interrupts (CLI)
///
/// # Safety
///
/// This function uses inline assembly to disable interrupts.
#[inline]
pub unsafe fn x86_cli() {
    core::arch::asm!("cli", options(nomem, nostack));
}

/// Enable interrupts (STI)
///
/// # Safety
///
/// This function uses inline assembly to enable interrupts.
#[inline]
pub unsafe fn x86_sti() {
    core::arch::asm!("sti", options(nomem, nostack));
}

/// Halt the CPU
///
/// # Safety
///
/// This function uses inline assembly to halt the CPU.
#[inline]
pub unsafe fn x86_hlt() {
    core::arch::asm!("hlt", options(nomem, nostack));
}

/// Check if interrupts are disabled
///
/// Returns true if interrupts are currently disabled (IF flag = 0).
#[inline]
pub fn arch_ints_disabled() -> bool {
    let rflags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq; pop {}",
            out(reg) rflags,
            options(nostack, nomem)
        );
    }
    rflags & rflags::IF == 0
}

/// ============================================================================
/// Debug Register Functions
/// ============================================================================

/// Read debug status (DR6)
///
/// # Safety
///
/// This function uses inline assembly to read DR6.
#[inline]
pub unsafe fn x86_read_debug_status(debug_state: &mut X86DebugState) {
    debug_state.dr6 = x86_read_dr6();
}

/// Read DR6 register
///
/// # Safety
///
/// This function uses inline assembly to read DR6.
#[inline]
unsafe fn x86_read_dr6() -> u64 {
    let dr6: u64;
    core::arch::asm!(
        "mov {}, dr6",
        out(reg) dr6,
        options(nomem, nostack)
    );
    dr6
}

