// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! AMD64 Multiprocessing (MP) Support
//!
//! This module provides support for multiple CPU cores on x86-64.

#![no_std]

use core::sync::atomic::{AtomicU32, Ordering};

/// Current CPU ID
static CPU_ID: AtomicU32 = AtomicU32::new(0);

/// Get the current CPU ID
///
/// Returns the ID of the calling CPU core.
pub fn x86_get_cpuid() -> u32 {
    CPU_ID.load(Ordering::Acquire)
}

/// Set the current CPU ID
///
/// # Safety
///
/// This should only be called during CPU initialization.
pub unsafe fn x86_set_cpuid(id: u32) {
    CPU_ID.store(id, Ordering::Release);
}

/// Get the total number of CPUs
pub fn x86_cpu_count() -> u32 {
    // TODO: Implement proper CPU count detection
    1
}

/// APIC ID to CPU number mapping
///
/// Converts an APIC ID to a CPU number.
pub fn x86_apic_id_to_cpu_num(apic_id: u32) -> i32 {
    // TODO: Implement proper APIC ID to CPU number mapping
    if apic_id == 0 {
        0
    } else {
        -1
    }
}

/// Per-CPU current thread offset
pub const PERCPU_CURRENT_THREAD_OFFSET: u32 = 0;

/// PerCPU structure placeholder
///
/// This is a minimal stub for the per-CPU data structure.
#[repr(C)]
pub struct PerCpu {
    /// APIC ID for this CPU
    pub apic_id: u32,
    /// Current thread pointer
    pub current_thread: usize,
    /// Default TSS RSP0 (kernel stack pointer)
    pub default_tss: TssState,
    /// Stack guard value
    pub stack_guard: u64,
    /// GPF return target for exception handling
    pub gpf_return_target: usize,
}

/// TSS state placeholder
#[repr(C)]
pub struct TssState {
    pub rsp0: u64,
}

/// Get the per-CPU structure for the current CPU
///
/// # Safety
///
/// This function assumes the per-CPU base is properly set up in GS.
pub unsafe fn x86_get_percpu() -> *mut PerCpu {
    // Read GS base to get per-CPU pointer
    let mut gs_base: u64;
    core::arch::asm!(
        "mov {}, gs:[0]",
        lateout(reg) gs_base,
        options(nostack, nomem)
    );
    gs_base as *mut PerCpu
}

/// Convert CPU number to mask
///
/// Converts a CPU number (0-indexed) to a bit mask for IPI targeting.
pub fn cpu_num_to_mask(cpu_num: u32) -> u64 {
    1u64 << cpu_num
}

/// Initialize per-CPU data
///
/// # Safety
///
/// Must be called with a valid CPU number.
pub unsafe fn x86_init_percpu(cpu_num: u32) {
    x86_set_cpuid(cpu_num);
    // TODO: Initialize per-CPU fields like current_thread, stack_guard, etc.
}

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 256;

/// Per-CPU structures array
///
/// Static array of per-CPU structures for all CPUs.
static mut PERCPUS: [PerCpu; MAX_CPUS] = unsafe { core::mem::zeroed() };

/// Get the per-CPU structures array
///
/// # Safety
///
/// Returns a pointer to the per-CPU array.
pub unsafe fn ap_percpus() -> *mut PerCpu {
    PERCPUS.as_ptr() as *mut PerCpu
}

/// Get the bootstrap CPU's per-CPU structure
///
/// # Safety
///
/// Returns a pointer to CPU 0's per-CPU structure.
pub unsafe fn bp_percpu() -> *mut PerCpu {
    &mut PERCPUS[0] as *mut PerCpu
}
