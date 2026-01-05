// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::vmexit_priv::*;

use crate::bits;
use core::fmt;
use core::str;
use core::ptr;

use crate::arch::hypervisor;
use crate::arch::amd64::apic;
use crate::arch::amd64::feature;
use crate::arch::amd64::mmu;
use crate::arch::amd64::pvclock;
use crate::explicit_memory::bytes;
use crate::fbl::canary;
use crate::hypervisor::interrupt_tracker;
use crate::hypervisor::ktrace;
use crate::kernel::auto_lock;
use crate::lib::ktrace;
use crate::platform;
use crate::platform::pc::timer;
use crate::vm::fault;
use crate::vm::physmap;
use crate::vm::pmm;
use crate::rustux::syscalls::hypervisor as rx_hypervisor;
use crate::rustux::time;
use crate::rustux::types::*;

use crate::pvclock_priv::*;
use crate::vcpu_priv::*;

const LOCAL_TRACE: bool = false;

const LOCAL_APIC_PHYS_BASE: u64 = 
    apic::APIC_PHYS_BASE | apic::IA32_APIC_BASE_XAPIC_ENABLE | apic::IA32_APIC_BASE_X2APIC_ENABLE;

const X2APIC_MSR_BASE: u64 = 0x800;
const X2APIC_MSR_MAX: u64 = 0x83f;

const MISC_ENABLE_FAST_STRINGS: u64 = 1u64 << 0;

const FIRST_EXTENDED_STATE_COMPONENT: u32 = 2;
const LAST_EXTENDED_STATE_COMPONENT: u32 = 9;
// From Volume 1, Section 13.4.
const XSAVE_LEGACY_REGION_SIZE: u32 = 512;
const XSAVE_HEADER_SIZE: u32 = 64;

const HYP_VENDOR_ID: &[u8; 13] = b"KVMKVMKVM\0\0\0";
const HYP_VENDOR_ID_LENGTH: usize = 12;

const KVM_FEATURE_NO_IO_DELAY: u64 = 1u64 << 1;

extern "C" {
    fn x86_call_external_interrupt_handler(vector: u64);
}

pub struct ExitInfo {
    pub entry_failure: bool,
    pub exit_reason: ExitReason,
    pub exit_qualification: u64,
    pub exit_instruction_length: u32,
    pub guest_physical_address: u64,
    pub guest_rip: u64,
}

impl ExitInfo {
    pub fn new(vmcs: &AutoVmcs) -> Self {
        // From Volume 3, Section 26.7.
        let full_exit_reason = vmcs.read(VmcsField32::EXIT_REASON);
        let entry_failure = bits::BIT(full_exit_reason, 31) != 0;
        let exit_reason = ExitReason::from_u32(bits::BITS(full_exit_reason, 15, 0) as u32);

        let exit_qualification = vmcs.read(VmcsFieldXX::EXIT_QUALIFICATION);
        let exit_instruction_length = vmcs.read(VmcsField32::EXIT_INSTRUCTION_LENGTH);
        let guest_physical_address = vmcs.read(VmcsField64::GUEST_PHYSICAL_ADDRESS);
        let guest_rip = vmcs.read(VmcsFieldXX::GUEST_RIP);

        let exit_info = Self {
            entry_failure,
            exit_reason,
            exit_qualification,
            exit_instruction_length,
            guest_physical_address,
            guest_rip,
        };

        if exit_info.exit_reason == ExitReason::EXTERNAL_INTERRUPT ||
           exit_info.exit_reason == ExitReason::IO_INSTRUCTION {
            return exit_info;
        }

        trace!("entry failure: {}", entry_failure);
        trace!("exit reason: {:#x} ({})", exit_reason as u32, exit_reason_name(exit_reason));
        trace!("exit qualification: {:#x}", exit_qualification);
        trace!("exit instruction length: {:#x}", exit_instruction_length);
        trace!("guest activity state: {:#x}", vmcs.read(VmcsField32::GUEST_ACTIVITY_STATE));
        trace!("guest interruptibility state: {:#x}",
              vmcs.read(VmcsField32::GUEST_INTERRUPTIBILITY_STATE));
        trace!("guest physical address: {:#x}", guest_physical_address);
        trace!("guest linear address: {:#x}", vmcs.read(VmcsFieldXX::GUEST_LINEAR_ADDRESS));
        trace!("guest rip: {:#x}", guest_rip);

        exit_info
    }
}

pub struct ExitInterruptionInformation {
    pub vector: u8,
    pub interruption_type: InterruptionType,
    pub valid: bool,
}

impl ExitInterruptionInformation {
    pub fn new(vmcs: &AutoVmcs) -> Self {
        let int_info = vmcs.read(VmcsField32::EXIT_INTERRUPTION_INFORMATION);
        Self {
            vector: bits::BITS(int_info, 7, 0) as u8,
            interruption_type: InterruptionType::from_u8(bits::BITS_SHIFT(int_info, 10, 8) as u8),
            valid: bits::BIT(int_info, 31) != 0,
        }
    }
}

pub struct CrAccessInfo {
    pub cr_number: u8,
    pub access_type: CrAccessType,
    pub reg: u8,
}

impl CrAccessInfo {
    pub fn new(qualification: u64) -> Self {
        // From Volume 3, Table 27-3.
        Self {
            cr_number: bits::BITS(qualification, 3, 0) as u8,
            access_type: CrAccessType::from_u8(bits::BITS_SHIFT(qualification, 5, 4) as u8),
            reg: bits::BITS_SHIFT(qualification, 11, 8) as u8,
        }
    }
}

pub struct IoInfo {
    pub access_size: u8,
    pub input: bool,
    pub string: bool,
    pub repeat: bool,
    pub port: u16,
}

impl IoInfo {
    pub fn new(qualification: u64) -> Self {
        Self {
            access_size: bits::BITS(qualification, 2, 0) as u8 + 1,
            input: bits::BIT_SHIFT(qualification, 3) != 0,
            string: bits::BIT_SHIFT(qualification, 4) != 0,
            repeat: bits::BIT_SHIFT(qualification, 5) != 0,
            port: bits::BITS_SHIFT(qualification, 31, 16) as u16,
        }
    }
}

pub struct EptViolationInfo {
    pub read: bool,
    pub write: bool,
    pub instruction: bool,
}

impl EptViolationInfo {
    pub fn new(qualification: u64) -> Self {
        // From Volume 3C, Table 27-7.
        Self {
            read: bits::BIT(qualification, 0) != 0,
            write: bits::BIT(qualification, 1) != 0,
            instruction: bits::BIT(qualification, 2) != 0,
        }
    }
}

pub struct InterruptCommandRegister {
    pub destination: u32,
    pub destination_mode: InterruptDestinationMode,
    pub delivery_mode: InterruptDeliveryMode,
    pub destination_shorthand: InterruptDestinationShorthand,
    pub vector: u8,
}

impl InterruptCommandRegister {
    pub fn new(hi: u32, lo: u32) -> Self {
        Self {
            destination: hi,
            destination_mode: if bits::BIT_SHIFT(lo, 11) != 0 {
                InterruptDestinationMode::LOGICAL
            } else {
                InterruptDestinationMode::PHYSICAL
            },
            delivery_mode: InterruptDeliveryMode::from_u8(bits::BITS_SHIFT(lo, 10, 8) as u8),
            destination_shorthand: InterruptDestinationShorthand::from_u8(bits::BITS_SHIFT(lo, 19, 18) as u8),
            vector: bits::BITS(lo, 7, 0) as u8,
        }
    }
}

