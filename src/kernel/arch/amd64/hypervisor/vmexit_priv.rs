// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::hypervisor::guest_physical_address_space::GuestPhysicalAddressSpace;
use crate::hypervisor::trap_map::TrapMap;
use crate::rustux::types::*;

/// VM exit reasons.
#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ExitReason {
    EXCEPTION = 0,  // NMI is an exception too
    EXTERNAL_INTERRUPT = 1,
    TRIPLE_FAULT = 2,
    INIT_SIGNAL = 3,
    STARTUP_IPI = 4,
    IO_SMI = 5,
    OTHER_SMI = 6,
    INTERRUPT_WINDOW = 7,
    NMI_WINDOW = 8,
    TASK_SWITCH = 9,
    CPUID = 10,
    GETSEC = 11,
    HLT = 12,
    INVD = 13,
    INVLPG = 14,
    RDPMC = 15,
    RDTSC = 16,
    RSM = 17,
    VMCALL = 18,
    VMCLEAR = 19,
    VMLAUNCH = 20,
    VMPTRLD = 21,
    VMPTRST = 22,
    VMREAD = 23,
    VMRESUME = 24,
    VMWRITE = 25,
    VMXOFF = 26,
    VMXON = 27,
    CONTROL_REGISTER_ACCESS = 28,
    MOV_DR = 29,
    IO_INSTRUCTION = 30,
    RDMSR = 31,
    WRMSR = 32,
    ENTRY_FAILURE_GUEST_STATE = 33,
    ENTRY_FAILURE_MSR_LOADING = 34,
    MWAIT = 36,
    MONITOR_TRAP_FLAG = 37,
    MONITOR = 39,
    PAUSE = 40,
    ENTRY_FAILURE_MACHINE_CHECK = 41,
    TPR_BELOW_THRESHOLD = 43,
    APIC_ACCESS = 44,
    VIRTUALIZED_EOI = 45,
    ACCESS_GDTR_OR_IDTR = 46,
    ACCESS_LDTR_OR_TR = 47,
    EPT_VIOLATION = 48,
    EPT_MISCONFIGURATION = 49,
    INVEPT = 50,
    RDTSCP = 51,
    VMX_PREEMPT_TIMER_EXPIRED = 52,
    INVVPID = 53,
    WBINVD = 54,
    XSETBV = 55,
    APIC_WRITE = 56,
    RDRAND = 57,
    INVPCID = 58,
    VMFUNC = 59,
    ENCLS = 60,
    RDSEED = 61,
    PAGE_MODIFICATION_LOG_FULL = 62,
    XSAVES = 63,
    XRSTORS = 64,
}

impl ExitReason {
    pub fn from_u32(value: u32) -> Self {
        if value <= Self::XRSTORS as u32 {
            // Safety: We're ensuring the value is within the enum range
            unsafe { std::mem::transmute(value) }
        } else {
            Self::EXCEPTION // Default to EXCEPTION for unknown values
        }
    }
}

