// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use core::mem::size_of;
use crate::kernel::vm::PAGE_SIZE;

// Constants for HCR_EL2 (Hypervisor Configuration Register).
pub const HCR_EL2_VM: u64 = 1 << 0;
pub const HCR_EL2_PTW: u64 = 1 << 2;
pub const HCR_EL2_FMO: u64 = 1 << 3;
pub const HCR_EL2_IMO: u64 = 1 << 4;
pub const HCR_EL2_AMO: u64 = 1 << 5;
pub const HCR_EL2_VI: u64 = 1 << 7;
pub const HCR_EL2_DC: u64 = 1 << 12;
pub const HCR_EL2_TWI: u64 = 1 << 13;
pub const HCR_EL2_TWE: u64 = 1 << 14;
pub const HCR_EL2_TSC: u64 = 1 << 19;
pub const HCR_EL2_TVM: u64 = 1 << 26;
pub const HCR_EL2_RW: u64 = 1 << 31;

// Constants for SCTLR_ELx (System Control Register).
pub const SCTLR_ELX_M: u32 = 1 << 0;
pub const SCTLR_ELX_A: u32 = 1 << 1;
pub const SCTLR_ELX_C: u32 = 1 << 2;
pub const SCTLR_ELX_SA: u32 = 1 << 3;
pub const SCTLR_ELX_I: u32 = 1 << 12;

pub const SCTLR_EL1_RES1: u32 = 0x00500800;
pub const SCTLR_EL2_RES1: u32 = 0x30c50830;

// Constants for floating-point state offsets.
pub const FS_Q0: usize = 0;
pub const FS_Q: usize = FS_Q0 + 16;
pub const FS_NUM_REGS: usize = 32;
pub const FS_FPSR: usize = FS_Q(FS_NUM_REGS);
pub const FS_FPCR: usize = FS_FPSR + 8;

// Constants for system state offsets.
pub const SS_SP_EL0: usize = 0;
pub const SS_TPIDR_EL0: usize = SS_SP_EL0 + 8;
pub const SS_TPIDRRO_EL0: usize = SS_TPIDR_EL0 + 8;
pub const SS_CNTKCTL_EL1: usize = SS_TPIDRRO_EL0 + 8;
pub const SS_CONTEXTIDR_EL1: usize = SS_CNTKCTL_EL1 + 8;
pub const SS_CPACR_EL1: usize = SS_CONTEXTIDR_EL1 + 8;
pub const SS_CSSELR_EL1: usize = SS_CPACR_EL1 + 8;
pub const SS_ELR_EL1: usize = SS_CSSELR_EL1 + 8;
pub const SS_ESR_EL1: usize = SS_ELR_EL1 + 8;
pub const SS_FAR_EL1: usize = SS_ESR_EL1 + 8;
pub const SS_MAIR_EL1: usize = SS_FAR_EL1 + 8;
pub const SS_MDSCR_EL1: usize = SS_MAIR_EL1 + 8;
pub const SS_PAR_EL1: usize = SS_MDSCR_EL1 + 8;
pub const SS_SCTLR_EL1: usize = SS_PAR_EL1 + 8;
pub const SS_SP_EL1: usize = SS_SCTLR_EL1 + 8;
pub const SS_SPSR_EL1: usize = SS_SP_EL1 + 8;
pub const SS_TCR_EL1: usize = SS_SPSR_EL1 + 8;
pub const SS_TPIDR_EL1: usize = SS_TCR_EL1 + 8;
pub const SS_TTBR0_EL1: usize = SS_TPIDR_EL1 + 8;
pub const SS_TTBR1_EL1: usize = SS_TTBR0_EL1 + 8;
pub const SS_VBAR_EL1: usize = SS_TTBR1_EL1 + 8;
pub const SS_ELR_EL2: usize = SS_VBAR_EL1 + 8;
pub const SS_SPSR_EL2: usize = SS_ELR_EL2 + 8;
pub const SS_VMPIDR_EL2: usize = SS_SPSR_EL2 + 8;

