// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Performance Monitoring support
//!
//! This module provides functions for configuring and using the x86 Performance
//! Monitoring Unit (PMU), which allows collecting hardware performance counters.

use crate::arch::amd64::{self, X86Iframe};
use crate::fbl::RefPtr;
use crate::vm::vm_object::VmObject;
use crate::rustux::types::*;
use crate::lib::rx_internal::device::cpu_trace::intel_pm::{RxX86PmuProperties, RxX86PmuConfig};

/// Get the properties of the CPU's performance monitoring capabilities
///
/// # Arguments
///
/// * `state` - Output buffer to receive the PMU properties
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn arch_perfmon_get_properties(state: &mut RxX86PmuProperties) -> RxStatus {
    unsafe { sys_arch_perfmon_get_properties(state) }
}

/// Initialize the performance monitoring subsystem
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn arch_perfmon_init() -> RxStatus {
    unsafe { sys_arch_perfmon_init() }
}

/// Assign a buffer for a CPU's performance monitoring data
///
/// # Arguments
///
/// * `cpu` - CPU index to assign the buffer to
/// * `vmo` - Reference counted virtual memory object to use as the buffer
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn arch_perfmon_assign_buffer(cpu: u32, vmo: RefPtr<VmObject>) -> RxStatus {
    unsafe { sys_arch_perfmon_assign_buffer(cpu, vmo) }
}

/// Stage a performance monitoring configuration
///
/// # Arguments
///
/// * `config` - The PMU configuration to stage
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn arch_perfmon_stage_config(config: &mut RxX86PmuConfig) -> RxStatus {
    unsafe { sys_arch_perfmon_stage_config(config) }
}

/// Start performance monitoring with the staged configuration
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn arch_perfmon_start() -> RxStatus {
    unsafe { sys_arch_perfmon_start() }
}

/// Stop performance monitoring
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn arch_perfmon_stop() -> RxStatus {
    unsafe { sys_arch_perfmon_stop() }
}

/// Clean up and finalize the performance monitoring subsystem
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn arch_perfmon_fini() -> RxStatus {
    unsafe { sys_arch_perfmon_fini() }
}

/// Handle a Performance Monitoring Interrupt (PMI)
///
/// This function is called when a performance monitoring interrupt occurs.
///
/// # Arguments
///
/// * `frame` - The interrupt frame at the time of the interrupt
///
/// # Safety
///
/// This function is called from the interrupt handler and should follow
/// all restrictions for interrupt context.
pub unsafe fn apic_pmi_interrupt_handler(frame: &mut X86Iframe) {
    sys_apic_pmi_interrupt_handler(frame)
}

// External function declarations
extern "C" {
    fn sys_arch_perfmon_get_properties(state: *mut RxX86PmuProperties) -> RxStatus;
    fn sys_arch_perfmon_init() -> RxStatus;
    fn sys_arch_perfmon_assign_buffer(cpu: u32, vmo: RefPtr<VmObject>) -> RxStatus;
    fn sys_arch_perfmon_stage_config(config: *mut RxX86PmuConfig) -> RxStatus;
    fn sys_arch_perfmon_start() -> RxStatus;
    fn sys_arch_perfmon_stop() -> RxStatus;
    fn sys_arch_perfmon_fini() -> RxStatus;
    fn sys_apic_pmi_interrupt_handler(frame: *mut X86Iframe);
}