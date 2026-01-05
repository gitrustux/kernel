// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::kernel::arch::arm64::hypervisor::{GuestState, GichState, VcpuExit};
use crate::kernel::arch::hypervisor::{GuestPhysicalAddressSpace, TrapMap, VcpuPacket};
use crate::kernel::dev::psci::{PSCI_SUCCESS, PSCI_NOT_SUPPORTED, PSCI64_CPU_ON};
use crate::kernel::dev::timer::arm_generic::{current_ticks, cntpct_to_rx_time};
use crate::kernel::syscalls::SyscallResult;
use crate::kernel::vm::{PhysAddr, VirtAddr, PAGE_SIZE};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};

/// Constants for timer control.
const TIMER_ENABLE: u32 = 1 << 0;
const TIMER_IMASK: u32 = 1 << 1;
const TIMER_ISTATUS: u32 = 1 << 2;

/// Constants for exception syndrome.
const ESR_EC_SHIFT: u32 = 26;
const ESR_ISS_MASK: u32 = 0x01ffffff;

/// Represents the exception syndrome register (ESR).
struct ExceptionSyndrome {
    ec: ExceptionClass,
    iss: u32,
}

impl ExceptionSyndrome {
    fn new(esr: u32) -> Self {
        Self {
            ec: ((esr >> ESR_EC_SHIFT) & 0x3f).into(),
            iss: esr & ESR_ISS_MASK,
        }
    }
}

/// Represents the exception class in the ESR.
#[derive(Clone, Copy)]
enum ExceptionClass {
    WfiWfeInstruction = 0x01,
    SmcInstruction = 0x02,
    SystemInstruction = 0x03,
    InstructionAbort = 0x04,
    DataAbort = 0x05,
    Unknown = 0xff,
}

impl From<u32> for ExceptionClass {
    fn from(value: u32) -> Self {
        match value {
            0x01 => Self::WfiWfeInstruction,
            0x02 => Self::SmcInstruction,
            0x03 => Self::SystemInstruction,
            0x04 => Self::InstructionAbort,
            0x05 => Self::DataAbort,
            _ => Self::Unknown,
        }
    }
}

/// Handles a WFI/WFE instruction.
fn handle_wfi_wfe_instruction(iss: u32, guest_state: &mut GuestState, gich_state: &mut GichState) -> SyscallResult {
    guest_state.system_state.elr_el2 += 4; // Increment PC
    let is_wfe = (iss & 0x1) != 0;
    if is_wfe {
        // Handle WFE
        VcpuExit::WfeInstruction.log(guest_state.system_state.elr_el2);
        crate::kernel::task::reschedule();
        SyscallResult::Ok(0)
    } else {
        // Handle WFI
        VcpuExit::WfiInstruction.log(guest_state.system_state.elr_el2);
        let deadline = if timer_enabled(guest_state) {
            if current_ticks() >= guest_state.cntv_cval_el0 {
                return SyscallResult::Ok(0);
            }
            cntpct_to_rx_time(guest_state.cntv_cval_el0)
        } else {
            u64::MAX // Infinite timeout
        };
        gich_state.interrupt_tracker.wait(deadline)
    }
}

/// Handles an SMC instruction.
fn handle_smc_instruction(iss: u32, guest_state: &mut GuestState, packet: &mut VcpuPacket) -> SyscallResult {
    let imm = iss & 0xffff;
    if imm != 0 {
        return SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported);
    }

    guest_state.system_state.elr_el2 += 4; // Increment PC
    match guest_state.x[0] {
        PSCI64_CPU_ON => {
            *packet = VcpuPacket::new_startup(guest_state.x[1], guest_state.x[2]);
            guest_state.x[0] = PSCI_SUCCESS;
            SyscallResult::Err(crate::kernel::syscalls::SyscallError::Next)
        }
        _ => {
            guest_state.x[0] = PSCI_NOT_SUPPORTED;
            SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported)
        }
    }
}