// Constants for guest state offsets.
pub const ES_RESUME: usize = 0;
pub const GS_X0: usize = ES_RESUME + 16;
pub const GS_X: usize = GS_X0 + 8;
pub const GS_NUM_REGS: usize = 31;
pub const GS_FP_STATE: usize = GS_X(GS_NUM_REGS) + 8;
pub const GS_SYSTEM_STATE: usize = GS_FP_STATE + FS_FPCR + 8;
pub const GS_CNTV_CTL_EL0: usize = GS_SYSTEM_STATE + SS_VMPIDR_EL2 + 8;
pub const GS_CNTV_CVAL_EL0: usize = GS_CNTV_CTL_EL0 + 8;
pub const GS_ESR_EL2: usize = GS_CNTV_CVAL_EL0 + 8;
pub const GS_FAR_EL2: usize = GS_ESR_EL2 + 8;
pub const GS_HPFAR_EL2: usize = GS_FAR_EL2 + 8;

// Constants for host state offsets.
pub const HS_X18: usize = GS_HPFAR_EL2 + 16;
pub const HS_X: usize = HS_X18 + 8;
pub const HS_NUM_REGS: usize = 13;
pub const HS_FP_STATE: usize = HS_X18 + HS_X(HS_NUM_REGS) + 8;
pub const HS_SYSTEM_STATE: usize = HS_FP_STATE + FS_FPCR + 8;

/// Represents the floating-point state.
#[repr(C)]
#[derive(Debug)]
pub struct FpState {
    pub q: [u128; FS_NUM_REGS],
    pub fpsr: u32,
    pub fpcr: u32,
}

/// Represents the system state.
#[repr(C)]
#[derive(Debug)]
pub struct SystemState {
    pub sp_el0: u64,
    pub tpidr_el0: u64,
    pub tpidrro_el0: u64,

    pub cntkctl_el1: u32,
    pub contextidr_el1: u32,
    pub cpacr_el1: u32,
    pub csselr_el1: u32,
    pub elr_el1: u64,
    pub esr_el1: u32,
    pub far_el1: u64,
    pub mair_el1: u64,
    pub mdscr_el1: u32,
    pub par_el1: u64,
    pub sctlr_el1: u32,
    pub sp_el1: u64,
    pub spsr_el1: u32,
    pub tcr_el1: u64,
    pub tpidr_el1: u64,
    pub ttbr0_el1: u64,
    pub ttbr1_el1: u64,
    pub vbar_el1: u64,

    pub elr_el2: u64,
    pub spsr_el2: u32,
    pub vmpidr_el2: u64,
}

/// Represents the guest state.
#[repr(C)]
#[derive(Debug)]
pub struct GuestState {
    pub x: [u64; GS_NUM_REGS],
    pub fp_state: FpState,
    pub system_state: SystemState,

    // Exit state.
    pub cntv_ctl_el0: u32,
    pub cntv_cval_el0: u64,
    pub esr_el2: u32,
    pub far_el2: u64,
    pub hpfar_el2: u64,
}

/// Represents the host state.
#[repr(C)]
#[derive(Debug)]
pub struct HostState {
    pub x: [u64; HS_NUM_REGS],
    pub fp_state: FpState,
    pub system_state: SystemState,
}

/// Represents the EL2 state.
#[repr(C)]
#[derive(Debug)]
pub struct El2State {
    pub resume: bool,
    pub guest_state: GuestState,
    pub host_state: HostState,
}

// Ensure the size of `El2State` fits within a page.
const _: () = assert!(size_of::<El2State>() <= PAGE_SIZE);

// Ensure the offsets of fields in `FpState` match the constants.
const _: () = assert!(core::mem::offset_of!(FpState, q) == FS_Q0);
const _: () = assert!(core::mem::offset_of!(FpState, q[FS_NUM_REGS - 1]) == FS_Q(FS_NUM_REGS - 1));
const _: () = assert!(core::mem::offset_of!(FpState, fpsr) == FS_FPSR);
const _: () = assert!(core::mem::offset_of!(FpState, fpcr) == FS_FPCR);

