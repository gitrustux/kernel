// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! VMX state management for virtualization
//!
//! This module provides types and functions for managing the state of the
//! host and guest during VMX operations (Intel's hardware virtualization).

use crate::rustux::types::*;
use core::mem::offset_of;

// Offsets for assembly code
/// Offset to resume flag in VmxState
pub const VS_RESUME: usize = 0;

/// Offset to host RIP in VmxState
pub const HS_RIP: usize = VS_RESUME + 8;
/// Offset to host RBX in VmxState
pub const HS_RBX: usize = HS_RIP + 8;
/// Offset to host RSP in VmxState
pub const HS_RSP: usize = HS_RBX + 8;
/// Offset to host RBP in VmxState
pub const HS_RBP: usize = HS_RSP + 8;
/// Offset to host R12 in VmxState
pub const HS_R12: usize = HS_RBP + 8;
/// Offset to host R13 in VmxState
pub const HS_R13: usize = HS_R12 + 8;
/// Offset to host R14 in VmxState
pub const HS_R14: usize = HS_R13 + 8;
/// Offset to host R15 in VmxState
pub const HS_R15: usize = HS_R14 + 8;
/// Offset to host RFLAGS in VmxState
pub const HS_RFLAGS: usize = HS_R15 + 8;

/// Offset to guest RAX in VmxState
pub const GS_RAX: usize = HS_RFLAGS + 16;
/// Offset to guest RCX in VmxState
pub const GS_RCX: usize = GS_RAX + 8;
/// Offset to guest RDX in VmxState
pub const GS_RDX: usize = GS_RCX + 8;
/// Offset to guest RBX in VmxState
pub const GS_RBX: usize = GS_RDX + 8;
/// Offset to guest RBP in VmxState
pub const GS_RBP: usize = GS_RBX + 8;
/// Offset to guest RSI in VmxState
pub const GS_RSI: usize = GS_RBP + 8;
/// Offset to guest RDI in VmxState
pub const GS_RDI: usize = GS_RSI + 8;
/// Offset to guest R8 in VmxState
pub const GS_R8: usize = GS_RDI + 8;
/// Offset to guest R9 in VmxState
pub const GS_R9: usize = GS_R8 + 8;
/// Offset to guest R10 in VmxState
pub const GS_R10: usize = GS_R9 + 8;
/// Offset to guest R11 in VmxState
pub const GS_R11: usize = GS_R10 + 8;
/// Offset to guest R12 in VmxState
pub const GS_R12: usize = GS_R11 + 8;
/// Offset to guest R13 in VmxState
pub const GS_R13: usize = GS_R12 + 8;
/// Offset to guest R14 in VmxState
pub const GS_R14: usize = GS_R13 + 8;
/// Offset to guest R15 in VmxState
pub const GS_R15: usize = GS_R14 + 8;
/// Offset to guest CR2 in VmxState
pub const GS_CR2: usize = GS_R15 + 8;

/// Host state that needs to be saved during VMX operations
#[repr(C)]
#[derive(Debug, Clone)]
pub struct HostState {
    /// Return address
    pub rip: u64,
    
    /// Callee-save registers
    pub rbx: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    
    /// Processor flags
    pub rflags: u64,
    
    /// Extended control registers
    pub xcr0: u64,
}

/// Guest register state during VMX operations
#[repr(C)]
#[derive(Debug, Clone)]
pub struct GuestState {
    // Note: RIP, RSP, and RFLAGS are automatically saved by VMX in the VMCS
    /// RAX register
    pub rax: u64,
    /// RCX register
    pub rcx: u64,
    /// RDX register
    pub rdx: u64,
    /// RBX register
    pub rbx: u64,
    /// RBP register
    pub rbp: u64,
    /// RSI register
    pub rsi: u64,
    /// RDI register
    pub rdi: u64,
    /// R8 register
    pub r8: u64,
    /// R9 register
    pub r9: u64,
    /// R10 register
    pub r10: u64,
    /// R11 register
    pub r11: u64,
    /// R12 register
    pub r12: u64,
    /// R13 register
    pub r13: u64,
    /// R14 register
    pub r14: u64,
    /// R15 register
    pub r15: u64,
    
