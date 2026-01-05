// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::kernel::arch::hypervisor::{GuestPhysicalAddressSpace, TrapMap};
use crate::kernel::syscalls::SyscallResult;
use crate::kernel::vm::PhysAddr;

/// Represents the exception class in the ESR (Exception Syndrome Register).
#[derive(Debug, Clone, Copy)]
pub enum ExceptionClass {
    WfiWfeInstruction = 0b000001,
    SmcInstruction = 0b010111,
    SystemInstruction = 0b011000,
    InstructionAbort = 0b100000,
    DataAbort = 0b100100,
}

impl ExceptionClass {
    /// Returns the name of the exception class.
    pub fn name(&self) -> &'static str {
        match self {
            ExceptionClass::WfiWfeInstruction => "WFI_WFE_INSTRUCTION",
            ExceptionClass::SmcInstruction => "SMC_INSTRUCTION",
            ExceptionClass::SystemInstruction => "SYSTEM_INSTRUCTION",
            ExceptionClass::InstructionAbort => "INSTRUCTION_ABORT",
            ExceptionClass::DataAbort => "DATA_ABORT",
        }
    }
}

/// Represents the exception syndrome for a VM exit.
pub struct ExceptionSyndrome {
    pub ec: ExceptionClass,
    pub iss: u32,
}

impl ExceptionSyndrome {
    /// Creates a new `ExceptionSyndrome` from the ESR value.
    pub fn new(esr: u32) -> Self {
        Self {
            ec: ((esr >> 26) & 0x3f).into(),
            iss: esr & 0x01ffffff,
        }
    }
}

impl From<u32> for ExceptionClass {
    fn from(value: u32) -> Self {
        match value {
            0b000001 => ExceptionClass::WfiWfeInstruction,
            0b010111 => ExceptionClass::SmcInstruction,
            0b011000 => ExceptionClass::SystemInstruction,
            0b100000 => ExceptionClass::InstructionAbort,
            0b100100 => ExceptionClass::DataAbort,
            _ => panic!("Unknown exception class"),
        }
    }
}

/// Represents a wait instruction that caused a VM exit.
pub struct WaitInstruction {
    pub is_wfe: bool,
}

impl WaitInstruction {
    /// Creates a new `WaitInstruction` from the ISS value.
    pub fn new(iss: u32) -> Self {
        Self {
            is_wfe: (iss & 0x1) != 0,
        }
    }
}

/// Represents an SMC instruction that caused a VM exit.
pub struct SmcInstruction {
    pub imm: u16,
}

impl SmcInstruction {
    /// Creates a new `SmcInstruction` from the ISS value.
    pub fn new(iss: u32) -> Self {
        Self {
            imm: (iss & 0xffff) as u16,
        }
    }
}

/// Represents a system register associated with a system instruction.
#[derive(Debug, Clone, Copy)]
pub enum SystemRegister {
    MairEl1 = 0b11000000 << 8 | 0b10100010,
    SctlrEl1 = 0b11000000 << 8 | 0b00010000,
    TcrEl1 = 0b11010000 << 8 | 0b00100000,
    Ttbr0El1 = 0b11000000 << 8 | 0b00100000,
    Ttbr1El1 = 0b11001000 << 8 | 0b00100000,
    OslarEl1 = 0b10100000 << 8 | 0b00010000,
    OslsrEl1 = 0b10100000 << 8 | 0b00010001,
    OsdlrEl1 = 0b10100000 << 8 | 0b00010011,
    DbgprcrEl1 = 0b10100000 << 8 | 0b00010100,
    IccSgi1rEl1 = 0b11101000 << 8 | 0b11001011,
}

/// Represents a system instruction that caused a VM exit.
pub struct SystemInstruction {
    pub sysreg: SystemRegister,
    pub xt: u8,
    pub read: bool,
}

impl SystemInstruction {
    /// Creates a new `SystemInstruction` from the ISS value.
    pub fn new(iss: u32) -> Self {
        Self {
            sysreg: ((iss >> 10) & 0x3ff).into(),
            xt: ((iss >> 5) & 0x1f) as u8,
            read: (iss & 0x1) != 0,
        }
    }
}

impl From<u32> for SystemRegister {
    fn from(value: u32) -> Self {
        match value {
            0b11000000 << 8 | 0b10100010 => SystemRegister::MairEl1,
            0b11000000 << 8 | 0b00010000 => SystemRegister::SctlrEl1,
            0b11010000 << 8 | 0b00100000 => SystemRegister::TcrEl1,
            0b11000000 << 8 | 0b00100000 => SystemRegister::Ttbr0El1,
            0b11001000 << 8 | 0b00100000 => SystemRegister::Ttbr1El1,
            0b10100000 << 8 | 0b00010000 => SystemRegister::OslarEl1,
            0b10100000 << 8 | 0b00010001 => SystemRegister::OslsrEl1,
            0b10100000 << 8 | 0b00010011 => SystemRegister::OsdlrEl1,
            0b10100000 << 8 | 0b00010100 => SystemRegister::DbgprcrEl1,
            0b11101000 << 8 | 0b11001011 => SystemRegister::IccSgi1rEl1,
            _ => panic!("Unknown system register"),
        }
    }
}

/// Represents an SGI (Software Generated Interrupt) register.
pub struct SgiRegister {
    pub aff3: u8,
    pub aff2: u8,
    pub aff1: u8,
    pub rs: u8,
    pub target_list: u16,
    pub int_id: u8,
    pub all_but_local: bool,
}

impl SgiRegister {
    /// Creates a new `SgiRegister` from the SGI value.
    pub fn new(sgi: u64) -> Self {
        Self {
            aff3: ((sgi >> 48) & 0xff) as u8,
            aff2: ((sgi >> 32) & 0xff) as u8,
            aff1: ((sgi >> 16) & 0xff) as u8,
            rs: ((sgi >> 44) & 0xf) as u8,
            target_list: (sgi & 0xffff) as u16,
            int_id: ((sgi >> 24) & 0xff) as u8,
            all_but_local: (sgi & (1 << 40)) != 0,
        }
    }
}

/// Represents a data abort that caused a VM exit.
pub struct DataAbort {
    pub valid: bool,
    pub access_size: u8,
    pub sign_extend: bool,
    pub xt: u8,
    pub read: bool,
}

impl DataAbort {
    /// Creates a new `DataAbort` from the ISS value.
    pub fn new(iss: u32) -> Self {
        Self {
            valid: (iss & (1 << 24)) != 0,
            access_size: 1 << ((iss >> 22) & 0x3),
            sign_extend: (iss & (1 << 21)) != 0,
            xt: ((iss >> 16) & 0x1f) as u8,
            read: (iss & (1 << 6)) == 0,
        }
    }
}

/// Handles a timer interrupt for the guest.
pub fn timer_maybe_interrupt(guest_state: &mut GuestState, gich_state: &mut GichState) {
    // Placeholder for timer interrupt logic.
}

/// Main VM exit handler.
pub fn vmexit_handler(
    hcr: &mut u64,
    guest_state: &mut GuestState,
    gich_state: &mut GichState,
    gpas: &GuestPhysicalAddressSpace,
    traps: &TrapMap,
    packet: &mut VcpuPacket,
) -> SyscallResult {
    // Placeholder for VM exit handling logic.
    SyscallResult::Ok(0)
}