pub struct VmCallInfo {
    pub type_: VmCallType,
    pub arg: [u64; 4],
}

impl VmCallInfo {
    pub fn new(guest_state: &GuestState) -> Self {
        // ABI is documented in Linux kernel documentation, see
        // Documents/virtual/kvm/hypercalls.txt
        Self {
            type_: VmCallType::from_u64(guest_state.rax),
            arg: [guest_state.rbx, guest_state.rcx, guest_state.rdx, guest_state.rsi],
        }
    }
}

fn next_rip(exit_info: &ExitInfo, vmcs: &mut AutoVmcs) {
    vmcs.write(VmcsFieldXX::GUEST_RIP, exit_info.guest_rip + exit_info.exit_instruction_length as u64);

    // Clear any flags blocking interrupt injection for a single instruction.
    let guest_interruptibility = vmcs.read(VmcsField32::GUEST_INTERRUPTIBILITY_STATE);
    let new_interruptibility = guest_interruptibility &
                              !(INTERRUPTIBILITY_STI_BLOCKING | INTERRUPTIBILITY_MOV_SS_BLOCKING);
    if new_interruptibility != guest_interruptibility {
        vmcs.write(VmcsField32::GUEST_INTERRUPTIBILITY_STATE, new_interruptibility);
    }
}

fn handle_external_interrupt(vmcs: &mut AutoVmcs) -> rx_status_t {
    let int_info = ExitInterruptionInformation::new(vmcs);
    debug_assert!(int_info.valid);
    debug_assert!(int_info.interruption_type == InterruptionType::EXTERNAL_INTERRUPT);
    unsafe { x86_call_external_interrupt_handler(int_info.vector as u64) };
    vmcs.invalidate();

    // If we are receiving an external interrupt because the thread is being
    // killed, we should exit with an error.
    if unsafe { get_current_thread().signals & THREAD_SIGNAL_KILL != 0 } {
        rx_ERR_CANCELED
    } else {
        rx_OK
    }
}

fn handle_interrupt_window(vmcs: &mut AutoVmcs, local_apic_state: &mut LocalApicState) -> rx_status_t {
    vmcs.interrupt_window_exiting(false);
    rx_OK
}

// From Volume 2, Section 3.2, Table 3-8  "Processor Extended State Enumeration
// Main Leaf (EAX = 0DH, ECX = 0)".
//
// Bits 31-00: Maximum size (bytes, from the beginning of the XSAVE/XRSTOR save
// area) required by enabled features in XCR0. May be different than ECX if some
// features at the end of the XSAVE save area are not enabled.
fn compute_xsave_size(guest_xcr0: u64, xsave_size: &mut u32) -> rx_status_t {
    *xsave_size = XSAVE_LEGACY_REGION_SIZE + XSAVE_HEADER_SIZE;
    for i in FIRST_EXTENDED_STATE_COMPONENT..=LAST_EXTENDED_STATE_COMPONENT {
        if guest_xcr0 & (1 << i) == 0 {
            continue;
        }
        
        let mut leaf = cpuid_leaf::default();
        if !x86_get_cpuid_subleaf(X86_CPUID_XSAVE, i, &mut leaf) {
            return rx_ERR_INTERNAL;
        }
        
        if leaf.a == 0 && leaf.b == 0 && leaf.c == 0 && leaf.d == 0 {
            continue;
        }
        
        let component_offset = leaf.b;
        let component_size = leaf.a;
        *xsave_size = component_offset + component_size;
    }
    
    rx_OK
}