pub fn exit_reason_name(exit_reason: ExitReason) -> &'static str {
    match exit_reason {
        ExitReason::EXCEPTION => "EXCEPTION",
        ExitReason::EXTERNAL_INTERRUPT => "EXTERNAL_INTERRUPT",
        ExitReason::TRIPLE_FAULT => "TRIPLE_FAULT",
        ExitReason::INIT_SIGNAL => "INIT_SIGNAL",
        ExitReason::STARTUP_IPI => "STARTUP_IPI",
        ExitReason::IO_SMI => "IO_SMI",
        ExitReason::OTHER_SMI => "OTHER_SMI",
        ExitReason::INTERRUPT_WINDOW => "INTERRUPT_WINDOW",
        ExitReason::NMI_WINDOW => "NMI_WINDOW",
        ExitReason::TASK_SWITCH => "TASK_SWITCH",
        ExitReason::CPUID => "CPUID",
        ExitReason::GETSEC => "GETSEC",
        ExitReason::HLT => "HLT",
        ExitReason::INVD => "INVD",
        ExitReason::INVLPG => "INVLPG",
        ExitReason::RDPMC => "RDPMC",
        ExitReason::RDTSC => "RDTSC",
        ExitReason::RSM => "RSM",
        ExitReason::VMCALL => "VMCALL",
        ExitReason::VMCLEAR => "VMCLEAR",
        ExitReason::VMLAUNCH => "VMLAUNCH",
        ExitReason::VMPTRLD => "VMPTRLD",
        ExitReason::VMPTRST => "VMPTRST",
        ExitReason::VMREAD => "VMREAD",
        ExitReason::VMRESUME => "VMRESUME",
        ExitReason::VMWRITE => "VMWRITE",
        ExitReason::VMXOFF => "VMXOFF",
        ExitReason::VMXON => "VMXON",
        ExitReason::CONTROL_REGISTER_ACCESS => "CONTROL_REGISTER_ACCESS",
        ExitReason::MOV_DR => "MOV_DR",
        ExitReason::IO_INSTRUCTION => "IO_INSTRUCTION",
        ExitReason::RDMSR => "RDMSR",
        ExitReason::WRMSR => "WRMSR",
        ExitReason::ENTRY_FAILURE_GUEST_STATE => "ENTRY_FAILURE_GUEST_STATE",
        ExitReason::ENTRY_FAILURE_MSR_LOADING => "ENTRY_FAILURE_MSR_LOADING",
        ExitReason::MWAIT => "MWAIT",
        ExitReason::MONITOR_TRAP_FLAG => "MONITOR_TRAP_FLAG",
        ExitReason::MONITOR => "MONITOR",
        ExitReason::PAUSE => "PAUSE",
        ExitReason::ENTRY_FAILURE_MACHINE_CHECK => "ENTRY_FAILURE_MACHINE_CHECK",
        ExitReason::TPR_BELOW_THRESHOLD => "TPR_BELOW_THRESHOLD",
        ExitReason::APIC_ACCESS => "APIC_ACCESS",
        ExitReason::VIRTUALIZED_EOI => "VIRTUALIZED_EOI",
        ExitReason::ACCESS_GDTR_OR_IDTR => "ACCESS_GDTR_OR_IDTR",
        ExitReason::ACCESS_LDTR_OR_TR => "ACCESS_LDTR_OR_TR",
        ExitReason::EPT_VIOLATION => "EPT_VIOLATION",
        ExitReason::EPT_MISCONFIGURATION => "EPT_MISCONFIGURATION",
        ExitReason::INVEPT => "INVEPT",
        ExitReason::RDTSCP => "RDTSCP",
        ExitReason::VMX_PREEMPT_TIMER_EXPIRED => "VMX_PREEMPT_TIMER_EXPIRED",
        ExitReason::INVVPID => "INVVPID",
        ExitReason::WBINVD => "WBINVD",
        ExitReason::XSETBV => "XSETBV",
        ExitReason::APIC_WRITE => "APIC_WRITE",
        ExitReason::RDRAND => "RDRAND",
        ExitReason::INVPCID => "INVPCID",
        ExitReason::VMFUNC => "VMFUNC",
        ExitReason::ENCLS => "ENCLS",
        ExitReason::RDSEED => "RDSEED",
        ExitReason::PAGE_MODIFICATION_LOG_FULL => "PAGE_MODIFICATION_LOG_FULL",
        ExitReason::XSAVES => "XSAVES",
        ExitReason::XRSTORS => "XRSTORS",
    }
}

/// VM exit interruption type.
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum InterruptionType {
    EXTERNAL_INTERRUPT = 0,
    NON_MASKABLE_INTERRUPT = 2,
    HARDWARE_EXCEPTION = 3,
    SOFTWARE_EXCEPTION = 6,
}

impl InterruptionType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::EXTERNAL_INTERRUPT,
            2 => Self::NON_MASKABLE_INTERRUPT,
            3 => Self::HARDWARE_EXCEPTION,
            6 => Self::SOFTWARE_EXCEPTION,
            _ => Self::EXTERNAL_INTERRUPT, // Default for invalid values
        }
    }
}