// Ensure the offsets of fields in `SystemState` match the constants.
const _: () = assert!(core::mem::offset_of!(SystemState, sp_el0) == SS_SP_EL0);
const _: () = assert!(core::mem::offset_of!(SystemState, tpidr_el0) == SS_TPIDR_EL0);
const _: () = assert!(core::mem::offset_of!(SystemState, tpidrro_el0) == SS_TPIDRRO_EL0);
const _: () = assert!(core::mem::offset_of!(SystemState, cntkctl_el1) == SS_CNTKCTL_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, contextidr_el1) == SS_CONTEXTIDR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, cpacr_el1) == SS_CPACR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, csselr_el1) == SS_CSSELR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, elr_el1) == SS_ELR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, esr_el1) == SS_ESR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, far_el1) == SS_FAR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, mair_el1) == SS_MAIR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, mdscr_el1) == SS_MDSCR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, par_el1) == SS_PAR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, sctlr_el1) == SS_SCTLR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, sp_el1) == SS_SP_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, spsr_el1) == SS_SPSR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, tcr_el1) == SS_TCR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, tpidr_el1) == SS_TPIDR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, ttbr0_el1) == SS_TTBR0_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, ttbr1_el1) == SS_TTBR1_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, vbar_el1) == SS_VBAR_EL1);
const _: () = assert!(core::mem::offset_of!(SystemState, elr_el2) == SS_ELR_EL2);
const _: () = assert!(core::mem::offset_of!(SystemState, spsr_el2) == SS_SPSR_EL2);
const _: () = assert!(core::mem::offset_of!(SystemState, vmpidr_el2) == SS_VMPIDR_EL2);

// Ensure the offsets of fields in `El2State` match the constants.
const _: () = assert!(core::mem::offset_of!(El2State, resume) == ES_RESUME);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.x) == GS_X0);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.x[GS_NUM_REGS - 1]) == GS_X(GS_NUM_REGS - 1));
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.fp_state) == GS_FP_STATE);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.fp_state.q) == GS_FP_STATE + FS_Q0);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.system_state) == GS_SYSTEM_STATE);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.cntv_ctl_el0) == GS_CNTV_CTL_EL0);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.cntv_cval_el0) == GS_CNTV_CVAL_EL0);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.esr_el2) == GS_ESR_EL2);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.far_el2) == GS_FAR_EL2);
const _: () = assert!(core::mem::offset_of!(El2State, guest_state.hpfar_el2) == GS_HPFAR_EL2);

const _: () = assert!(core::mem::offset_of!(El2State, host_state.x) == HS_X18);
const _: () = assert!(core::mem::offset_of!(El2State, host_state.x[HS_NUM_REGS - 1]) == HS_X18 + HS_X(HS_NUM_REGS - 1));
const _: () = assert!(core::mem::offset_of!(El2State, host_state.fp_state) == HS_FP_STATE);
const _: () = assert!(core::mem::offset_of!(El2State, host_state.fp_state.q) == HS_FP_STATE + FS_Q0);
const _: () = assert!(core::mem::offset_of!(El2State, host_state.system_state) == HS_SYSTEM_STATE);

/// Enable EL2 mode.
pub unsafe fn rx_el2_on(ttbr0: PhysAddr, stack_top: PhysAddr) -> SyscallResult {
    // Placeholder for EL2 enable logic.
    SyscallResult::Ok(0)
}

/// Disable EL2 mode.
pub unsafe fn rx_el2_off() -> SyscallResult {
    // Placeholder for EL2 disable logic.
    SyscallResult::Ok(0)
}

/// Invalidate TLB entries for an IPA.
pub unsafe fn rx_el2_tlbi_ipa(vttbr: PhysAddr, addr: VirtAddr, terminal: bool) -> SyscallResult {
    // Placeholder for TLB invalidate logic.
    SyscallResult::Ok(0)
}

/// Invalidate TLB entries for a VMID.
pub unsafe fn rx_el2_tlbi_vmid(vttbr: PhysAddr) -> SyscallResult {
    // Placeholder for TLB invalidate logic.
    SyscallResult::Ok(0)
}

/// Resume execution of a guest.
pub unsafe fn rx_el2_resume(vttbr: PhysAddr, state: PhysAddr, hcr: u64) -> SyscallResult {
    // Placeholder for guest resume logic.
    SyscallResult::Ok(0)
}