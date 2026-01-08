// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Multiprocessor support for x86
//!
//! This module defines the per-CPU structure that contains state for each CPU
//! in the system, as well as functions for CPU management and inter-processor
//! communication.

// Constants for offsets into the per-CPU structure, used by assembly code
/// Offset to direct pointer
pub const PERCPU_DIRECT_OFFSET: usize = 0x0;
/// Offset to current thread
pub const PERCPU_CURRENT_THREAD_OFFSET: usize = 0x8;
// ZX_TLS_STACK_GUARD_OFFSET at 0x10
// ZX_TLS_UNSAFE_SP_OFFSET at 0x18
/// Offset to saved user stack pointer
pub const PERCPU_SAVED_USER_SP_OFFSET: usize = 0x20;
/// Offset to GPF return target
pub const PERCPU_GPF_RETURN_OFFSET: usize = 0x40;
/// Offset to CPU number
pub const PERCPU_CPU_NUM_OFFSET: usize = 0x48;
/// Offset to default TSS
pub const PERCPU_DEFAULT_TSS_OFFSET: usize = 0x50;

/// Offset of default_tss.rsp0
pub const PERCPU_KERNEL_SP_OFFSET: usize = PERCPU_DEFAULT_TSS_OFFSET + 4;

use crate::arch::amd64;
use crate::arch::amd64::idt::Tss;
use crate::kernel::align::{Aligned, CpuAlign};
use crate::kernel::cpu::{CpuNum, NUM_ASSIGNED_IST_ENTRIES};
use crate::kernel::vm::layout::PAGE_SIZE;
use crate::rustux::tls::{ZX_TLS_STACK_GUARD_OFFSET, ZX_TLS_UNSAFE_SP_OFFSET};
use crate::rustux::types::*;
use crate::kernel::thread::Thread;
use crate::kernel::arch::amd64::include::arch::current_thread::{x86_read_gs_offset32, x86_read_gs_offset64, x86_write_gs_offset32};
use core::mem::offset_of;
use core::sync::atomic::AtomicU8;

/// Per-CPU structure for x86 processors
#[repr(C, align(64))] // __CPU_ALIGN is typically 64 bytes
pub struct X86PerCpu {
    /// Direct pointer to this struct (for easy access via GS segment)
    pub direct: *mut X86PerCpu,

    /// The current thread running on this CPU
    pub current_thread: *mut Thread,

    /// Stack guard value (used by TLS)
    pub stack_guard: usize,

    /// Kernel unsafe stack pointer (used by TLS)
    pub kernel_unsafe_sp: usize,

    /// Temporarily saved user stack pointer during syscalls
    pub saved_user_sp: usize,

    /// Whether blocking is disallowed on this CPU
    pub blocking_disallowed: u32,

    /// Memory for IPI-free rescheduling of idle CPUs with monitor/mwait
    pub monitor: *mut u8,

    /// Local APIC ID for this CPU
    pub apic_id: u32,

    /// If nonzero and we receive a GPF, change the return IP to this value
    pub gpf_return_target: usize,

    /// CPU number
    pub cpu_num: CpuNum,

    /// This CPU's default TSS
    pub default_tss: Tss,

    /// Reserved space for interrupt stacks
    pub interrupt_stacks: [[u8; PAGE_SIZE]; NUM_ASSIGNED_IST_ENTRIES],
}

// Static assertions to ensure the structure layout matches the expected offsets
const _: () = assert!(offset_of!(X86PerCpu, direct) == PERCPU_DIRECT_OFFSET);
const _: () = assert!(offset_of!(X86PerCpu, current_thread) == PERCPU_CURRENT_THREAD_OFFSET);
const _: () = assert!(offset_of!(X86PerCpu, stack_guard) == ZX_TLS_STACK_GUARD_OFFSET);
const _: () = assert!(offset_of!(X86PerCpu, kernel_unsafe_sp) == ZX_TLS_UNSAFE_SP_OFFSET);
const _: () = assert!(offset_of!(X86PerCpu, saved_user_sp) == PERCPU_SAVED_USER_SP_OFFSET);
const _: () = assert!(offset_of!(X86PerCpu, gpf_return_target) == PERCPU_GPF_RETURN_OFFSET);
const _: () = assert!(offset_of!(X86PerCpu, cpu_num) == PERCPU_CPU_NUM_OFFSET);
// TODO: Fix TSS offset assertions when TSS structure is properly implemented
// const _: () = assert!(offset_of!(X86PerCpu, default_tss) == PERCPU_DEFAULT_TSS_OFFSET);
// const _: () = assert!(offset_of!(X86PerCpu, default_tss) + 4 == PERCPU_KERNEL_SP_OFFSET);

// Global data
extern "C" {
    /// Bootstrap processor's per-CPU data
    pub static BP_PERCPU: X86PerCpu;
    
    /// Application processors' per-CPU data (array)
    pub static AP_PERCPUS: *mut X86PerCpu;
    
    /// Number of CPUs in the system
    pub static X86_NUM_CPUS: AtomicU8;
}