    /// CR2 control register
    pub cr2: u64,
    
    /// Extended control register XCR0
    pub xcr0: u64,
}

/// Complete VMX state (host and guest)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VmxState {
    /// Flag indicating whether to resume or not
    pub resume: bool,
    /// Host CPU state
    pub host_state: HostState,
    /// Guest CPU state
    pub guest_state: GuestState,
}

// Static assertions to verify the layout matches the expected offsets
const _: () = assert!(offset_of!(VmxState, resume) == VS_RESUME);

const _: () = assert!(offset_of!(VmxState, host_state.rip) == HS_RIP);
const _: () = assert!(offset_of!(VmxState, host_state.rsp) == HS_RSP);
const _: () = assert!(offset_of!(VmxState, host_state.rbp) == HS_RBP);
const _: () = assert!(offset_of!(VmxState, host_state.rbx) == HS_RBX);
const _: () = assert!(offset_of!(VmxState, host_state.r12) == HS_R12);
const _: () = assert!(offset_of!(VmxState, host_state.r13) == HS_R13);
const _: () = assert!(offset_of!(VmxState, host_state.r14) == HS_R14);
const _: () = assert!(offset_of!(VmxState, host_state.r15) == HS_R15);
const _: () = assert!(offset_of!(VmxState, host_state.rflags) == HS_RFLAGS);

const _: () = assert!(offset_of!(VmxState, guest_state.rax) == GS_RAX);
const _: () = assert!(offset_of!(VmxState, guest_state.rbx) == GS_RBX);
const _: () = assert!(offset_of!(VmxState, guest_state.rcx) == GS_RCX);
const _: () = assert!(offset_of!(VmxState, guest_state.rdx) == GS_RDX);
const _: () = assert!(offset_of!(VmxState, guest_state.rdi) == GS_RDI);
const _: () = assert!(offset_of!(VmxState, guest_state.rsi) == GS_RSI);
const _: () = assert!(offset_of!(VmxState, guest_state.rbp) == GS_RBP);
const _: () = assert!(offset_of!(VmxState, guest_state.r8) == GS_R8);
const _: () = assert!(offset_of!(VmxState, guest_state.r9) == GS_R9);
const _: () = assert!(offset_of!(VmxState, guest_state.r10) == GS_R10);
const _: () = assert!(offset_of!(VmxState, guest_state.r11) == GS_R11);
const _: () = assert!(offset_of!(VmxState, guest_state.r12) == GS_R12);
const _: () = assert!(offset_of!(VmxState, guest_state.r13) == GS_R13);
const _: () = assert!(offset_of!(VmxState, guest_state.r14) == GS_R14);
const _: () = assert!(offset_of!(VmxState, guest_state.r15) == GS_R15);
const _: () = assert!(offset_of!(VmxState, guest_state.cr2) == GS_CR2);

/// Launch the guest and save the host state
///
/// # Arguments
///
/// * `vmx_state` - The VMX state structure to use
///
/// # Returns
///
/// If we return `Ok(())`, we have exited from the guest successfully.
/// Otherwise, we have failed to launch the guest.
///
/// # Safety
///
/// This function is unsafe because it:
/// - Directly modifies the CPU state
/// - Can cause VM entries and exits
/// - May execute arbitrary guest code
pub unsafe fn vmx_enter(vmx_state: &mut VmxState) -> RxStatus {
    sys_vmx_enter(vmx_state)
}

/// Exit the guest and load the saved host state
///
/// This function is never called directly, but is executed on exit from a guest.
/// It calls vmx_exit before returning through vmx_enter.
///
/// # Arguments
///
/// * `vmx_state` - The VMX state structure to use
///
/// # Safety
///
/// This function is unsafe because it:
/// - Directly modifies the CPU state
/// - Is part of the VM exit handling mechanism
/// - Is typically called from assembly
pub unsafe fn vmx_exit(vmx_state: &mut VmxState) {
    sys_vmx_exit(vmx_state)
}

extern "C" {
    /// Entry point for VM exit
    pub fn vmx_exit_entry();

    // System function declarations
    fn sys_vmx_enter(vmx_state: *mut VmxState) -> RxStatus;
    fn sys_vmx_exit(vmx_state: *mut VmxState);
}