// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

#![allow(non_snake_case)]

use crate::arch::arm64;
use crate::kernel::align::*;
use crate::kernel::cpu::*;
use crate::reg::*;
use crate::rustux::compiler::*;

// bits for mpidr register
pub const MPIDR_AFF0_MASK: u64 = 0xFF;
pub const MPIDR_AFF0_SHIFT: u64 = 0;
pub const MPIDR_AFF1_MASK: u64 = 0xFF << 8;
pub const MPIDR_AFF1_SHIFT: u64 = 8;
pub const MPIDR_AFF2_MASK: u64 = 0xFF << 16;
pub const MPIDR_AFF2_SHIFT: u64 = 16;
pub const MPIDR_AFF3_MASK: u64 = 0xFF << 32;
pub const MPIDR_AFF3_SHIFT: u64 = 32;

// construct a ARM MPID from cluster (AFF1) and cpu number (AFF0)
#[macro_export]
macro_rules! ARM64_MPID {
    ($cluster:expr, $cpu:expr) => {
        ((($cluster << MPIDR_AFF1_SHIFT) & MPIDR_AFF1_MASK) | 
        (($cpu << MPIDR_AFF0_SHIFT) & MPIDR_AFF0_MASK))
    };
}

// TODO: add support for AFF2 and AFF3

// Per cpu structure, pointed to by x18 while in kernel mode.
// Aligned on the maximum architectural cache line to avoid cache
// line sharing between cpus.
#[repr(C)]
#[repr(align(CPU_CACHE_LINE_SIZE))]
pub struct arm64_percpu {
    // cpu number
    pub cpu_num: u32,
    // Whether blocking is disallowed. See arch_blocking_disallowed().
    pub blocking_disallowed: u32,
}

extern "C" {
    pub fn arch_init_cpu_map(cluster_count: u32, cluster_cpus: *const u32);
    pub fn arm64_init_percpu_early();
    
    // Global variables
    pub static mut arm_num_cpus: u32;
    pub static mut arm64_cpu_cluster_ids: [u32; SMP_MAX_CPUS];
    pub static mut arm64_cpu_cpu_ids: [u32; SMP_MAX_CPUS];
}

// Use the x18 register to always point at the local cpu structure to allow fast access
// to a per cpu structure.
// Do not directly access fields of this structure
#[thread_local]
static mut __arm64_percpu: *mut arm64_percpu = core::ptr::null_mut();

#[inline(always)]
pub unsafe fn arm64_write_percpu_ptr(percpu: *mut arm64_percpu) {
    core::arch::asm!("mov x18, {}", in(reg) percpu);
    __arm64_percpu = percpu;
}

#[inline(always)]
pub unsafe fn arm64_read_percpu_ptr() -> *mut arm64_percpu {
    let percpu: *mut arm64_percpu;
    core::arch::asm!("mov {}, x18", out(reg) percpu);
    percpu
}

#[inline(always)]
pub unsafe fn arm64_read_percpu_u32(offset: usize) -> u32 {
    let val: u32;
    // mark as volatile to force a read of the field to make sure
    // the compiler always emits a read when asked and does not cache
    // a copy between
    core::arch::asm!(
        "ldr {0:w}, [x18, {1}]",
        out(reg) val,
        in(reg) offset,
        options(nomem, nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub unsafe fn arm64_write_percpu_u32(offset: usize, val: u32) {
    core::arch::asm!(
        "str {0:w}, [x18, {1}]",
        in(reg) val,
        in(reg) offset,
        options(nomem, nostack, preserves_flags)
    );
}

#[inline(always)]
pub unsafe fn arch_curr_cpu_num() -> cpu_num_t {
    arm64_read_percpu_u32(core::mem::offset_of!(arm64_percpu, cpu_num))
}

#[inline(always)]
pub unsafe fn arch_max_num_cpus() -> u32 {
    arm_num_cpus
}

// translate a cpu number back to the cluster ID (AFF1)
#[inline(always)]
pub unsafe fn arch_cpu_num_to_cluster_id(cpu: u32) -> u32 {
    arm64_cpu_cluster_ids[cpu as usize]
}

// translate a cpu number back to the MP cpu number within a cluster (AFF0)
#[inline(always)]
pub unsafe fn arch_cpu_num_to_cpu_id(cpu: u32) -> u32 {
    arm64_cpu_cpu_ids[cpu as usize]
}

pub unsafe fn arch_mpid_to_cpu_num(cluster: u32, cpu: u32) -> cpu_num_t;

#[macro_export]
macro_rules! READ_PERCPU_FIELD32 {
    ($field:ident) => {
        unsafe { arm64_read_percpu_u32(core::mem::offset_of!(arm64_percpu, $field)) }
    };
}

#[macro_export]
macro_rules! WRITE_PERCPU_FIELD32 {
    ($field:ident, $value:expr) => {
        unsafe { arm64_write_percpu_u32(core::mem::offset_of!(arm64_percpu, $field), $value) }
    };
}