// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::hypervisor::state_invalidator::StateInvalidator;

// MSR constants
pub const X86_MSR_IA32_VMX_PINBASED_CTLS: u32 = 0x0481;
pub const X86_MSR_IA32_VMX_PROCBASED_CTLS: u32 = 0x0482;
pub const X86_MSR_IA32_VMX_EXIT_CTLS: u32 = 0x0483;
pub const X86_MSR_IA32_VMX_ENTRY_CTLS: u32 = 0x0484;
pub const X86_MSR_IA32_VMX_PROCBASED_CTLS2: u32 = 0x048b;
pub const X86_MSR_IA32_VMX_TRUE_PINBASED_CTLS: u32 = 0x048d;
pub const X86_MSR_IA32_VMX_TRUE_PROCBASED_CTLS: u32 = 0x048e;
pub const X86_MSR_IA32_VMX_TRUE_EXIT_CTLS: u32 = 0x048f;
pub const X86_MSR_IA32_VMX_TRUE_ENTRY_CTLS: u32 = 0x0490;

// PROCBASED_CTLS2 flags
pub const PROCBASED_CTLS2_EPT: u32 = 1u32 << 1;
pub const PROCBASED_CTLS2_RDTSCP: u32 = 1u32 << 3;
pub const PROCBASED_CTLS2_X2APIC: u32 = 1u32 << 4;
pub const PROCBASED_CTLS2_VPID: u32 = 1u32 << 5;
pub const PROCBASED_CTLS2_UNRESTRICTED_GUEST: u32 = 1u32 << 7;
pub const PROCBASED_CTLS2_INVPCID: u32 = 1u32 << 12;

// PROCBASED_CTLS flags
pub const PROCBASED_CTLS_INT_WINDOW_EXITING: u32 = 1u32 << 2;
pub const PROCBASED_CTLS_HLT_EXITING: u32 = 1u32 << 7;
pub const PROCBASED_CTLS_CR3_LOAD_EXITING: u32 = 1u32 << 15;
pub const PROCBASED_CTLS_CR3_STORE_EXITING: u32 = 1u32 << 16;
pub const PROCBASED_CTLS_CR8_LOAD_EXITING: u32 = 1u32 << 19;
pub const PROCBASED_CTLS_CR8_STORE_EXITING: u32 = 1u32 << 20;
pub const PROCBASED_CTLS_TPR_SHADOW: u32 = 1u32 << 21;
pub const PROCBASED_CTLS_IO_EXITING: u32 = 1u32 << 24;
pub const PROCBASED_CTLS_MSR_BITMAPS: u32 = 1u32 << 28;
pub const PROCBASED_CTLS_PAUSE_EXITING: u32 = 1u32 << 30;
pub const PROCBASED_CTLS_PROCBASED_CTLS2: u32 = 1u32 << 31;

// PINBASED_CTLS flags
pub const PINBASED_CTLS_EXT_INT_EXITING: u32 = 1u32 << 0;
pub const PINBASED_CTLS_NMI_EXITING: u32 = 1u32 << 3;

// EXIT_CTLS flags
pub const EXIT_CTLS_64BIT_MODE: u32 = 1u32 << 9;
pub const EXIT_CTLS_ACK_INT_ON_EXIT: u32 = 1u32 << 15;
pub const EXIT_CTLS_SAVE_IA32_PAT: u32 = 1u32 << 18;
pub const EXIT_CTLS_LOAD_IA32_PAT: u32 = 1u32 << 19;
pub const EXIT_CTLS_SAVE_IA32_EFER: u32 = 1u32 << 20;
pub const EXIT_CTLS_LOAD_IA32_EFER: u32 = 1u32 << 21;

// ENTRY_CTLS flags
pub const ENTRY_CTLS_IA32E_MODE: u32 = 1u32 << 9;
pub const ENTRY_CTLS_LOAD_IA32_PAT: u32 = 1u32 << 14;
pub const ENTRY_CTLS_LOAD_IA32_EFER: u32 = 1u32 << 15;

// LINK_POINTER values
pub const LINK_POINTER_INVALIDATE: u64 = u64::MAX;

// GUEST_XX_ACCESS_RIGHTS flags
pub const GUEST_XX_ACCESS_RIGHTS_UNUSABLE: u32 = 1u32 << 16;
// See Volume 3, Section 24.4.1 for access rights format.
pub const GUEST_XX_ACCESS_RIGHTS_TYPE_A: u32 = 1u32 << 0;
pub const GUEST_XX_ACCESS_RIGHTS_TYPE_W: u32 = 1u32 << 1;
pub const GUEST_XX_ACCESS_RIGHTS_TYPE_E: u32 = 1u32 << 2;
pub const GUEST_XX_ACCESS_RIGHTS_TYPE_CODE: u32 = 1u32 << 3;
// See Volume 3, Section 3.4.5.1 for valid non-system selector types.
pub const GUEST_XX_ACCESS_RIGHTS_S: u32 = 1u32 << 4;
pub const GUEST_XX_ACCESS_RIGHTS_P: u32 = 1u32 << 7;
pub const GUEST_XX_ACCESS_RIGHTS_L: u32 = 1u32 << 13;
pub const GUEST_XX_ACCESS_RIGHTS_D: u32 = 1u32 << 14;
// See Volume 3, Section 3.5 for valid system selectors types.
pub const GUEST_TR_ACCESS_RIGHTS_TSS_BUSY_16_BIT: u32 = 3u32 << 0;
pub const GUEST_TR_ACCESS_RIGHTS_TSS_BUSY: u32 = 11u32 << 0;

