// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Processor Trace support for x86
//!
//! This module provides functionality for working with Intel Processor Trace (PT),
//! a hardware feature that allows tracking control flow through an application.

use crate::lib::rx_internal::device::cpu_trace::intel_pt::{RxItraceBufferDescriptor, RxX86PtRegs};
use crate::rustux::types::*;

/// Intel PT trace mode options
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IptTraceMode {
    /// Trace specific CPUs
    TraceCpus,
    /// Trace specific threads
    TraceThreads,
}

/// Initialize x86 processor trace support
///
/// This function should be called early in system initialization
/// to set up processor trace capabilities.
///
/// # Safety
///
/// This function modifies CPU state and should only be called during
/// system initialization.
pub unsafe fn x86_processor_trace_init() {
    sys_x86_processor_trace_init();
}

/// Allocate trace resources
///
/// # Arguments
///
/// * `mode` - Whether to trace CPUs or threads
/// * `num_traces` - Number of trace buffers to allocate
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn x86_ipt_alloc_trace(mode: IptTraceMode, num_traces: u32) -> RxStatus {
    unsafe { sys_x86_ipt_alloc_trace(mode, num_traces) }
}

/// Free previously allocated trace resources
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn x86_ipt_free_trace() -> RxStatus {
    unsafe { sys_x86_ipt_free_trace() }
}

/// Start the trace
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn x86_ipt_start() -> RxStatus {
    unsafe { sys_x86_ipt_start() }
}

/// Stop the trace
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn x86_ipt_stop() -> RxStatus {
    unsafe { sys_x86_ipt_stop() }
}

/// Stage trace data configuration
///
/// # Arguments
///
/// * `descriptor` - Trace buffer descriptor
/// * `regs` - Processor trace register values to stage
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn x86_ipt_stage_trace_data(
    descriptor: RxItraceBufferDescriptor,
    regs: &RxX86PtRegs,
) -> RxStatus {
    unsafe { sys_x86_ipt_stage_trace_data(descriptor, regs) }
}

/// Get trace data from a buffer
///
/// # Arguments
///
/// * `descriptor` - Trace buffer descriptor
/// * `regs` - Output buffer to receive processor trace register values
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn x86_ipt_get_trace_data(
    descriptor: RxItraceBufferDescriptor,
    regs: &mut RxX86PtRegs,
) -> RxStatus {
    unsafe { sys_x86_ipt_get_trace_data(descriptor, regs) }
}

// External function declarations
extern "C" {
    fn sys_x86_processor_trace_init();
    fn sys_x86_ipt_alloc_trace(mode: IptTraceMode, num_traces: u32) -> RxStatus;
    fn sys_x86_ipt_free_trace() -> RxStatus;
    fn sys_x86_ipt_start() -> RxStatus;
    fn sys_x86_ipt_stop() -> RxStatus;
    fn sys_x86_ipt_stage_trace_data(
        descriptor: RxItraceBufferDescriptor,
        regs: *const RxX86PtRegs,
    ) -> RxStatus;
    fn sys_x86_ipt_get_trace_data(
        descriptor: RxItraceBufferDescriptor,
        regs: *mut RxX86PtRegs,
    ) -> RxStatus;
}