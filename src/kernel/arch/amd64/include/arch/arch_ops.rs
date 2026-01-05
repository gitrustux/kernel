// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture-specific operations for x86_64
//!
//! This module provides low-level CPU operations specific to the x86_64
//! architecture, including interrupt control, cache operations, and 
//! performance primitives.

use crate::kernel::arch::amd64;
use crate::kernel::arch::amd64::mp;
use crate::rustux::types::*;
use core::sync::atomic;
use core::sync::atomic::Ordering;

/// Enable interrupts
///
/// # Safety
///
/// This function is unsafe because it directly modifies the CPU state,
/// which can affect interrupt handling system-wide.
#[inline]
pub unsafe fn arch_enable_ints() {
    core::sync::atomic::compiler_fence(Ordering::SeqCst);
    core::arch::asm!("sti", options(nomem, nostack));
}

/// Disable interrupts
///
/// # Safety
///
/// This function is unsafe because it directly modifies the CPU state,
/// which can affect interrupt handling system-wide.
#[inline]
pub unsafe fn arch_disable_ints() {
    core::arch::asm!("cli", options(nomem, nostack));
    core::sync::atomic::compiler_fence(Ordering::SeqCst);
}

/// Check if interrupts are disabled
///
/// # Returns
///
/// `true` if interrupts are disabled, `false` otherwise
#[inline]
pub fn arch_ints_disabled() -> bool {
    let flags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq",
            "popq {}",
            out(reg) flags,
            options(nomem, preserves_flags)
        );
    }
    (flags & (1 << 9)) == 0
}

/// Get the current CPU cycle count
///
/// # Returns
///
/// The current CPU timestamp counter value
#[inline]
pub fn arch_cycle_count() -> u64 {
    unsafe { crate::kernel::arch::amd64::asm::rdtsc() }
}

/// Pause the CPU (used in spin loops)
///
/// This function provides a hint to the CPU that we're in a spin loop,
/// which can improve performance and power consumption.
#[inline]
pub fn arch_spinloop_pause() {
    unsafe {
        core::arch::asm!("pause", options(nomem, nostack));
    }
}

/// Full memory barrier
///
/// Ensures that all memory operations before this point
/// are visible to other CPUs before any operations after this point.
#[inline]
pub fn mb() {
    unsafe {
        core::arch::asm!("mfence", options(nomem, nostack));
    }
}

/// Symmetric multiprocessing memory barrier
///
/// On x86_64, this is equivalent to a full memory barrier.
#[inline]
pub fn smp_mb() {
    mb();
}

/// Get the architecture-specific CPU features
///
/// This is a legacy function. Use cpuid instead for detailed feature detection.
///
/// # Returns
///
/// Always returns 0 on x86_64
#[inline]
pub fn arch_cpu_features() -> u32 {
    0 // Use cpuid instead
}

/// Get the data cache line size
///
/// # Returns
///
/// The data cache line size in bytes
pub fn arch_dcache_line_size() -> u32 {
    unsafe { sys_arch_dcache_line_size() }
}

/// Get the instruction cache line size
///
/// # Returns
///
/// The instruction cache line size in bytes
pub fn arch_icache_line_size() -> u32 {
    unsafe { sys_arch_icache_line_size() }
}

/// Log architecture-specific data for process creation
///
/// This can only be called after the process has been created and before
/// it is running.
///
/// # Arguments
///
/// * `pid` - Process ID
/// * `pt_phys` - Physical address of the page table
///
/// # Safety
///
/// This function is unsafe because it accesses physical memory addresses.
pub unsafe fn arch_trace_process_create(pid: u64, pt_phys: PAddr) {
    sys_arch_trace_process_create(pid, pt_phys);
}

// System function declarations
extern "C" {
    fn sys_arch_dcache_line_size() -> u32;
    fn sys_arch_icache_line_size() -> u32;
    fn sys_arch_trace_process_create(pid: u64, pt_phys: PAddr);
}

// Export common CPU operations to assembly
#[no_mangle]
pub unsafe extern "C" fn rust_arch_enable_ints() {
    arch_enable_ints();
}

#[no_mangle]
pub unsafe extern "C" fn rust_arch_disable_ints() {
    arch_disable_ints();
}

#[no_mangle]
pub extern "C" fn rust_arch_ints_disabled() -> bool {
    arch_ints_disabled()
}

#[no_mangle]
pub extern "C" fn rust_arch_cycle_count() -> u64 {
    arch_cycle_count()
}

#[no_mangle]
pub extern "C" fn rust_arch_spinloop_pause() {
    arch_spinloop_pause();
}

#[no_mangle]
pub extern "C" fn rust_mb() {
    mb();
}

#[no_mangle]
pub extern "C" fn rust_smp_mb() {
    smp_mb();
}