/// Initialize per-CPU data for a CPU
///
/// This needs to be run very early in the boot process from start.S and as
/// each CPU is brought up.
///
/// # Arguments
///
/// * `cpu_num` - The CPU number to initialize
///
/// # Safety
///
/// This function modifies CPU state directly and must be called only during
/// system initialization or CPU bringup.
pub unsafe fn x86_init_percpu(cpu_num: u32) {
    sys_x86_init_percpu(cpu_num);
}

/// Set the bootstrap processor's APIC ID
///
/// Used to set the bootstrap processor's apic_id once the APIC is initialized.
///
/// # Arguments
///
/// * `apic_id` - The APIC ID for the bootstrap processor
///
/// # Safety
///
/// This function should only be called once during system initialization.
pub unsafe fn x86_set_local_apic_id(apic_id: u32) {
    sys_x86_set_local_apic_id(apic_id);
}

/// Convert an APIC ID to a CPU number
///
/// # Arguments
///
/// * `apic_id` - The APIC ID to convert
///
/// # Returns
///
/// The CPU number corresponding to the APIC ID, or a negative error code on failure
pub fn x86_apic_id_to_cpu_num(apic_id: u32) -> i32 {
    unsafe { sys_x86_apic_id_to_cpu_num(apic_id) }
}

/// Allocate necessary structures for APs to run
///
/// # Arguments
///
/// * `apic_ids` - Array of APIC IDs for the APs
/// * `cpu_count` - Number of CPUs in the system
///
/// # Returns
///
/// A status code indicating success or the type of failure
///
/// # Safety
///
/// This function allocates system memory and should only be called during system initialization.
pub unsafe fn x86_allocate_ap_structures(apic_ids: &[u32], cpu_count: u8) -> RxStatus {
    sys_x86_allocate_ap_structures(apic_ids.as_ptr(), cpu_count)
}

/// Get a reference to the current CPU's per-CPU data
///
/// # Returns
///
/// A reference to the current CPU's per-CPU structure
#[inline]
pub fn x86_get_percpu() -> &'static X86PerCpu {
    unsafe {
        let ptr = x86_read_gs_offset64(PERCPU_DIRECT_OFFSET as u32) as *const X86PerCpu;
        &*ptr
    }
}

/// Get the current CPU number
///
/// # Returns
///
/// The current CPU's number
#[inline]
pub fn arch_curr_cpu_num() -> CpuNum {
    x86_get_percpu().cpu_num
}

/// Get the maximum number of CPUs supported by the system
///
/// # Returns
///
/// The maximum number of CPUs
#[inline]
pub fn arch_max_num_cpus() -> u32 {
    unsafe { X86_NUM_CPUS.load(core::sync::atomic::Ordering::Acquire) as u32 }
}

/// Read a 32-bit field from the current CPU's per-CPU structure
///
/// # Arguments
///
/// * `offset` - Offset into the per-CPU structure
///
/// # Returns
///
/// The value at the specified offset
///
/// # Safety
///
/// This function is unsafe because it performs a direct read from the GS segment
/// at the specified offset without type checking.
#[inline]
pub unsafe fn read_percpu_field32(offset: usize) -> u32 {
    x86_read_gs_offset32(offset as u32)
}

/// Write a 32-bit value to a field in the current CPU's per-CPU structure
///
/// # Arguments
///
/// * `offset` - Offset into the per-CPU structure
/// * `value` - Value to write
///
/// # Safety
///
/// This function is unsafe because it performs a direct write to the GS segment
/// at the specified offset without type checking.
#[inline]
pub unsafe fn write_percpu_field32(offset: usize, value: u32) {
    x86_write_gs_offset32(offset as u32, value);
}

/// Handle IPI for halting a CPU
///
/// # Arguments
///
/// * `arg` - Unused argument
///
/// # Safety
///
/// This function never returns and halts the CPU.
pub unsafe extern "C" fn x86_ipi_halt_handler(_arg: *mut core::ffi::c_void) -> ! {
    sys_x86_ipi_halt_handler();
}

/// Entry point for secondary CPUs
///
/// # Arguments
///
/// * `aps_still_booting` - Pointer to counter of APs still in boot process
/// * `thread` - Pointer to initial thread for this CPU
///
/// # Safety
///
/// This function is called during CPU initialization and should not be called directly.
pub unsafe fn x86_secondary_entry(aps_still_booting: &core::sync::atomic::AtomicI32, thread: *mut Thread) {
    sys_x86_secondary_entry(aps_still_booting as *const _ as *mut _, thread);
}

/// Force all CPUs except the local one and BSP to halt
///
/// # Safety
///
/// This function sends IPIs to halt CPUs and should only be used in emergency situations.
pub unsafe fn x86_force_halt_all_but_local_and_bsp() {
    sys_x86_force_halt_all_but_local_and_bsp();
}

// FFI declarations for the system functions
extern "C" {
    fn sys_x86_init_percpu(cpu_num: u32);
    fn sys_x86_set_local_apic_id(apic_id: u32);
    fn sys_x86_apic_id_to_cpu_num(apic_id: u32) -> i32;
    fn sys_x86_allocate_ap_structures(apic_ids: *const u32, cpu_count: u8) -> RxStatus;
    fn sys_x86_ipi_halt_handler() -> !;
    fn sys_x86_secondary_entry(aps_still_booting: *mut i32, thread: *mut Thread);
    fn sys_x86_force_halt_all_but_local_and_bsp();
}