/// X2APIC MSR addresses from Volume 3, Section 10.12.1.2.
#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum X2ApicMsr {
    ID = 0x802,
    VERSION = 0x803,
    EOI = 0x80b,
    TPR = 0x808,
    LDR = 0x80d,
    SVR = 0x80f,
    ISR_31_0 = 0x810,
    ISR_63_32 = 0x811,
    ISR_95_64 = 0x812,
    ISR_127_96 = 0x813,
    ISR_159_128 = 0x814,
    ISR_191_160 = 0x815,
    ISR_223_192 = 0x816,
    ISR_255_224 = 0x817,
    TMR_31_0 = 0x818,
    TMR_63_32 = 0x819,
    TMR_95_64 = 0x81a,
    TMR_127_96 = 0x81b,
    TMR_159_128 = 0x81c,
    TMR_191_160 = 0x81d,
    TMR_223_192 = 0x81e,
    TMR_255_224 = 0x81f,
    IRR_31_0 = 0x820,
    IRR_63_32 = 0x821,
    IRR_95_64 = 0x822,
    IRR_127_96 = 0x823,
    IRR_159_128 = 0x824,
    IRR_191_160 = 0x825,
    IRR_223_192 = 0x826,
    IRR_255_224 = 0x827,
    ESR = 0x828,
    LVT_CMCI = 0x82f,
    ICR = 0x830,
    LVT_TIMER = 0x832,
    LVT_THERMAL_SENSOR = 0x833,
    LVT_MONITOR = 0x834,
    LVT_LINT0 = 0x835,
    LVT_LINT1 = 0x836,
    LVT_ERROR = 0x837,
    INITIAL_COUNT = 0x838,
    DCR = 0x83e,
    SELF_IPI = 0x83f,
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum InterruptDeliveryMode {
    FIXED = 0,
    SMI = 2,
    NMI = 4,
    INIT = 5,
    STARTUP = 6,
}

impl InterruptDeliveryMode {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::FIXED,
            2 => Self::SMI,
            4 => Self::NMI,
            5 => Self::INIT,
            6 => Self::STARTUP,
            _ => Self::FIXED, // Default for invalid values
        }
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum InterruptDestinationMode {
    PHYSICAL = 0,
    LOGICAL = 1,
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum InterruptDestinationShorthand {
    NO_SHORTHAND = 0,
    SELF = 1,
    ALL_INCLUDING_SELF = 2,
    ALL_EXCLUDING_SELF = 3,
}

impl InterruptDestinationShorthand {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::NO_SHORTHAND,
            1 => Self::SELF,
            2 => Self::ALL_INCLUDING_SELF,
            3 => Self::ALL_EXCLUDING_SELF,
            _ => Self::NO_SHORTHAND, // Default for invalid values
        }
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum CrAccessType {
    MOV_TO_CR = 0,
    MOV_FROM_CR = 1,
    CLTS = 2,
    LMSW = 3,
}

impl CrAccessType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::MOV_TO_CR,
            1 => Self::MOV_FROM_CR,
            2 => Self::CLTS,
            3 => Self::LMSW,
            _ => Self::MOV_TO_CR, // Default for invalid values
        }
    }
}

#[repr(i64)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum VmCallStatus {
    OK = 0,
    FAULT = -14,
    OP_NOT_SUPPORTED = -95,
    NO_SYS = -1000,
}

#[repr(u64)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum VmCallType {
    CLOCK_PAIRING = 9,
}

impl VmCallType {
    pub fn from_u64(value: u64) -> Self {
        match value {
            9 => Self::CLOCK_PAIRING,
            _ => Self::CLOCK_PAIRING, // Default for invalid values
        }
    }
}

pub struct AutoVmcs;
pub struct GuestState;
pub struct LocalApicState;
pub struct PvClockState;

/// Function to handle VM exits
pub fn vmexit_handler(
    vmcs: &mut AutoVmcs,
    guest_state: &mut GuestState,
    local_apic_state: &mut LocalApicState,
    pvclock: &mut PvClockState,
    gpas: &mut GuestPhysicalAddressSpace,
    traps: &mut TrapMap,
    packet: &mut rx_port_packet_t,
) -> rx_status_t;