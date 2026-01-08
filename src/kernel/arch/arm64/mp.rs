// Copyright 2025 Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::ops::*;
use crate::kernel::dev::interrupt::*;
use crate::err::*;
use crate::kernel::event::*;
use crate::platform::*;
use crate::trace::*;
use crate::rustux::types::*;

const LOCAL_TRACE: bool = false;

// map of cluster/cpu to cpu_id
static mut ARM64_CPU_MAP: [[u32; SMP_CPU_MAX_CLUSTER_CPUS]; SMP_CPU_MAX_CLUSTERS] = [[0; SMP_CPU_MAX_CLUSTER_CPUS]; SMP_CPU_MAX_CLUSTERS];

// cpu id to cluster and id within cluster map
static mut ARM64_CPU_CLUSTER_IDS: [u32; SMP_MAX_CPUS] = [0; SMP_MAX_CPUS];
static mut ARM64_CPU_CPU_IDS: [u32; SMP_MAX_CPUS] = [0; SMP_MAX_CPUS];

// total number of detected cpus
static mut ARM_NUM_CPUS: u32 = 1;

// per cpu structures, each cpu will point to theirs using the x18 register
static mut ARM64_PERCPU_ARRAY: [Arm64Percpu; SMP_MAX_CPUS] = [Arm64Percpu::new(); SMP_MAX_CPUS];

// Define constants
const MPIDR_AFF0_MASK: u64 = 0xFF;
const MPIDR_AFF0_SHIFT: u64 = 0;
const MPIDR_AFF1_MASK: u64 = 0xFF00;
const MPIDR_AFF1_SHIFT: u64 = 8;

// Define types that are used in this file
type cpu_mask_t = u32;
type cpu_num_t = u32;

// Define structs and enums
#[derive(Copy, Clone)]
pub struct Arm64Percpu {
    pub cpu_num: u32,
    // Additional fields would be defined here
}

impl Arm64Percpu {
    const fn new() -> Self {
        Arm64Percpu {
            cpu_num: 0,
            // Initialize other fields
        }
    }
}

#[repr(u32)]
pub enum MpIpiTarget {
    MpIpiTargetAll = 0,
    MpIpiTargetAllButLocal = 1,
    MpIpiTargetMask = 2,
}

#[repr(u32)]
pub enum MpIpi {
    MpIpiReschedule = 0,
    // Add other IPI types as needed
}

// initializes cpu_map and arm_num_cpus
pub fn arch_init_cpu_map(cluster_count: u32, cluster_cpus: &[u32]) {
    assert!(cluster_count <= SMP_CPU_MAX_CLUSTERS as u32, "Too many clusters");

    // assign cpu_ids sequentially
    let mut cpu_id = 0;
    
    for cluster in 0..cluster_count {
        let cpus = cluster_cpus[cluster as usize];
        assert!(cpus <= SMP_CPU_MAX_CLUSTER_CPUS as u32, "Too many CPUs in cluster");
        
        for cpu in 0..cpus {
            unsafe {
                // given cluster:cpu, translate to global cpu id
                ARM64_CPU_MAP[cluster as usize][cpu as usize] = cpu_id;

                // given global gpu_id, translate to cluster and cpu number within cluster
                ARM64_CPU_CLUSTER_IDS[cpu_id as usize] = cluster;
                ARM64_CPU_CPU_IDS[cpu_id as usize] = cpu;

                // set the per cpu structure's cpu id
                ARM64_PERCPU_ARRAY[cpu_id as usize].cpu_num = cpu_id;
            }

            cpu_id += 1;
        }
    }
    
    unsafe {
        ARM_NUM_CPUS = cpu_id;
    }
    
    smp_mb();
}

// do the 'slow' lookup by mpidr to cpu number
fn arch_curr_cpu_num_slow() -> u32 {
    let mpidr = unsafe { __arm_rsr64("mpidr_el1") };
    let cluster = ((mpidr & MPIDR_AFF1_MASK) >> MPIDR_AFF1_SHIFT) as usize;
    let cpu = ((mpidr & MPIDR_AFF0_MASK) >> MPIDR_AFF0_SHIFT) as usize;

    unsafe { ARM64_CPU_MAP[cluster][cpu] }
}

pub fn arch_mpid_to_cpu_num(cluster: u32, cpu: u32) -> cpu_num_t {
    unsafe { ARM64_CPU_MAP[cluster as usize][cpu as usize] }
}