fn handle_cpuid(exit_info: &ExitInfo, vmcs: &mut AutoVmcs, guest_state: &mut GuestState) -> rx_status_t {
    let leaf = guest_state.rax as u32;
    let subleaf = guest_state.rcx as u32;

    next_rip(exit_info, vmcs);
    
    match leaf {
        X86_CPUID_BASE | X86_CPUID_EXT_BASE => {
            let mut eax = 0;
            let mut ebx = 0;
            let mut ecx = 0;
            let mut edx = 0;
            
            cpuid(leaf, &mut eax, &mut ebx, &mut ecx, &mut edx);
            
            guest_state.rax = eax as u64;
            guest_state.rbx = ebx as u64;
            guest_state.rcx = ecx as u64;
            guest_state.rdx = edx as u64;
            
            rx_OK
        }
        X86_CPUID_BASE + 1..=MAX_SUPPORTED_CPUID | X86_CPUID_EXT_BASE + 1..=MAX_SUPPORTED_CPUID_EXT => {
            let mut eax = 0;
            let mut ebx = 0;
            let mut ecx = 0;
            let mut edx = 0;
            
            cpuid_c(leaf, subleaf, &mut eax, &mut ebx, &mut ecx, &mut edx);
            
            guest_state.rax = eax as u64;
            guest_state.rbx = ebx as u64;
            guest_state.rcx = ecx as u64;
            guest_state.rdx = edx as u64;
            
            match leaf {
                X86_CPUID_MODEL_FEATURES => {
                    // Override the initial local APIC ID. From Vol 2, Table 3-8.
                    guest_state.rbx &= !(0xff << 24);
                    guest_state.rbx |= (vmcs.read(VmcsField16::VPID) as u64 - 1) << 24;
                    // Enable the hypervisor bit.
                    guest_state.rcx |= 1u64 << X86_FEATURE_HYPERVISOR.bit;
                    // Enable the x2APIC bit.
                    guest_state.rcx |= 1u64 << X86_FEATURE_X2APIC.bit;
                    // Disable the VMX bit.
                    guest_state.rcx &= !(1u64 << X86_FEATURE_VMX.bit);
                    // Disable the PDCM bit.
                    guest_state.rcx &= !(1u64 << X86_FEATURE_PDCM.bit);
                    // Disable MONITOR/MWAIT.
                    guest_state.rcx &= !(1u64 << X86_FEATURE_MON.bit);
                    // Disable THERM_INTERRUPT and THERM_STATUS MSRs
                    guest_state.rcx &= !(1u64 << X86_FEATURE_TM2.bit);
                    // Enable the SEP (SYSENTER support).
                    guest_state.rdx |= 1u64 << X86_FEATURE_SEP.bit;
                    // Disable the Thermal Monitor bit.
                    guest_state.rdx &= !(1u64 << X86_FEATURE_TM.bit);
                    // Disable the THERM_CONTROL_MSR bit.
                    guest_state.rdx &= !(1u64 << X86_FEATURE_ACPI.bit);
                }
                X86_CPUID_TOPOLOGY => {
                    guest_state.rdx = vmcs.read(VmcsField16::VPID) as u64 - 1;
                }
                X86_CPUID_XSAVE => {
                    if subleaf == 0 {
                        let mut xsave_size = 0;
                        let status = compute_xsave_size(guest_state.xcr0, &mut xsave_size);
                        if status != rx_OK {
                            return status;
                        }
                        guest_state.rbx = xsave_size as u64;
                    } else if subleaf == 1 {
                        guest_state.rax &= !(1u64 << 3);
                    }
                }
                X86_CPUID_THERMAL_AND_POWER => {
                    // Disable the performance energy bias bit.
                    guest_state.rcx &= !(1u64 << X86_FEATURE_PERF_BIAS.bit);
                    // Disable the hardware coordination feedback bit.
                    guest_state.rcx &= !(1u64 << X86_FEATURE_HW_FEEDBACK.bit);
                    guest_state.rax &= !(
                        // Disable Digital Thermal Sensor
                        1u64 << X86_FEATURE_DTS.bit |
                        // Disable Package Thermal Status MSR.
                        1u64 << X86_FEATURE_PTM.bit |
                        // Disable THERM_STATUS MSR bits 10/11 & THERM_INTERRUPT MSR bit 24
                        1u64 << X86_FEATURE_PTM.bit |
                        // Disable HWP MSRs.
                        1u64 << X86_FEATURE_HWP.bit |
                        1u64 << X86_FEATURE_HWP_NOT.bit |
                        1u64 << X86_FEATURE_HWP_ACT.bit |
                        1u64 << X86_FEATURE_HWP_PREF.bit);
                }
                X86_CPUID_PERFORMANCE_MONITORING => {
                    // Disable all performance monitoring.
                    // 31-07 = Reserved 0, 06-00 = 1 if event is not available.
                    let performance_monitoring_no_events = 0b1111111u32;
                    guest_state.rax = 0;
                    guest_state.rbx = performance_monitoring_no_events as u64;
                    guest_state.rcx = 0;
                    guest_state.rdx = 0;
                }
                X86_CPUID_MON => {
                    // MONITOR/MWAIT are not implemented.
                    guest_state.rax = 0;
                    guest_state.rbx = 0;
                    guest_state.rcx = 0;
                    guest_state.rdx = 0;
                }
                X86_CPUID_EXTENDED_FEATURE_FLAGS => {
                    // It's possible when running under KVM in nVMX mode, that host
                    // CPUID indicates that invpcid is supported but VMX doesn't allow
                    // to enable INVPCID bit in secondary processor based controls.
                    // Therefore explicitly clear INVPCID bit in CPUID if the VMX flag
                    // wasn't set.
                    if (vmcs.read(VmcsField32::PROCBASED_CTLS2) & PROCBASED_CTLS2_INVPCID) == 0 {
                        guest_state.rbx &= !(1u64 << X86_FEATURE_INVPCID.bit);
                    }
                    // Disable the Processor Trace bit.
                    guest_state.rbx &= !(1u64 << X86_FEATURE_PT.bit);
                    // Disable:
                    //  * Indirect Branch Prediction Barrier bit
                    //  * Single Thread Indirect Branch Predictors bit
                    //  * Speculative Store Bypass Disable bit
                    // These imply support for the IA32_SPEC_CTRL and IA32_PRED_CMD
                    // MSRs, which are not implemented.
                    guest_state.rdx &= !(
                        1u64 << X86_FEATURE_IBRS_IBPB.bit |
                        1u64 << X86_FEATURE_STIBP.bit |
                        1u64 << X86_FEATURE_SSBD.bit);
                }
                _ => {}
            }
            rx_OK
        }
        X86_CPUID_HYP_VENDOR => {
            // This leaf is commonly used to identify a hypervisor via ebx:ecx:edx.
            let regs = unsafe { ptr::read(HYP_VENDOR_ID.as_ptr() as *const [u32; 4]) };
            // Since Rustux hypervisor disguises itself as KVM, it needs to return
            // in EAX max CPUID function supported by hypervisor. Zero in EAX
            // should be interpreted as 0x40000001. Details are available in the
            // Linux kernel documentation (Documentation/virtual/kvm/cpuid.txt).
            guest_state.rax = X86_CPUID_KVM_FEATURES as u64;
            guest_state.rbx = regs[0] as u64;
            guest_state.rcx = regs[1] as u64;
            guest_state.rdx = regs[2] as u64;
            rx_OK
        }
        X86_CPUID_KVM_FEATURES => {
            // We support KVM clock.
            guest_state.rax = KVM_FEATURE_CLOCK_SOURCE_OLD | KVM_FEATURE_CLOCK_SOURCE | KVM_FEATURE_NO_IO_DELAY;
            guest_state.rbx = 0;
            guest_state.rcx = 0;
            guest_state.rdx = 0;
            rx_OK
        }
        // From Volume 2A, CPUID instruction reference. If the EAX value is outside
        // the range recognized by CPUID then the information for the highest
        // supported base information leaf is returned. Any value in ECX is
        // honored.
        _ => {
            let mut eax = 0;
            let mut ebx = 0;
            let mut ecx = 0;
            let mut edx = 0;
            
            cpuid_c(MAX_SUPPORTED_CPUID, subleaf, &mut eax, &mut ebx, &mut ecx, &mut edx);
            
            guest_state.rax = eax as u64;
            guest_state.rbx = ebx as u64;
            guest_state.rcx = ecx as u64;
            guest_state.rdx = edx as u64;
            
            rx_OK
        }
    }
}

fn handle_hlt(exit_info: &ExitInfo, vmcs: &mut AutoVmcs, local_apic_state: &mut LocalApicState) -> rx_status_t {
    next_rip(exit_info, vmcs);
    local_apic_state.interrupt_tracker.wait(rx_TIME_INFINITE, vmcs)
}

fn handle_cr0_write(vmcs: &mut AutoVmcs, guest_state: &mut GuestState, val: u64) -> rx_status_t {
    // Ensure that CR0.NE is set since it is set in X86_MSR_IA32_VMX_CR0_FIXED1.
    let cr0 = val | X86_CR0_NE;
    if cr0_is_invalid(vmcs, cr0) {
        return rx_ERR_INVALID_ARGS;
    }
    
    vmcs.write(VmcsFieldXX::GUEST_CR0, cr0);
    
    // From Volume 3, Section 26.3.1.1: If CR0.PG and EFER.LME are set then EFER.LMA and the IA-32e
    // mode guest entry control must also be set.
    let mut efer = vmcs.read(VmcsField64::GUEST_IA32_EFER);
    
    if !((efer & X86_EFER_LME != 0) && (cr0 & X86_CR0_PG != 0)) {
        return rx_OK;
    }
    
    vmcs.write(VmcsField64::GUEST_IA32_EFER, efer | X86_EFER_LMA);
    
    vmcs.set_control(
        VmcsField32::ENTRY_CTLS,
        read_msr(X86_MSR_IA32_VMX_TRUE_ENTRY_CTLS),
        read_msr(X86_MSR_IA32_VMX_ENTRY_CTLS),
        ENTRY_CTLS_IA32E_MODE,
        0
    )
}

fn register_value(vmcs: &mut AutoVmcs, guest_state: &GuestState, register_id: u8, out: &mut u64) -> rx_status_t {
    match register_id {
        // From Intel Volume 3, Table 27-3.
        0 => {
            *out = guest_state.rax;
            rx_OK
        }
        1 => {
            *out = guest_state.rcx;
            rx_OK
        }
        2 => {
            *out = guest_state.rdx;
            rx_OK
        }
        3 => {
            *out = guest_state.rbx;
            rx_OK
        }
        4 => {
            *out = vmcs.read(VmcsFieldXX::GUEST_RSP);
            rx_OK
        }
        5 => {
            *out = guest_state.rbp;
            rx_OK
        }
        6 => {
            *out = guest_state.rsi;
            rx_OK
        }
        7 => {
            *out = guest_state.rdi;
            rx_OK
        }
        8 => {
            *out = guest_state.r8;
            rx_OK
        }
        9 => {
            *out = guest_state.r9;
            rx_OK
        }
        10 => {
            *out = guest_state.r10;
            rx_OK
        }
        11 => {
            *out = guest_state.r11;
            rx_OK
        }
        12 => {
            *out = guest_state.r12;
            rx_OK
        }
        13 => {
            *out = guest_state.r13;
            rx_OK
        }
        14 => {
            *out = guest_state.r14;
            rx_OK
        }
        15 => {
            *out = guest_state.r15;
            rx_OK
        }
        _ => rx_ERR_INVALID_ARGS
    }
}