pub const GUEST_XX_ACCESS_RIGHTS_DEFAULT: u32 = GUEST_XX_ACCESS_RIGHTS_TYPE_A |
                                              GUEST_XX_ACCESS_RIGHTS_TYPE_W |
                                              GUEST_XX_ACCESS_RIGHTS_S |
                                              GUEST_XX_ACCESS_RIGHTS_P;

// GUEST_INTERRUPTIBILITY_STATE flags
pub const INTERRUPTIBILITY_STI_BLOCKING: u32 = 1u32 << 0;
pub const INTERRUPTIBILITY_MOV_SS_BLOCKING: u32 = 1u32 << 1;

// VMCS fields
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum VmcsField16 {
    VPID = 0x0000,
    GUEST_CS_SELECTOR = 0x0802,
    GUEST_TR_SELECTOR = 0x080e,
    HOST_ES_SELECTOR = 0x0c00,
    HOST_CS_SELECTOR = 0x0c02,
    HOST_SS_SELECTOR = 0x0c04,
    HOST_DS_SELECTOR = 0x0c06,
    HOST_FS_SELECTOR = 0x0c08,
    HOST_GS_SELECTOR = 0x0c0a,
    HOST_TR_SELECTOR = 0x0c0c,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum VmcsField64 {
    MSR_BITMAPS_ADDRESS = 0x2004,
    EXIT_MSR_STORE_ADDRESS = 0x2006,
    EXIT_MSR_LOAD_ADDRESS = 0x2008,
    ENTRY_MSR_LOAD_ADDRESS = 0x200a,
    EPT_POINTER = 0x201a,
    GUEST_PHYSICAL_ADDRESS = 0x2400,
    LINK_POINTER = 0x2800,
    GUEST_IA32_PAT = 0x2804,
    GUEST_IA32_EFER = 0x2806,
    HOST_IA32_PAT = 0x2c00,
    HOST_IA32_EFER = 0x2c02,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum VmcsField32 {
    PINBASED_CTLS = 0x4000,
    PROCBASED_CTLS = 0x4002,
    EXCEPTION_BITMAP = 0x4004,
    PAGEFAULT_ERRORCODE_MASK = 0x4006,
    PAGEFAULT_ERRORCODE_MATCH = 0x4008,
    EXIT_CTLS = 0x400c,
    EXIT_MSR_STORE_COUNT = 0x400e,
    EXIT_MSR_LOAD_COUNT = 0x4010,
    ENTRY_CTLS = 0x4012,
    ENTRY_MSR_LOAD_COUNT = 0x4014,
    ENTRY_INTERRUPTION_INFORMATION = 0x4016,
    ENTRY_EXCEPTION_ERROR_CODE = 0x4018,
    PROCBASED_CTLS2 = 0x401e,
    INSTRUCTION_ERROR = 0x4400,
    EXIT_REASON = 0x4402,
    EXIT_INTERRUPTION_INFORMATION = 0x4404,
    EXIT_INTERRUPTION_ERROR_CODE = 0x4406,
    EXIT_INSTRUCTION_LENGTH = 0x440c,
    EXIT_INSTRUCTION_INFORMATION = 0x440e,
    HOST_IA32_SYSENTER_CS = 0x4c00,

    GUEST_ES_LIMIT = 0x4800,
    GUEST_CS_LIMIT = 0x4802,
    GUEST_SS_LIMIT = 0x4804,
    GUEST_DS_LIMIT = 0x4806,
    GUEST_FS_LIMIT = 0x4808,
    GUEST_GS_LIMIT = 0x480a,
    GUEST_LDTR_LIMIT = 0x480c,
    GUEST_TR_LIMIT = 0x480e,

    GUEST_GDTR_LIMIT = 0x4810,
    GUEST_IDTR_LIMIT = 0x4812,
    GUEST_CS_ACCESS_RIGHTS = 0x4816,
    GUEST_ES_ACCESS_RIGHTS = 0x4814,
    GUEST_SS_ACCESS_RIGHTS = 0x4818,
    GUEST_DS_ACCESS_RIGHTS = 0x481a,
    GUEST_FS_ACCESS_RIGHTS = 0x481c,
    GUEST_GS_ACCESS_RIGHTS = 0x481e,
    GUEST_LDTR_ACCESS_RIGHTS = 0x4820,
    GUEST_TR_ACCESS_RIGHTS = 0x4822,
    GUEST_INTERRUPTIBILITY_STATE = 0x4824,
    GUEST_ACTIVITY_STATE = 0x4826,
    GUEST_IA32_SYSENTER_CS = 0x482a,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum VmcsFieldXX {
    CR0_GUEST_HOST_MASK = 0x6000,
    CR4_GUEST_HOST_MASK = 0x6002,
    CR0_READ_SHADOW = 0x6004,
    CR4_READ_SHADOW = 0x6006,
    EXIT_QUALIFICATION = 0x6400,
    GUEST_LINEAR_ADDRESS = 0x640a,
    GUEST_CR0 = 0x6800,
    GUEST_CR3 = 0x6802,
    GUEST_CR4 = 0x6804,

    GUEST_ES_BASE = 0x6806,
    GUEST_CS_BASE = 0x6808,
    GUEST_SS_BASE = 0x680A,
    GUEST_DS_BASE = 0x680C,
    GUEST_FS_BASE = 0x680E,
    GUEST_GS_BASE = 0x6810,
    GUEST_TR_BASE = 0x6814,

    GUEST_GDTR_BASE = 0x6816,
    GUEST_IDTR_BASE = 0x6818,
    GUEST_RSP = 0x681c,
    GUEST_RIP = 0x681e,
    GUEST_RFLAGS = 0x6820,
    GUEST_PENDING_DEBUG_EXCEPTIONS = 0x6822,
    GUEST_IA32_SYSENTER_ESP = 0x6824,
    GUEST_IA32_SYSENTER_EIP = 0x6826,
    HOST_CR0 = 0x6c00,
    HOST_CR3 = 0x6c02,
    HOST_CR4 = 0x6c04,
    HOST_FS_BASE = 0x6c06,
    HOST_GS_BASE = 0x6c08,
    HOST_TR_BASE = 0x6c0a,
    HOST_GDTR_BASE = 0x6c0c,
    HOST_IDTR_BASE = 0x6c0e,
    HOST_IA32_SYSENTER_ESP = 0x6c10,
    HOST_IA32_SYSENTER_EIP = 0x6c12,
    HOST_RSP = 0x6c14,
    HOST_RIP = 0x6c16,
}

// INVEPT invalidation types
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum InvEpt {
    SINGLE_CONTEXT = 1,
    ALL_CONTEXT = 2,
}

// Trait for VMCS field value operations
pub trait VmcsFieldValue {
    type Output;
    
    fn read(self) -> Self::Output;
    fn write(self, val: Self::Output);
}

impl VmcsFieldValue for VmcsField16 {
    type Output = u16;
    
    fn read(self) -> Self::Output {
        super::vmread(self as u64) as u16
    }
    
    fn write(self, val: Self::Output) {
        super::vmwrite(self as u64, val as u64);
    }
}

impl VmcsFieldValue for VmcsField32 {
    type Output = u32;
    
    fn read(self) -> Self::Output {
        super::vmread(self as u64) as u32
    }
    
    fn write(self, val: Self::Output) {
        super::vmwrite(self as u64, val as u64);
    }
}

impl VmcsFieldValue for VmcsField64 {
    type Output = u64;
    
    fn read(self) -> Self::Output {
        super::vmread(self as u64)
    }
    
    fn write(self, val: Self::Output) {
        super::vmwrite(self as u64, val);
    }
}

impl VmcsFieldValue for VmcsFieldXX {
    type Output = u64;
    
    fn read(self) -> Self::Output {
        super::vmread(self as u64)
    }
    
    fn write(self, val: Self::Output) {
        super::vmwrite(self as u64, val);
    }
}

// Register access trait for consistent register copying
pub trait RegisterAccess {
    fn rax(&self) -> u64;
    fn rcx(&self) -> u64;
    fn rdx(&self) -> u64;
    fn rbx(&self) -> u64;
    fn rbp(&self) -> u64;
    fn rsi(&self) -> u64;
    fn rdi(&self) -> u64;
    fn r8(&self) -> u64;
    fn r9(&self) -> u64;
    fn r10(&self) -> u64;
    fn r11(&self) -> u64;
    fn r12(&self) -> u64;
    fn r13(&self) -> u64;
    fn r14(&self) -> u64;
    fn r15(&self) -> u64;
    
    fn set_rax(&mut self, val: u64);
    fn set_rcx(&mut self, val: u64);
    fn set_rdx(&mut self, val: u64);
    fn set_rbx(&mut self, val: u64);
    fn set_rbp(&mut self, val: u64);
    fn set_rsi(&mut self, val: u64);
    fn set_rdi(&mut self, val: u64);
    fn set_r8(&mut self, val: u64);
    fn set_r9(&mut self, val: u64);
    fn set_r10(&mut self, val: u64);
    fn set_r11(&mut self, val: u64);
    fn set_r12(&mut self, val: u64);
    fn set_r13(&mut self, val: u64);
    fn set_r14(&mut self, val: u64);
    fn set_r15(&mut self, val: u64);
}