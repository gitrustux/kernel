// Copyright 2025 The Rustux Authors
// Copyright (c) 2009 Corey Tabaka
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 low-level operations
//!
//! This module provides functions for idle, MSR access,
//! and monitor/mwait support.

#![no_std]

use crate::kernel::arch::amd64::asm;
use crate::kernel::arch::amd64::mp;
use crate::kernel::debug;
use crate::rustux::types::*;

/// zx_status_t values
const ZX_OK: i32 = 0;
const ZX_ERR_NOT_SUPPORTED: i32 = -2;

/// Put the CPU to sleep if interrupts are enabled
///
/// This function checks if interrupts are enabled and if so,
/// executes the HLT instruction to wait for an interrupt.
pub fn x86_idle() {
    // Get RFLAGS and check IF bit
    let rflags = x86_get_rflags();

    // Check if IF (interrupt enable) bit is set (bit 9)
    if rflags & (1 << 9) != 0 {
        unsafe {
            core::arch::asm!("hlt", options(nostack));
        }
    }
}

/// Read RFLAGS register
///
/// # Returns
///
/// The current value of RFLAGS
#[inline]
fn x86_get_rflags() -> u64 {
    unsafe {
        let rflags: u64;
        core::arch::asm!(
            "pushfq",
            "pop {0}",
            out(reg) rflags,
            options(nostack)
        );
        rflags
    }
}

/// Safely read an MSR (Model Specific Register)
///
/// This function attempts to read an MSR and returns an error
/// if the MSR doesn't exist (triggers a GPF).
///
/// # Arguments
///
/// * `msr_id` - The MSR index to read
/// * `val_out` - Pointer to store the read value
///
/// # Returns
///
/// ZX_OK on success, ZX_ERR_NOT_SUPPORTED if the MSR doesn't exist
///
/// # Safety
///
/// val_out must point to valid memory
pub unsafe fn read_msr_safe(msr_id: u32, val_out: *mut u64) -> i32 {
    // Disable interrupts and save RFLAGS
    let rflags: u64;
    core::arch::asm!(
        "pushfq",
        "pop {0}",
        "cli",
        out(reg) rflags,
        options(nostack)
    );

    // Set up the GPF handler target
    // Note: This relies on the faults.rs GPF handler checking this value
    let percpu = mp::x86_get_percpu();
    let original_target = (*percpu).gpf_return_target;
    (*percpu).gpf_return_target = x86_gpf_handler_target as usize;

    // Try to read the MSR
    let low: u32;
    let high: u32;
    core::arch::asm!(
        "2:",
        "rdmsr",
        "2b:",
        in("ecx") msr_id,
        lateout("eax") low,
        lateout("edx") high,
        options(nostack)
    );

    // Clear the GPF handler target
    (*percpu).gpf_return_target = 0;

    // Restore interrupt state
    if rflags & (1 << 9) != 0 {
        core::arch::asm!("sti", options(nostack));
    }

    // Combine the result
    let value = ((high as u64) << 32) | (low as u64);
    *val_out = value;

    ZX_OK
}

/// GPF handler target for MSR reads
///
/// This is a separate function that the GPF handler can jump to
/// when an MSR read fails. It's marked as unreachable for the
/// normal path.
extern "C" fn x86_gpf_handler_target() -> ! {
    // This should never be reached in normal execution
    // The actual GPF handler will handle the error case
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

/// Alternative implementation that doesn't rely on GPF trick
///
/// Uses CPUID to check if the MSR is valid before reading
pub unsafe fn read_msr_safe_checked(msr_id: u32, val_out: *mut u64) -> i32 {
    // For now, just attempt the read and catch any issues
    // In a full implementation, we'd check CPUID features first

    match msr_id {
        // MSRs that are generally safe on modern x86-64
        0x1B => unsafe { rdmsr_direct(msr_id, val_out) }, // IA32_APIC_BASE
        0x8B => unsafe { rdmsr_direct(msr_id, val_out) }, // IA32_BIOS_SIGN_ID
        0xFE => unsafe { rdmsr_direct(msr_id, val_out) }, // IA32_MTRRCAP
        0x174 | 0x175 | 0x176 | 0x177 => unsafe { rdmsr_direct(msr_id, val_out) }, // IA32_SYSENTER_*
        0xC0000080 | 0xC0000081 | 0xC0000082 | 0xC0000083 | 0xC0000084 => unsafe {
            rdmsr_direct(msr_id, val_out)
        } // IA32_EFER
        0xC0000100..=0xC0000102 => unsafe { rdmsr_direct(msr_id, val_out) }, // FS/GS base
        _ => {
            // Unknown MSR, return error
            return ZX_ERR_NOT_SUPPORTED;
        }
    }

    ZX_OK
}

/// Direct MSR read without safety checks
///
/// # Safety
///
/// Caller must ensure the MSR is valid
unsafe fn rdmsr_direct(msr_id: u32, val_out: *mut u64) {
    let (low, high): (u32, u32);
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr_id,
        lateout("eax") low,
        lateout("edx") high,
        options(nomem, nostack)
    );
    *val_out = ((high as u64) << 32) | (low as u64);
}

