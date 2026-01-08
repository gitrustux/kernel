// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Hypervisor Kernel Tracing
//!
//! This module provides kernel tracing support for hypervisor events.
//! It defines VCPU metadata and exit tracing for debugging and profiling.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Ktrace tag for VCPU metadata
pub const TAG_VCPU_META: u32 = 0x1000;

/// Ktrace tag for VCPU exit metadata
pub const TAG_VCPU_EXIT_META: u32 = 0x1001;

/// Ktrace tag for VCPU exit
pub const TAG_VCPU_EXIT: u32 = 0x1002;

/// VCPU metadata kinds
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuMeta {
    /// Waiting for interrupt
    Interrupt = 0,
    /// Waiting for port
    Port = 1,
}

impl VcpuMeta {
    /// Get the name for this metadata type
    pub fn name(&self) -> &'static str {
        match self {
            VcpuMeta::Interrupt => "wait:interrupt",
            VcpuMeta::Port => "wait:port",
        }
    }
}

/// VCPU exit reasons
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcpuExit {
    // ARM64 exits
    /// Underflow maintenance interrupt
    #[cfg(target_arch = "aarch64")]
    UnderflowMaintenanceInterrupt = 0,
    /// Physical interrupt
    #[cfg(target_arch = "aarch64")]
    PhysicalInterrupt = 1,
    /// WFI instruction
    #[cfg(target_arch = "aarch64")]
    WfiInstruction = 2,
    /// WFE instruction
    #[cfg(target_arch = "aarch64")]
    WfeInstruction = 3,
    /// SMC instruction
    #[cfg(target_arch = "aarch64")]
    SmcInstruction = 4,
    /// System instruction
    #[cfg(target_arch = "aarch64")]
    SystemInstruction = 5,
    /// Instruction abort
    #[cfg(target_arch = "aarch64")]
    InstructionAbort = 6,
    /// Data abort
    #[cfg(target_arch = "aarch64")]
    DataAbort = 7,

    // x86_64 exits
    /// External interrupt
    #[cfg(target_arch = "x86_64")]
    ExternalInterrupt = 0,
    /// Interrupt window
    #[cfg(target_arch = "x86_64")]
    InterruptWindow = 1,
    /// CPUID instruction
    #[cfg(target_arch = "x86_64")]
    Cpuid = 2,
    /// HLT instruction
    #[cfg(target_arch = "x86_64")]
    Hlt = 3,
    /// Control register access
    #[cfg(target_arch = "x86_64")]
    ControlRegisterAccess = 4,
    /// I/O instruction
    #[cfg(target_arch = "x86_64")]
    IoInstruction = 5,
    /// RDMSR instruction
    #[cfg(target_arch = "x86_64")]
    Rdmsr = 6,
    /// WRMSR instruction
    #[cfg(target_arch = "x86_64")]
    Wrmsr = 7,
    /// VM entry failure
    #[cfg(target_arch = "x86_64")]
    VmEntryFailure = 8,
    /// EPT violation
    #[cfg(target_arch = "x86_64")]
    EptViolation = 9,
    /// XSETBV instruction
    #[cfg(target_arch = "x86_64")]
    Xsetbv = 10,
    /// PAUSE instruction
    #[cfg(target_arch = "x86_64")]
    Pause = 11,
    /// VMCALL
    #[cfg(target_arch = "x86_64")]
    Vmcall = 12,

    /// Unknown exit
    Unknown = 100,
    /// Failure
    Failure = 101,
}

impl VcpuExit {
    /// Get the name for this exit reason
    pub fn name(&self) -> &'static str {
        match self {
            #[cfg(target_arch = "aarch64")]
            VcpuExit::UnderflowMaintenanceInterrupt => "exit:underflow_maintenance_interrupt",
            #[cfg(target_arch = "aarch64")]
            VcpuExit::PhysicalInterrupt => "exit:physical_interrupt",
            #[cfg(target_arch = "aarch64")]
            VcpuExit::WfiInstruction => "exit:wfi_instruction",
            #[cfg(target_arch = "aarch64")]
            VcpuExit::WfeInstruction => "exit:wfe_instruction",
            #[cfg(target_arch = "aarch64")]
            VcpuExit::SmcInstruction => "exit:smc_instruction",
            #[cfg(target_arch = "aarch64")]
            VcpuExit::SystemInstruction => "exit:system_instruction",
            #[cfg(target_arch = "aarch64")]
            VcpuExit::InstructionAbort => "exit:instruction_abort",
            #[cfg(target_arch = "aarch64")]
            VcpuExit::DataAbort => "exit:data_abort",

            #[cfg(target_arch = "x86_64")]
            VcpuExit::ExternalInterrupt => "exit:external_interrupt",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::InterruptWindow => "exit:interrupt_window",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::Cpuid => "exit:cpuid",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::Hlt => "exit:hlt",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::ControlRegisterAccess => "exit:control_register_access",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::IoInstruction => "exit:io_instruction",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::Rdmsr => "exit:rdmsr",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::Wrmsr => "exit:wrmsr",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::VmEntryFailure => "exit:vm_entry_failure",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::EptViolation => "exit:ept_violation",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::Xsetbv => "exit:xsetbv",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::Pause => "exit:pause",
            #[cfg(target_arch = "x86_64")]
            VcpuExit::Vmcall => "exit:vmcall",

            VcpuExit::Unknown => "exit:unknown",
            VcpuExit::Failure => "exit:failure",
        }
    }

    /// Get the number of metadata types
    pub const fn meta_count() -> u32 {
        2
    }

    /// Get the number of exit reasons
    pub const fn exit_count() -> u32 {
        #[cfg(target_arch = "aarch64")]
        return 8 + 2; // 8 ARM64 exits + 2 generic

        #[cfg(target_arch = "x86_64")]
        return 13 + 2; // 13 x86 exits + 2 generic
    }
}