/// Handles a system instruction.
fn handle_system_instruction(
    iss: u32,
    hcr: &mut u64,
    guest_state: &mut GuestState,
    gpas: &GuestPhysicalAddressSpace,
    packet: &mut VcpuPacket,
) -> SyscallResult {
    let sysreg = (iss >> 10) & 0x3ff;
    let xt = (iss >> 5) & 0x1f;
    let read = (iss & 0x1) != 0;
    let reg = guest_state.x[xt as usize];

    match sysreg {
        0x180 => { // MAIR_EL1
            guest_state.system_state.mair_el1 = reg;
            guest_state.system_state.elr_el2 += 4;
            SyscallResult::Ok(0)
        }
        0x100 => { // SCTLR_EL1
            if read {
                return SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported);
            }
            let sctlr_el1 = reg as u32;
            if sctlr_el1 & SCTLR_ELX_M != 0 {
                *hcr &= !HCR_EL2_DC;
                if sctlr_el1 & SCTLR_ELX_C != 0 {
                    *hcr &= !HCR_EL2_TVM;
                }
                clean_invalidate_cache(gpas.arch_aspace().arch_table_phys(), MMU_GUEST_TOP_SHIFT);
            }
            guest_state.system_state.sctlr_el1 = sctlr_el1;
            guest_state.system_state.elr_el2 += 4;
            SyscallResult::Ok(0)
        }
        _ => SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported),
    }
}

/// Handles an instruction abort.
fn handle_instruction_abort(guest_state: &mut GuestState, gpas: &GuestPhysicalAddressSpace) -> SyscallResult {
    let guest_paddr = guest_state.hpfar_el2;
    gpas.page_fault(guest_paddr)
}

/// Handles a data abort.
fn handle_data_abort(
    iss: u32,
    guest_state: &mut GuestState,
    gpas: &GuestPhysicalAddressSpace,
    traps: &TrapMap,
    packet: &mut VcpuPacket,
) -> SyscallResult {
    let guest_paddr = guest_state.hpfar_el2 | (guest_state.far_el2 & (PAGE_SIZE - 1));
    match traps.find_trap(guest_paddr) {
        Ok(trap) => {
            guest_state.system_state.elr_el2 += 4;
            match trap.kind() {
                TrapKind::Bell => {
                    *packet = VcpuPacket::new_bell(trap.key(), guest_paddr);
                    trap.queue(packet)
                }
                TrapKind::Mem => {
                    let data_abort = DataAbort::new(iss);
                    *packet = VcpuPacket::new_mem(
                        trap.key(),
                        guest_paddr,
                        data_abort.access_size,
                        data_abort.sign_extend,
                        data_abort.xt,
                        data_abort.read,
                        if !data_abort.read { guest_state.x[data_abort.xt as usize] } else { 0 },
                    );
                    SyscallResult::Err(crate::kernel::syscalls::SyscallError::Next)
                }
                _ => SyscallResult::Err(crate::kernel::syscalls::SyscallError::BadState),
            }
        }
        Err(_) => gpas.page_fault(guest_paddr),
    }
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
    let syndrome = ExceptionSyndrome::new(guest_state.esr_el2);
    let status = match syndrome.ec {
        ExceptionClass::WfiWfeInstruction => handle_wfi_wfe_instruction(syndrome.iss, guest_state, gich_state),
        ExceptionClass::SmcInstruction => handle_smc_instruction(syndrome.iss, guest_state, packet),
        ExceptionClass::SystemInstruction => handle_system_instruction(syndrome.iss, hcr, guest_state, gpas, packet),
        ExceptionClass::InstructionAbort => handle_instruction_abort(guest_state, gpas),
        ExceptionClass::DataAbort => handle_data_abort(syndrome.iss, guest_state, gpas, traps, packet),
        _ => SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported),
    };

    if status.is_err() && status != SyscallResult::Err(crate::kernel::syscalls::SyscallError::Next) {
        println!(
            "VM exit handler for {:?} in EL{} at {:#x} returned {:?}",
            syndrome.ec,
            (guest_state.system_state.spsr_el2 >> 2) & 0x3,
            guest_state.system_state.elr_el2,
            status
        );
    }
    status
}