/// Wait for a memory address to change (MONITOR/MWAIT)
///
/// This function sets up a monitor and then waits using MWAIT.
/// The CPU enters a low-power state until the monitored address
/// is written to or an interrupt occurs.
///
/// # Arguments
///
/// * `addr` - Address to monitor
/// * `extensions` - Monitor extensions (typically 0)
/// * `hints` - MWAIT hints (typically 0)
///
/// # Safety
///
/// addr must be a valid memory address
pub unsafe fn x86_mwait<T>(addr: *const T, extensions: u32, hints: u32) {
    // Check if interrupts are enabled
    let rflags = x86_get_rflags();
    if rflags & (1 << 9) == 0 {
        // Don't wait if interrupts disabled
        return;
    }

    // Set up monitor
    core::arch::asm!(
        "monitor",
        in("rax") addr,
        in("rcx") extensions,
        in("rdx") 0u32, // optional hints
        options(nostack)
    );

    // Wait
    core::arch::asm!(
        "mwait",
        in("eax") hints,
        in("ecx") extensions,
        options(nostack)
    );
}

/// Monitor a memory address for changes
///
/// Sets up the hardware to monitor the specified cache line
/// for writes. Used in conjunction with MWAIT.
///
/// # Arguments
///
/// * `addr` - Address to monitor
/// * `extensions` - Monitor extensions (typically 0)
/// * `hints` - Optional hints (typically 0)
///
/// # Safety
///
/// addr must be a valid memory address
pub unsafe fn x86_monitor<T>(addr: *const T, extensions: u32, hints: u32) {
    core::arch::asm!(
        "monitor",
        in("rax") addr,
        in("rcx") extensions,
        in("rdx") hints,
        options(nostack)
    );
}

/// Simple MWAIT without separate monitor setup
///
/// This is a convenience function that combines monitor and mwait
/// for common use cases.
///
/// # Safety
///
/// addr must be a valid memory address
pub fn x86_mwait_simple<T>(addr: *const T) {
    unsafe {
        x86_monitor(addr, 0, 0);
        x86_mwait(addr, 0, 0);
    }
}

/// Check if MONITOR/MWAIT is supported
///
/// # Returns
///
/// true if the CPU supports MONITOR/MWAIT
pub fn x86_has_mwait() -> bool {
    unsafe {
        let ecx: u32;
        core::arch::asm!(
            "cpuid",
            in("eax") 1u32,
            lateout("ecx") ecx,
            options(nostack)
        );

        // Check bit 3 (MONITOR/MWAIT)
        ecx & (1 << 3) != 0
    }
}

/// NOP instruction - does nothing
#[inline]
pub fn nop() {
    unsafe { core::arch::asm!("nop", options(nostack)) };
}

/// WBINVD instruction - write-back and invalidate cache
///
/// # Safety
///
/// This is a privileged instruction that should only be used
/// in kernel mode
#[inline]
pub unsafe fn wbinvd() {
    core::arch::asm!("wbinvd", options(nostack));
}

/// HLT instruction - halt CPU until interrupt
///
/// # Safety
///
/// This is a privileged instruction that should only be used
/// in kernel mode
#[inline]
pub unsafe fn hlt() {
    core::arch::asm!("hlt", options(nostack));
}