/// Report VCPU metadata names to ktrace
///
/// This registers all VCPU metadata and exit names with the ktrace system.
pub fn ktrace_report_vcpu_meta() {
    println!("Ktrace: Reporting VCPU metadata");

    // Report VCPU metadata names
    for i in 0..VcpuMeta::meta_count() {
        let meta = match i {
            0 => VcpuMeta::Interrupt,
            1 => VcpuMeta::Port,
            _ => continue,
        };
        ktrace_name_etc(TAG_VCPU_META, i, 0, meta.name(), true);
    }

    // Report VCPU exit names
    for i in 0..VcpuExit::exit_count() {
        let exit = vcpu_exit_from_u32(i);
        ktrace_name_etc(TAG_VCPU_EXIT_META, i, 0, exit.name(), true);
    }
}

/// Trace a VCPU metadata event
///
/// # Arguments
///
/// * `tag` - Ktrace tag
/// * `meta` - VCPU metadata type
pub fn ktrace_vcpu(tag: u32, meta: VcpuMeta) {
    ktrace(tag, meta as u32, 0, 0, 0);
}

/// Trace a VCPU exit event
///
/// # Arguments
///
/// * `exit` - VCPU exit reason
/// * `exit_address` - Address where exit occurred
pub fn ktrace_vcpu_exit(exit: VcpuExit, exit_address: u64) {
    ktrace(
        TAG_VCPU_EXIT,
        exit as u32,
        exit_address as u32,
        (exit_address >> 32) as u32,
        0,
    );
}

/// Convert a u32 to VcpuExit
fn vcpu_exit_from_u32(value: u32) -> VcpuExit {
    match value {
        #[cfg(target_arch = "aarch64")]
        0 => VcpuExit::UnderflowMaintenanceInterrupt,
        #[cfg(target_arch = "aarch64")]
        1 => VcpuExit::PhysicalInterrupt,
        #[cfg(target_arch = "aarch64")]
        2 => VcpuExit::WfiInstruction,
        #[cfg(target_arch = "aarch64")]
        3 => VcpuExit::WfeInstruction,
        #[cfg(target_arch = "aarch64")]
        4 => VcpuExit::SmcInstruction,
        #[cfg(target_arch = "aarch64")]
        5 => VcpuExit::SystemInstruction,
        #[cfg(target_arch = "aarch64")]
        6 => VcpuExit::InstructionAbort,
        #[cfg(target_arch = "aarch64")]
        7 => VcpuExit::DataAbort,

        #[cfg(target_arch = "x86_64")]
        0 => VcpuExit::ExternalInterrupt,
        #[cfg(target_arch = "x86_64")]
        1 => VcpuExit::InterruptWindow,
        #[cfg(target_arch = "x86_64")]
        2 => VcpuExit::Cpuid,
        #[cfg(target_arch = "x86_64")]
        3 => VcpuExit::Hlt,
        #[cfg(target_arch = "x86_64")]
        4 => VcpuExit::ControlRegisterAccess,
        #[cfg(target_arch = "x86_64")]
        5 => VcpuExit::IoInstruction,
        #[cfg(target_arch = "x86_64")]
        6 => VcpuExit::Rdmsr,
        #[cfg(target_arch = "x86_64")]
        7 => VcpuExit::Wrmsr,
        #[cfg(target_arch = "x86_64")]
        8 => VcpuExit::VmEntryFailure,
        #[cfg(target_arch = "x86_64")]
        9 => VcpuExit::EptViolation,
        #[cfg(target_arch = "x86_64")]
        10 => VcpuExit::Xsetbv,
        #[cfg(target_arch = "x86_64")]
        11 => VcpuExit::Pause,
        #[cfg(target_arch = "x86_64")]
        12 => VcpuExit::Vmcall,

        100 => VcpuExit::Unknown,
        101 => VcpuExit::Failure,
        _ => VcpuExit::Unknown,
    }
}

/// Ktrace name registration
fn ktrace_name_etc(tag: u32, id: u32, extra: u32, name: &str, always: bool) {
    println!(
        "Ktrace: Name tag={:#x} id={} extra={} name={}",
        tag, id, extra, name
    );
    // TODO: Implement actual ktrace name registration
    let _ = always;
}

/// Ktrace event
fn ktrace(tag: u32, a: u32, b: u32, c: u32, d: u32) {
    println!(
        "Ktrace: Event tag={:#x} a={:#x} b={:#x} c={:#x} d={:#x}",
        tag, a, b, c, d
    );
    // TODO: Implement actual ktrace event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcpu_meta_names() {
        assert_eq!(VcpuMeta::Interrupt.name(), "wait:interrupt");
        assert_eq!(VcpuMeta::Port.name(), "wait:port");
    }

    #[test]
    fn test_vcpu_exit_names() {
        assert_eq!(VcpuExit::Unknown.name(), "exit:unknown");
        assert_eq!(VcpuExit::Failure.name(), "exit:failure");
    }

    #[test]
    fn test_vcpu_exit_counts() {
        assert!(VcpuExit::meta_count() >= 2);
        assert!(VcpuExit::exit_count() >= 2);
    }

    #[test]
    fn test_ktrace_vcpu() {
        // Just ensure it doesn't panic
        ktrace_vcpu(TAG_VCPU_META, VcpuMeta::Interrupt);
    }

    #[test]
    fn test_ktrace_vcpu_exit() {
        // Just ensure it doesn't panic
        ktrace_vcpu_exit(VcpuExit::Unknown, 0x1000);
    }
}