pub fn arch_prepare_current_cpu_idle_state(idle: bool) {
    // no-op
}

pub fn arch_mp_reschedule(mask: cpu_mask_t) -> rx_status_t {
    arch_mp_send_ipi(MpIpiTarget::MpIpiTargetMask, mask, MpIpi::MpIpiReschedule)
}

pub fn arch_mp_send_ipi(target: MpIpiTarget, mut mask: cpu_mask_t, ipi: MpIpi) -> rx_status_t {
    ltrace!("target {:?} mask {:#x}, ipi {:?}\n", target, mask, ipi);

    // translate the high level target + mask mechanism into just a mask
    match target {
        MpIpiTarget::MpIpiTargetAll => {
            mask = (1 << SMP_MAX_CPUS) - 1;
        },
        MpIpiTarget::MpIpiTargetAllButLocal => {
            mask = (1 << SMP_MAX_CPUS) - 1;
            mask &= !cpu_num_to_mask(arch_curr_cpu_num());
        },
        MpIpiTarget::MpIpiTargetMask => {},
    }

    interrupt_send_ipi(mask, ipi as u32)
}

pub fn arm64_init_percpu_early() {
    // slow lookup the current cpu id and setup the percpu structure
    let cpu = arch_curr_cpu_num_slow() as usize;

    unsafe {
        arm64_write_percpu_ptr(&ARM64_PERCPU_ARRAY[cpu]);
    }
}

pub fn arch_mp_init_percpu() {
    interrupt_init_percpu();
}

pub fn arch_flush_state_and_halt(flush_done: &Event) {
    debug_assert!(arch_ints_disabled());
    event_signal(flush_done, false);
    platform_halt_cpu();
    panic!("control should never reach here\n");
}

pub fn arch_mp_prep_cpu_unplug(cpu_id: u32) -> rx_status_t {
    unsafe {
        if cpu_id == 0 || cpu_id >= ARM_NUM_CPUS {
            return RX_ERR_INVALID_ARGS;
        }
    }
    RX_OK
}

pub fn arch_mp_cpu_unplug(cpu_id: u32) -> rx_status_t {
    // we do not allow unplugging the bootstrap processor
    unsafe {
        if cpu_id == 0 || cpu_id >= ARM_NUM_CPUS {
            return RX_ERR_INVALID_ARGS;
        }
    }
    RX_OK
}

// External function declarations
extern "C" {
    fn __arm_rsr64(reg: &str) -> u64;
    fn arm64_write_percpu_ptr(ptr: *const Arm64Percpu);
    fn cpu_num_to_mask(cpu_num: u32) -> cpu_mask_t;
    pub fn arch_curr_cpu_num() -> u32;
    pub fn arch_ints_disabled() -> bool;
    fn smp_mb();
    fn interrupt_init_percpu();
    fn interrupt_send_ipi(mask: cpu_mask_t, ipi: u32) -> rx_status_t;
    fn platform_halt_cpu() -> !;
    fn event_signal(event: &Event, reschedule: bool);
}

// Constants that should be defined elsewhere but referenced here
const SMP_CPU_MAX_CLUSTERS: usize = 16;
const SMP_CPU_MAX_CLUSTER_CPUS: usize = 16;
const SMP_MAX_CPUS: usize = 16;

// Status codes
const RX_OK: rx_status_t = 0;
const RX_ERR_INVALID_ARGS: rx_status_t = -10;
/// Prepare CPU for idle state
pub fn arm64_prepare_cpu_idle(_idle: bool) {
    // TODO: Implement CPU idle preparation
    // May involve WFI/WFE instructions
}

/// Reschedule CPUs
pub fn arm64_mp_reschedule(_cpu_mask: u32) {
    // TODO: Implement reschedule IPI
}

/// Hotplug CPU
pub fn arm64_mp_cpu_hotplug(_cpu_id: u32) -> i32 {
    // TODO: Implement CPU hotplug
    0
}

/// Unplug CPU
pub fn arm64_mp_cpu_unplug(_cpu_id: u32) -> i32 {
    // TODO: Implement CPU unplug
    0
}

/// Get CPU count
pub fn arm64_cpu_count() -> u32 {
    // TODO: Implement actual CPU count detection
    1
}