fn handle_control_register_access(exit_info: &ExitInfo, vmcs: &mut AutoVmcs, guest_state: &mut GuestState) -> rx_status_t {
    let cr_access_info = CrAccessInfo::new(exit_info.exit_qualification);
    
    match cr_access_info.access_type {
        CrAccessType::MOV_TO_CR => {
            // Handle CR0 only.
            if cr_access_info.cr_number != 0 {
                return rx_ERR_NOT_SUPPORTED;
            }
            
            let mut val = 0;
            let status = register_value(vmcs, guest_state, cr_access_info.reg, &mut val);
            
            if status != rx_OK {
                return status;
            }
            
            let status = handle_cr0_write(vmcs, guest_state, val);
            
            if status != rx_OK {
                return status;
            }
            
            next_rip(exit_info, vmcs);
            rx_OK
        }
        _ => rx_ERR_NOT_SUPPORTED
    }
}

fn handle_io_instruction(
    exit_info: &ExitInfo,
    vmcs: &mut AutoVmcs,
    guest_state: &mut GuestState,
    traps: &mut hypervisor::TrapMap,
    packet: &mut rx_port_packet_t
) -> rx_status_t {
    let io_info = IoInfo::new(exit_info.exit_qualification);
    
    if io_info.string || io_info.repeat {
        dprintf!(CRITICAL, "Unsupported IO instruction\n");
        return rx_ERR_NOT_SUPPORTED;
    }

    let mut trap = hypervisor::Trap::default();
    let status = traps.find_trap(rx_GUEST_TRAP_IO, io_info.port as u64, &mut trap);
    
    if status != rx_OK {
        dprintf!(CRITICAL, "Unhandled IO port {} {:#x}\n",
                if io_info.input { "in" } else { "out" }, io_info.port);
        return status;
    }
    
    next_rip(exit_info, vmcs);

    unsafe { libc::memset(packet as *mut rx_port_packet_t as *mut libc::c_void, 0, core::mem::size_of::<rx_port_packet_t>()) };
    
    packet.key = trap.key();
    packet.type_ = rx_PKT_TYPE_GUEST_IO;
    packet.guest_io.port = io_info.port;
    packet.guest_io.access_size = io_info.access_size;
    packet.guest_io.input = io_info.input;
    
    if io_info.input {
        // From Volume 1, Section 3.4.1.1: 32-bit operands generate a 32-bit
        // result, zero-extended to a 64-bit result in the destination general-
        // purpose register.
        if io_info.access_size == 4 {
            guest_state.rax = 0;
        }
    } else {
        unsafe {
            libc::memcpy(
                packet.guest_io.data.as_mut_ptr() as *mut libc::c_void,
                &guest_state.rax as *const u64 as *const libc::c_void,
                io_info.access_size as usize
            );
        }
        
        if trap.has_port() {
            return trap.queue(*packet, vmcs);
        }
        // If there was no port for the range, then return to user-space.
    }

    rx_ERR_NEXT
}

fn handle_apic_rdmsr(
    exit_info: &ExitInfo,
    vmcs: &mut AutoVmcs,
    guest_state: &mut GuestState,
    local_apic_state: &mut LocalApicState
) -> rx_status_t {
        match unsafe { std::mem::transmute::<u64, X2ApicMsr>(guest_state.rcx) } {
            X2ApicMsr::ID => {
                next_rip(exit_info, vmcs);
                guest_state.rax = vmcs.read(VmcsField16::VPID) as u64 - 1;
                rx_OK
            }
            X2ApicMsr::VERSION => {
                next_rip(exit_info, vmcs);
                // We choose 15H as it causes us to be seen as a modern APIC by Linux,
                // and is the highest non-reserved value. See Volume 3 Section 10.4.8.
                const VERSION: u32 = 0x15;
                const MAX_LVT_ENTRY: u32 = 0x6; // LVT entries minus 1.
                const EOI_SUPPRESSION: u32 = 0; // Disable support for EOI-broadcast suppression.
                guest_state.rax = VERSION | (MAX_LVT_ENTRY << 16) | (EOI_SUPPRESSION << 24);
                rx_OK
            }
            X2ApicMsr::SVR => {
                // Spurious interrupt vector resets to 0xff. See Volume 3 Section 10.12.5.1.
                next_rip(exit_info, vmcs);
                guest_state.rax = 0xff;
                rx_OK
            }
            X2ApicMsr::TPR | 
            X2ApicMsr::LDR |
            X2ApicMsr::ISR_31_0..=X2ApicMsr::ISR_255_224 |
            X2ApicMsr::TMR_31_0..=X2ApicMsr::TMR_255_224 |
            X2ApicMsr::IRR_31_0..=X2ApicMsr::IRR_255_224 |
            X2ApicMsr::ESR |
            X2ApicMsr::LVT_MONITOR => {
                // These registers reset to 0. See Volume 3 Section 10.12.5.1.
                next_rip(exit_info, vmcs);
                guest_state.rax = 0;
                rx_OK
            }
            X2ApicMsr::LVT_LINT0 |
            X2ApicMsr::LVT_LINT1 |
            X2ApicMsr::LVT_THERMAL_SENSOR |
            X2ApicMsr::LVT_CMCI => {
                // LVT registers reset with the mask bit set. See Volume 3 Section 10.12.5.1.
                next_rip(exit_info, vmcs);
                guest_state.rax = LVT_MASKED;
                rx_OK
            }
            X2ApicMsr::LVT_TIMER => {
                next_rip(exit_info, vmcs);
                guest_state.rax = local_apic_state.lvt_timer as u64;
                rx_OK
            }
            _ => {
                // Issue a general protection fault for write only and unimplemented
                // registers.
                dprintf!(INFO, "Unhandled x2APIC rdmsr {:#x}\n", guest_state.rcx);
                local_apic_state.interrupt_tracker.virtual_interrupt(X86_INT_GP_FAULT);
                rx_OK
            }
        }
    }
    
    fn handle_rdmsr(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        guest_state: &mut GuestState,
        local_apic_state: &mut LocalApicState
    ) -> rx_status_t {
        // On execution of rdmsr, rcx specifies the MSR and the value is loaded into edx:eax.
        match guest_state.rcx as u32 {
            X86_MSR_IA32_APIC_BASE => {
                next_rip(exit_info, vmcs);
                guest_state.rax = LOCAL_APIC_PHYS_BASE;
                if vmcs.read(VmcsField16::VPID) == 1 {
                    guest_state.rax |= apic::IA32_APIC_BASE_BSP;
                }
                guest_state.rdx = 0;
                rx_OK
            }
            // From Volume 4, Section 2.1, Table 2-2: For now, only enable fast strings.
            X86_MSR_IA32_MISC_ENABLE => {
                next_rip(exit_info, vmcs);
                guest_state.rax = read_msr(X86_MSR_IA32_MISC_ENABLE) & MISC_ENABLE_FAST_STRINGS;
                guest_state.rdx = 0;
                rx_OK
            }
            X86_MSR_DRAM_ENERGY_STATUS |
            X86_MSR_DRAM_POWER_LIMIT |
            // From Volume 3, Section 28.2.6.2: The MTRRs have no effect on the memory
            // type used for an access to a guest-physical address.
            X86_MSR_IA32_MTRRCAP |
            X86_MSR_IA32_MTRR_DEF_TYPE |
            X86_MSR_IA32_MTRR_FIX64K_00000 |
            X86_MSR_IA32_MTRR_FIX16K_80000..=X86_MSR_IA32_MTRR_FIX16K_A0000 |
            X86_MSR_IA32_MTRR_FIX4K_C0000..=X86_MSR_IA32_MTRR_FIX4K_F8000 |
            X86_MSR_IA32_MTRR_PHYSBASE0..=X86_MSR_IA32_MTRR_PHYSMASK9 |
            // From Volume 3, Section 9.11.4: For now, 0.
            X86_MSR_IA32_PLATFORM_ID |
            // From Volume 3, Section 9.11.7: 0 indicates no microcode update is loaded.
            X86_MSR_IA32_BIOS_SIGN_ID |
            // From Volume 3, Section 15.3.1: 0 indicates that our machine has no
            // checking capabilities.
            X86_MSR_IA32_MCG_CAP |
            X86_MSR_IA32_MCG_STATUS |
            X86_MSR_IA32_TEMPERATURE_TARGET |
            X86_MSR_PKG_ENERGY_STATUS |
            X86_MSR_PLATFORM_ENERGY_COUNTER |
            X86_MSR_PLATFORM_POWER_LIMIT |
            X86_MSR_PP0_ENERGY_STATUS |
            X86_MSR_PP0_POWER_LIMIT |
            X86_MSR_PP1_ENERGY_STATUS |
            X86_MSR_PP1_POWER_LIMIT |
            X86_MSR_RAPL_POWER_UNIT => {
                next_rip(exit_info, vmcs);
                guest_state.rax = 0;
                guest_state.rdx = 0;
                rx_OK
            }
            msr @ X2APIC_MSR_BASE..=X2APIC_MSR_MAX => {
                guest_state.rcx = msr as u64;
                handle_apic_rdmsr(exit_info, vmcs, guest_state, local_apic_state)
            }
            _ => {
                dprintf!(INFO, "Unhandled rdmsr {:#x}\n", guest_state.rcx);
                local_apic_state.interrupt_tracker.virtual_interrupt(X86_INT_GP_FAULT);
                rx_OK
            }
        }
    }
    
    fn lvt_deadline(local_apic_state: &LocalApicState) -> rx_time_t {
        if (local_apic_state.lvt_timer & LVT_TIMER_MODE_MASK) != LVT_TIMER_MODE_ONESHOT &&
           (local_apic_state.lvt_timer & LVT_TIMER_MODE_MASK) != LVT_TIMER_MODE_PERIODIC {
            return 0;
        }
        
        let shift = bits::BITS_SHIFT(local_apic_state.lvt_divide_config, 1, 0) |
                    (bits::BIT_SHIFT(local_apic_state.lvt_divide_config, 3) << 2);
        let divisor_shift = (shift + 1) & 7;
        let duration = ticks_to_nanos(local_apic_state.lvt_initial_count << divisor_shift);
        
        rx_time_add_duration(current_time(), duration)
    }
    
    fn update_timer(local_apic_state: &mut LocalApicState, deadline: rx_time_t) {
        timer_cancel(&mut local_apic_state.timer);
        if deadline > 0 {
            timer_set_oneshot(&mut local_apic_state.timer, deadline, deadline_callback, local_apic_state as *mut _ as *mut libc::c_void);
        }
    }
    
    fn deadline_callback(_timer: *mut timer_t, _now: rx_time_t, arg: *mut libc::c_void) {
        let local_apic_state = unsafe { &mut *(arg as *mut LocalApicState) };
        if local_apic_state.lvt_timer & LVT_MASKED != 0 {
            return;
        }
        
        if (local_apic_state.lvt_timer & LVT_TIMER_MODE_MASK) == LVT_TIMER_MODE_PERIODIC {
            update_timer(local_apic_state, lvt_deadline(local_apic_state));
        }
        
        let vector = local_apic_state.lvt_timer & LVT_TIMER_VECTOR_MASK;
        local_apic_state.interrupt_tracker.virtual_interrupt(vector);
    }
    
    fn ipi_target_mask(icr: &InterruptCommandRegister, self_id: u16) -> u64 {
        match icr.destination_shorthand {
            InterruptDestinationShorthand::NO_SHORTHAND => 1u64 << icr.destination,
            InterruptDestinationShorthand::SELF => 1u64 << (self_id - 1),
            InterruptDestinationShorthand::ALL_INCLUDING_SELF => u64::MAX,
            InterruptDestinationShorthand::ALL_EXCLUDING_SELF => !(1u64 << (self_id - 1)),
        }
    }
    
    fn handle_ipi(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        guest_state: &mut GuestState,
        packet: &mut rx_port_packet_t
    ) -> rx_status_t {
        if guest_state.rax > u32::MAX as u64 || guest_state.rdx > u32::MAX as u64 {
            return rx_ERR_INVALID_ARGS;
        }
        
        let icr = InterruptCommandRegister::new(
            guest_state.rdx as u32,
            guest_state.rax as u32
        );
        
        if icr.destination_mode == InterruptDestinationMode::LOGICAL {
            dprintf!(CRITICAL, "Logical IPI destination mode is not supported\n");
            return rx_ERR_NOT_SUPPORTED;
        }
        
        match icr.delivery_mode {
            InterruptDeliveryMode::FIXED => {
                let self_id = vmcs.read(VmcsField16::VPID);
                
                unsafe { libc::memset(packet as *mut rx_port_packet_t as *mut libc::c_void, 0, core::mem::size_of::<rx_port_packet_t>()) };
                
                packet.type_ = rx_PKT_TYPE_GUEST_VCPU;
                packet.guest_vcpu.type_ = rx_PKT_GUEST_VCPU_INTERRUPT;
                packet.guest_vcpu.interrupt.mask = ipi_target_mask(&icr, self_id);
                packet.guest_vcpu.interrupt.vector = icr.vector;
                
                next_rip(exit_info, vmcs);
                rx_ERR_NEXT
            }
            InterruptDeliveryMode::INIT => {
                // Ignore INIT IPIs, we only need STARTUP to bring up a VCPU.
                next_rip(exit_info, vmcs);
                rx_OK
            }
            InterruptDeliveryMode::STARTUP => {
                unsafe { libc::memset(packet as *mut rx_port_packet_t as *mut libc::c_void, 0, core::mem::size_of::<rx_port_packet_t>()) };
                
                packet.type_ = rx_PKT_TYPE_GUEST_VCPU;
                packet.guest_vcpu.type_ = rx_PKT_GUEST_VCPU_STARTUP;
                packet.guest_vcpu.startup.id = icr.destination;
                packet.guest_vcpu.startup.entry = (icr.vector as u64) << 12;
                
                next_rip(exit_info, vmcs);
                rx_ERR_NEXT
            }
            _ => {
                dprintf!(CRITICAL, "Unsupported IPI delivery mode {:#x}\n",
                        icr.delivery_mode as u8);
                rx_ERR_NOT_SUPPORTED
            }
        }
    }
    
    fn handle_apic_wrmsr(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        guest_state: &mut GuestState,
        local_apic_state: &mut LocalApicState,
        packet: &mut rx_port_packet_t
    ) -> rx_status_t {
        match unsafe { std::mem::transmute::<u64, X2ApicMsr>(guest_state.rcx) } {
            X2ApicMsr::EOI | X2ApicMsr::ESR => {
                if guest_state.rax != 0 {
                    // Non-zero writes to EOI and ESR cause GP fault. See Volume 3 Section 10.12.1.2.
                    local_apic_state.interrupt_tracker.virtual_interrupt(X86_INT_GP_FAULT);
                    return rx_OK;
                }
                // Fall through to default processing
                next_rip(exit_info, vmcs);
                rx_OK
            }
            X2ApicMsr::TPR |
            X2ApicMsr::SVR |
            X2ApicMsr::LVT_MONITOR |
            X2ApicMsr::LVT_ERROR |
            X2ApicMsr::LVT_LINT0 |
            X2ApicMsr::LVT_LINT1 |
            X2ApicMsr::LVT_THERMAL_SENSOR |
            X2ApicMsr::LVT_CMCI => {
                if guest_state.rdx != 0 || guest_state.rax > u32::MAX as u64 {
                    return rx_ERR_INVALID_ARGS;
                }
                next_rip(exit_info, vmcs);
                rx_OK
            }
            X2ApicMsr::LVT_TIMER => {
                if guest_state.rax > u32::MAX as u64 {
                    return rx_ERR_INVALID_ARGS;
                }
                if (guest_state.rax & LVT_TIMER_MODE_MASK) == LVT_TIMER_MODE_RESERVED {
                    return rx_ERR_INVALID_ARGS;
                }
                next_rip(exit_info, vmcs);
                local_apic_state.lvt_timer = guest_state.rax as u32;
                update_timer(local_apic_state, lvt_deadline(local_apic_state));
                rx_OK
            }
            X2ApicMsr::INITIAL_COUNT => {
                if guest_state.rax > u32::MAX as u64 {
                    return rx_ERR_INVALID_ARGS;
                }
                next_rip(exit_info, vmcs);
                local_apic_state.lvt_initial_count = guest_state.rax as u32;
                update_timer(local_apic_state, lvt_deadline(local_apic_state));
                rx_OK
            }
            X2ApicMsr::DCR => {
                if guest_state.rax > u32::MAX as u64 {
                    return rx_ERR_INVALID_ARGS;
                }
                next_rip(exit_info, vmcs);
                local_apic_state.lvt_divide_config = guest_state.rax as u32;
                update_timer(local_apic_state, lvt_deadline(local_apic_state));
                rx_OK
            }
            X2ApicMsr::SELF_IPI => {
                next_rip(exit_info, vmcs);
                let vector = (guest_state.rax & 0xFF) as u8;
                local_apic_state.interrupt_tracker.virtual_interrupt(vector);
                rx_OK
            }
            X2ApicMsr::ICR => {
                handle_ipi(exit_info, vmcs, guest_state, packet)
            }
            _ => {
                // Issue a general protection fault for read only and unimplemented
                // registers.
                dprintf!(INFO, "Unhandled x2APIC wrmsr {:#x}\n", guest_state.rcx);
                local_apic_state.interrupt_tracker.virtual_interrupt(X86_INT_GP_FAULT);
                rx_OK
            }
        }
    }
    
    fn handle_kvm_wrmsr(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        guest_state: &mut GuestState,
        local_apic_state: &mut LocalApicState,
        pvclock: &mut PvClockState,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace
    ) -> rx_status_t {
        let guest_paddr = bits::BITS(guest_state.rax, 31, 0) | (bits::BITS(guest_state.rdx, 31, 0) << 32);
    
        next_rip(exit_info, vmcs);
        match guest_state.rcx as u32 {
            KVM_SYSTEM_TIME_MSR_OLD | KVM_SYSTEM_TIME_MSR => {
                if (guest_paddr & 1) != 0 {
                    pvclock_reset_clock(pvclock, gpas, guest_paddr & !1)
                } else {
                    pvclock_stop_clock(pvclock);
                    rx_OK
                }
            }
            KVM_BOOT_TIME_OLD | KVM_BOOT_TIME => {
                pvclock_update_boot_time(gpas, guest_paddr)
            }
            _ => {
                local_apic_state.interrupt_tracker.virtual_interrupt(X86_INT_GP_FAULT);
                rx_OK
            }
        }
    }
    
    fn handle_wrmsr(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        guest_state: &mut GuestState,
        local_apic_state: &mut LocalApicState,
        pvclock: &mut PvClockState,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace,
        packet: &mut rx_port_packet_t
    ) -> rx_status_t {
        // On execution of wrmsr, rcx specifies the MSR and edx:eax contains the value to be written.
        match guest_state.rcx as u32 {
            X86_MSR_IA32_APIC_BASE => {
                if guest_state.rdx != 0 {
                    return rx_ERR_INVALID_ARGS;
                }
                if (guest_state.rax & !apic::IA32_APIC_BASE_BSP) != LOCAL_APIC_PHYS_BASE {
                    return rx_ERR_INVALID_ARGS;
                }
                next_rip(exit_info, vmcs);
                rx_OK
            }
            // See note in handle_rdmsr.
            X86_MSR_IA32_MTRRCAP |
            X86_MSR_IA32_MTRR_DEF_TYPE |
            X86_MSR_IA32_MTRR_FIX64K_00000 |
            X86_MSR_IA32_MTRR_FIX16K_80000..=X86_MSR_IA32_MTRR_FIX16K_A0000 |
            X86_MSR_IA32_MTRR_FIX4K_C0000..=X86_MSR_IA32_MTRR_FIX4K_F8000 |
            X86_MSR_IA32_MTRR_PHYSBASE0..=X86_MSR_IA32_MTRR_PHYSMASK9 |
            X86_MSR_IA32_BIOS_SIGN_ID |
            X86_MSR_DRAM_POWER_LIMIT |
            X86_MSR_PP0_POWER_LIMIT |
            X86_MSR_PP1_POWER_LIMIT |
            X86_MSR_PLATFORM_POWER_LIMIT |
            // From AMD64 Volume 2, Section 6.1.1: CSTAR is unused, but Linux likes to
            // set a null handler, even when not in compatibility mode. Just ignore it.
            X86_MSR_IA32_CSTAR => {
                next_rip(exit_info, vmcs);
                rx_OK
            }
            X86_MSR_IA32_TSC_DEADLINE => {
                if (local_apic_state.lvt_timer & LVT_TIMER_MODE_MASK) != LVT_TIMER_MODE_TSC_DEADLINE {
                    return rx_ERR_INVALID_ARGS;
                }
                next_rip(exit_info, vmcs);
                let tsc_deadline = (guest_state.rdx << 32) | (guest_state.rax & 0xFFFFFFFF);
                update_timer(local_apic_state, ticks_to_nanos(tsc_deadline));
                rx_OK
            }
            msr @ X2APIC_MSR_BASE..=X2APIC_MSR_MAX => {
                guest_state.rcx = msr as u64;
                handle_apic_wrmsr(exit_info, vmcs, guest_state, local_apic_state, packet)
            }
            KVM_SYSTEM_TIME_MSR_OLD |
            KVM_SYSTEM_TIME_MSR |
            KVM_BOOT_TIME_OLD |
            KVM_BOOT_TIME => {
                handle_kvm_wrmsr(exit_info, vmcs, guest_state, local_apic_state, pvclock, gpas)
            }
            _ => {
                dprintf!(INFO, "Unhandled wrmsr {:#x}\n", guest_state.rcx);
                local_apic_state.interrupt_tracker.virtual_interrupt(X86_INT_GP_FAULT);
                rx_OK
            }
        }
    }
    
    // Returns the page address for a given page table entry.
    //
    // If the page address is for a large page, we additionally calculate the offset
    // to the correct guest physical page that backs the large page.
    fn page_addr(pt_addr: rx_paddr_t, level: usize, guest_vaddr: rx_vaddr_t) -> rx_paddr_t {
        let mut off = 0;
        if IS_LARGE_PAGE(pt_addr) {
            if level == 1 {
                off = guest_vaddr & PAGE_OFFSET_MASK_HUGE;
            } else if level == 2 {
                off = guest_vaddr & PAGE_OFFSET_MASK_LARGE;
            }
        }
        (pt_addr & X86_PG_FRAME) + (off & X86_PG_FRAME)
    }
    
    fn get_page(
        vmcs: &AutoVmcs,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace,
        guest_vaddr: rx_vaddr_t,
        host_paddr: &mut rx_paddr_t
    ) -> rx_status_t {
        let indices = [
            VADDR_TO_PML4_INDEX(guest_vaddr),
            VADDR_TO_PDP_INDEX(guest_vaddr),
            VADDR_TO_PD_INDEX(guest_vaddr),
            VADDR_TO_PT_INDEX(guest_vaddr),
        ];
        
        let mut pt_addr = vmcs.read(VmcsFieldXX::GUEST_CR3);
        let mut pa: rx_paddr_t = 0;
        
        for level in 0..=X86_PAGING_LEVELS {
            let status = gpas.get_page(page_addr(pt_addr, level - 1, guest_vaddr), &mut pa);
            if status != rx_OK {
                return status;
            }
            
            if level == X86_PAGING_LEVELS || IS_LARGE_PAGE(pt_addr) {
                break;
            }
            
            let pt = unsafe { &*(paddr_to_physmap(pa) as *const [pt_entry_t]) };
            pt_addr = pt[indices[level]];
            
            if !IS_PAGE_PRESENT(pt_addr) {
                return rx_ERR_NOT_FOUND;
            }
        }
        
        *host_paddr = pa;
        rx_OK
    }
    
    fn fetch_data(
        vmcs: &AutoVmcs,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace,
        guest_vaddr: rx_vaddr_t,
        data: &mut [u8],
        size: usize
    ) -> rx_status_t {
        // TODO(abdulla): Make this handle a fetch that crosses more than two pages.
        if size > PAGE_SIZE {
            return rx_ERR_OUT_OF_RANGE;
        }
    
        let mut pa: rx_paddr_t = 0;
        let status = get_page(vmcs, gpas, guest_vaddr, &mut pa);
        if status != rx_OK {
            return status;
        }
    
        let page_offset = guest_vaddr & PAGE_OFFSET_MASK_4KB;
        let page = unsafe { &*(paddr_to_physmap(pa) as *const [u8]) };
        let from_page = core::cmp::min(size, PAGE_SIZE - page_offset as usize);
        
        bytes::mandatory_memcpy(&mut data[0..from_page], &page[page_offset as usize..(page_offset as usize + from_page)]);
    
        // If the fetch is not split across pages, return.
        if from_page == size {
            return rx_OK;
        }
    
        let status = get_page(vmcs, gpas, guest_vaddr + size as rx_vaddr_t, &mut pa);
        if status != rx_OK {
            return status;
        }
    
        let page = unsafe { &*(paddr_to_physmap(pa) as *const [u8]) };
        bytes::mandatory_memcpy(&mut data[from_page..size], &page[0..(size - from_page)]);
        rx_OK
    }
    
    fn handle_trap(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        read: bool,
        guest_paddr: rx_vaddr_t,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace,
        traps: &mut hypervisor::TrapMap,
        packet: &mut rx_port_packet_t
    ) -> rx_status_t {
        if exit_info.exit_instruction_length > X86_MAX_INST_LEN as u32 {
            return rx_ERR_INTERNAL;
        }
    
        let mut trap = hypervisor::Trap::default();
        let status = traps.find_trap(rx_GUEST_TRAP_BELL, guest_paddr, &mut trap);
        if status != rx_OK {
            return status;
        }
        
        next_rip(exit_info, vmcs);
    
        match trap.kind() {
            rx_GUEST_TRAP_BELL => {
                if read {
                    return rx_ERR_NOT_SUPPORTED;
                }
                
                unsafe { libc::memset(packet as *mut rx_port_packet_t as *mut libc::c_void, 0, core::mem::size_of::<rx_port_packet_t>()) };
                
                packet.key = trap.key();
                packet.type_ = rx_PKT_TYPE_GUEST_BELL;
                packet.guest_bell.addr = guest_paddr;
                
                if !trap.has_port() {
                    return rx_ERR_BAD_STATE;
                }
                
                trap.queue(*packet, vmcs)
            }
            rx_GUEST_TRAP_MEM => {
                unsafe { libc::memset(packet as *mut rx_port_packet_t as *mut libc::c_void, 0, core::mem::size_of::<rx_port_packet_t>()) };
                
                packet.key = trap.key();
                packet.type_ = rx_PKT_TYPE_GUEST_MEM;
                packet.guest_mem.addr = guest_paddr;
                packet.guest_mem.inst_len = exit_info.exit_instruction_length as u8;
                
                // See Volume 3, Section 5.2.1.
                let efer = vmcs.read(VmcsField64::GUEST_IA32_EFER);
                let cs_access_rights = vmcs.read(VmcsField32::GUEST_CS_ACCESS_RIGHTS);
                
                if (efer & X86_EFER_LMA != 0) && (cs_access_rights & GUEST_XX_ACCESS_RIGHTS_L != 0) {
                    // IA32-e 64 bit mode.
                    packet.guest_mem.default_operand_size = 4;
                } else if cs_access_rights & GUEST_XX_ACCESS_RIGHTS_D != 0 {
                    // CS.D set (and not 64 bit mode).
                    packet.guest_mem.default_operand_size = 4;
                } else {
                    // CS.D clear (and not 64 bit mode).
                    packet.guest_mem.default_operand_size = 2;
                }
                
                let mut data = [0u8; 15]; // X86_MAX_INST_LEN
                let status = fetch_data(
                    vmcs,
                    gpas,
                    exit_info.guest_rip,
                    &mut data,
                    packet.guest_mem.inst_len as usize
                );
                
                if status == rx_OK {
                    packet.guest_mem.inst_buf.copy_from_slice(&data[0..packet.guest_mem.inst_len as usize]);
                    rx_ERR_NEXT
                } else {
                    status
                }
            }
            _ => rx_ERR_BAD_STATE
        }
    }
    
    fn handle_ept_violation(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace,
        traps: &mut hypervisor::TrapMap,
        packet: &mut rx_port_packet_t
    ) -> rx_status_t {
        let ept_violation_info = EptViolationInfo::new(exit_info.exit_qualification);
        let guest_paddr = exit_info.guest_physical_address;
        
        let status = handle_trap(
            exit_info,
            vmcs,
            ept_violation_info.read,
            guest_paddr,
            gpas,
            traps,
            packet
        );
        
        match status {
            rx_ERR_NOT_FOUND => {
                // If there was no trap associated with this address and it is outside of
                // guest physical address space, return failure.
                if guest_paddr >= gpas.size() {
                    return rx_ERR_OUT_OF_RANGE;
                }
    
                let status = gpas.page_fault(guest_paddr);
                if status != rx_OK {
                    dprintf!(CRITICAL, "Unhandled EPT violation {:#x}\n", exit_info.guest_physical_address);
                }
                status
            }
            _ => status
        }
    }
    
    fn handle_xsetbv(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        guest_state: &mut GuestState
    ) -> rx_status_t {
        let guest_cr4 = vmcs.read(VmcsFieldXX::GUEST_CR4);
        if (guest_cr4 & X86_CR4_OSXSAVE) == 0 {
            return rx_ERR_INVALID_ARGS;
        }
    
        // We only support XCR0.
        if guest_state.rcx != 0 {
            return rx_ERR_INVALID_ARGS;
        }
    
        let mut leaf = cpuid_leaf::default();
        if !x86_get_cpuid_subleaf(X86_CPUID_XSAVE, 0, &mut leaf) {
            return rx_ERR_INTERNAL;
        }
    
        // Check that XCR0 is valid.
        let xcr0_bitmap = ((leaf.d as u64) << 32) | leaf.a as u64;
        let xcr0 = (guest_state.rdx << 32) | (guest_state.rax & 0xFFFFFFFF);
        
        if (!xcr0_bitmap & xcr0) != 0 ||
           // x87 state must be enabled.
           (xcr0 & X86_XSAVE_STATE_BIT_X87) != X86_XSAVE_STATE_BIT_X87 ||
           // If AVX state is enabled, SSE state must be enabled.
           (xcr0 & (X86_XSAVE_STATE_BIT_AVX | X86_XSAVE_STATE_BIT_SSE)) == X86_XSAVE_STATE_BIT_AVX {
            return rx_ERR_INVALID_ARGS;
        }
    
        guest_state.xcr0 = xcr0;
        next_rip(exit_info, vmcs);
        rx_OK
    }
    
    fn handle_pause(exit_info: &ExitInfo, vmcs: &mut AutoVmcs) -> rx_status_t {
        next_rip(exit_info, vmcs);
        vmcs.invalidate();
        thread_reschedule();
        rx_OK
    }
    
    fn handle_vmcall(
        exit_info: &ExitInfo,
        vmcs: &mut AutoVmcs,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace,
        guest_state: &mut GuestState
    ) -> rx_status_t {
        let info = VmCallInfo::new(guest_state);
        
        match info.type_ {
            VmCallType::CLOCK_PAIRING => {
                if info.arg[1] != 0 {
                    dprintf!(INFO, "CLOCK_PAIRING hypercall doesn't support clock type {}\n",
                            info.arg[1]);
                    guest_state.rax = VmCallStatus::OP_NOT_SUPPORTED as u64;
                } else {
                    let status = pvclock_populate_offset(gpas, info.arg[0]);
                    if status != rx_OK {
                        dprintf!(INFO, "Populating lock offset failed with {}\n", status);
                        guest_state.rax = VmCallStatus::FAULT as u64;
                    } else {
                        guest_state.rax = VmCallStatus::OK as u64;
                    }
                }
            }
            _ => {
                dprintf!(INFO, "Unknown VMCALL({}) (arg0={:#x}, arg1={:#x}, arg2={:#x}, arg3={:#x})\n",
                        info.type_ as u64,
                        info.arg[0], info.arg[1], info.arg[2], info.arg[3]);
                guest_state.rax = VmCallStatus::NO_SYS as u64;
            }
        }
        
        next_rip(exit_info, vmcs);
        // We never fail in case of hypercalls, we just return/propagate errors to the caller.
        rx_OK
    }
    
    pub fn vmexit_handler(
        vmcs: &mut AutoVmcs,
        guest_state: &mut GuestState,
        local_apic_state: &mut LocalApicState,
        pvclock: &mut PvClockState,
        gpas: &mut hypervisor::GuestPhysicalAddressSpace,
        traps: &mut hypervisor::TrapMap,
        packet: &mut rx_port_packet_t
    ) -> rx_status_t {
        let exit_info = ExitInfo::new(vmcs);
        let status = match exit_info.exit_reason {
            ExitReason::EXTERNAL_INTERRUPT => {
                ktrace_vcpu_exit(VCPU_EXTERNAL_INTERRUPT, exit_info.guest_rip);
                handle_external_interrupt(vmcs)
            }
            ExitReason::INTERRUPT_WINDOW => {
                trace!("handling interrupt window\n\n");
                ktrace_vcpu_exit(VCPU_INTERRUPT_WINDOW, exit_info.guest_rip);
                handle_interrupt_window(vmcs, local_apic_state)
            }
            ExitReason::CPUID => {
                trace!("handling CPUID\n\n");
                ktrace_vcpu_exit(VCPU_CPUID, exit_info.guest_rip);
                handle_cpuid(&exit_info, vmcs, guest_state)
            }
            ExitReason::HLT => {
                trace!("handling HLT\n\n");
                ktrace_vcpu_exit(VCPU_HLT, exit_info.guest_rip);
                handle_hlt(&exit_info, vmcs, local_apic_state)
            }
            ExitReason::CONTROL_REGISTER_ACCESS => {
                trace!("handling control-register access\n\n");
                ktrace_vcpu_exit(VCPU_CONTROL_REGISTER_ACCESS, exit_info.guest_rip);
                handle_control_register_access(&exit_info, vmcs, guest_state)
            }
            ExitReason::IO_INSTRUCTION => {
                ktrace_vcpu_exit(VCPU_IO_INSTRUCTION, exit_info.guest_rip);
                handle_io_instruction(&exit_info, vmcs, guest_state, traps, packet)
            }
            ExitReason::RDMSR => {
                trace!("handling RDMSR {:#x}\n\n", guest_state.rcx);
                ktrace_vcpu_exit(VCPU_RDMSR, exit_info.guest_rip);
                handle_rdmsr(&exit_info, vmcs, guest_state, local_apic_state)
            }
            ExitReason::WRMSR => {
                trace!("handling WRMSR {:#x}\n\n", guest_state.rcx);
                ktrace_vcpu_exit(VCPU_WRMSR, exit_info.guest_rip);
                handle_wrmsr(&exit_info, vmcs, guest_state, local_apic_state, pvclock, gpas, packet)
            }
            ExitReason::ENTRY_FAILURE_GUEST_STATE | ExitReason::ENTRY_FAILURE_MSR_LOADING => {
                trace!("handling VM entry failure\n\n");
                ktrace_vcpu_exit(VCPU_VM_ENTRY_FAILURE, exit_info.guest_rip);
                rx_ERR_BAD_STATE
            }
            ExitReason::EPT_VIOLATION => {
                trace!("handling EPT violation\n\n");
                ktrace_vcpu_exit(VCPU_EPT_VIOLATION, exit_info.guest_rip);
                handle_ept_violation(&exit_info, vmcs, gpas, traps, packet)
            }
            ExitReason::XSETBV => {
                trace!("handling XSETBV\n\n");
                ktrace_vcpu_exit(VCPU_XSETBV, exit_info.guest_rip);
                handle_xsetbv(&exit_info, vmcs, guest_state)
            }
            ExitReason::PAUSE => {
                trace!("handling PAUSE\n\n");
                ktrace_vcpu_exit(VCPU_PAUSE, exit_info.guest_rip);
                handle_pause(&exit_info, vmcs)
            }
            ExitReason::VMCALL => {
                trace!("handling VMCALL\n\n");
                ktrace_vcpu_exit(VCPU_VMCALL, exit_info.guest_rip);
                handle_vmcall(&exit_info, vmcs, gpas, guest_state)
            }
            // Currently all exceptions except NMI delivered to guest directly. NMI causes vmexit
            // and handled by host via IDT as any other interrupt/exception.
            ExitReason::EXCEPTION | _ => {
                ktrace_vcpu_exit(VCPU_UNKNOWN, exit_info.guest_rip);
                rx_ERR_NOT_SUPPORTED
            }
        };
        
        if status != rx_OK && status != rx_ERR_NEXT && status != rx_ERR_CANCELED {
            dprintf!(CRITICAL, "VM exit handler for {} ({}) at RIP {:#x} returned {}\n",
                    exit_info.exit_reason as u32,
                    exit_reason_name(exit_info.exit_reason),
                    exit_info.guest_rip,
                    status);
        }
        
        status
    }