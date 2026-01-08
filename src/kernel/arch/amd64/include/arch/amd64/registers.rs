// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 CPU register definitions and manipulation
//!
//! This module provides constants and functions for manipulating x86 CPU
//! registers, including control registers, MSRs, flags, and debug registers.

// Control Register (CR) flags
/// CR0: Protected mode enable
pub const X86_CR0_PE: u64 = 0x00000001;
/// CR0: Monitor coprocessor
pub const X86_CR0_MP: u64 = 0x00000002;
/// CR0: Emulation
pub const X86_CR0_EM: u64 = 0x00000004;
/// CR0: Task switched
pub const X86_CR0_TS: u64 = 0x00000008;
/// CR0: Enable x87 exception
pub const X86_CR0_NE: u64 = 0x00000020;
/// CR0: Supervisor write protect
pub const X86_CR0_WP: u64 = 0x00010000;
/// CR0: Not write-through
pub const X86_CR0_NW: u64 = 0x20000000;
/// CR0: Cache disable
pub const X86_CR0_CD: u64 = 0x40000000;
/// CR0: Enable paging
pub const X86_CR0_PG: u64 = 0x80000000;
/// CR4: PAE paging
pub const X86_CR4_PAE: u64 = 0x00000020;
/// CR4: Page global enable
pub const X86_CR4_PGE: u64 = 0x00000080;
/// CR4: OS supports fxsave
pub const X86_CR4_OSFXSR: u64 = 0x00000200;
/// CR4: OS supports XMM exception
pub const X86_CR4_OSXMMEXPT: u64 = 0x00000400;
/// CR4: User-mode instruction prevention
pub const X86_CR4_UMIP: u64 = 0x00000800;
/// CR4: Enable VMX
pub const X86_CR4_VMXE: u64 = 0x00002000;
/// CR4: Enable {rd,wr}{fs,gs}base
pub const X86_CR4_FSGSBASE: u64 = 0x00010000;
/// CR4: Process-context ID enable
pub const X86_CR4_PCIDE: u64 = 0x00020000;
/// CR4: OS supports xsave
pub const X86_CR4_OSXSAVE: u64 = 0x00040000;
/// CR4: SMEP protection enabling
pub const X86_CR4_SMEP: u64 = 0x00100000;
/// CR4: SMAP protection enabling
pub const X86_CR4_SMAP: u64 = 0x00200000;
/// CR4: Disabling PSE bit in the CR4
pub const X86_CR4_PSE: u64 = 0xffffffef;

// Extended Feature Enable Register (EFER) flags
/// EFER: Enable SYSCALL
pub const X86_EFER_SCE: u64 = 0x00000001;
/// EFER: Long mode enable
pub const X86_EFER_LME: u64 = 0x00000100;
/// EFER: Long mode active
pub const X86_EFER_LMA: u64 = 0x00000400;
/// EFER: Enable execute disable bit
pub const X86_EFER_NXE: u64 = 0x00000800;

// MSR definitions
/// MSR: Platform ID
pub const X86_MSR_IA32_PLATFORM_ID: u32 = 0x00000017;
/// MSR: APIC base physical address
pub const X86_MSR_IA32_APIC_BASE: u32 = 0x0000001b;
/// MSR: TSC adjust
pub const X86_MSR_IA32_TSC_ADJUST: u32 = 0x0000003b;
/// MSR: BIOS update signature
pub const X86_MSR_IA32_BIOS_SIGN_ID: u32 = 0x0000008b;
/// MSR: MTRR capability
pub const X86_MSR_IA32_MTRRCAP: u32 = 0x000000fe;
/// MSR: SYSENTER CS
pub const X86_MSR_IA32_SYSENTER_CS: u32 = 0x00000174;
/// MSR: SYSENTER ESP
pub const X86_MSR_IA32_SYSENTER_ESP: u32 = 0x00000175;
/// MSR: SYSENTER EIP
pub const X86_MSR_IA32_SYSENTER_EIP: u32 = 0x00000176;
/// MSR: Global machine check capability
pub const X86_MSR_IA32_MCG_CAP: u32 = 0x00000179;
/// MSR: Global machine check status
pub const X86_MSR_IA32_MCG_STATUS: u32 = 0x0000017a;
/// MSR: Enable/disable misc processor features
pub const X86_MSR_IA32_MISC_ENABLE: u32 = 0x000001a0;
/// MSR: Temperature target
pub const X86_MSR_IA32_TEMPERATURE_TARGET: u32 = 0x000001a2;
/// MSR: MTRR PhysBase0
pub const X86_MSR_IA32_MTRR_PHYSBASE0: u32 = 0x00000200;
/// MSR: MTRR PhysMask0
pub const X86_MSR_IA32_MTRR_PHYSMASK0: u32 = 0x00000201;
/// MSR: MTRR PhysMask9
pub const X86_MSR_IA32_MTRR_PHYSMASK9: u32 = 0x00000213;
/// MSR: MTRR default type
pub const X86_MSR_IA32_MTRR_DEF_TYPE: u32 = 0x000002ff;
/// MSR: MTRR FIX64K_00000
pub const X86_MSR_IA32_MTRR_FIX64K_00000: u32 = 0x00000250;
/// MSR: MTRR FIX16K_80000
pub const X86_MSR_IA32_MTRR_FIX16K_80000: u32 = 0x00000258;
/// MSR: MTRR FIX16K_A0000
pub const X86_MSR_IA32_MTRR_FIX16K_A0000: u32 = 0x00000259;
/// MSR: MTRR FIX4K_C0000
pub const X86_MSR_IA32_MTRR_FIX4K_C0000: u32 = 0x00000268;
/// MSR: MTRR FIX4K_F8000
pub const X86_MSR_IA32_MTRR_FIX4K_F8000: u32 = 0x0000026f;
/// MSR: PAT
pub const X86_MSR_IA32_PAT: u32 = 0x00000277;
/// MSR: TSC deadline
pub const X86_MSR_IA32_TSC_DEADLINE: u32 = 0x000006e0;
/// MSR: EFER
pub const X86_MSR_IA32_EFER: u32 = 0xc0000080;
/// MSR: System call address
pub const X86_MSR_IA32_STAR: u32 = 0xc0000081;
/// MSR: Long mode call address
pub const X86_MSR_IA32_LSTAR: u32 = 0xc0000082;
/// MSR: IA32-e compat call address
pub const X86_MSR_IA32_CSTAR: u32 = 0xc0000083;
/// MSR: System call flag mask
pub const X86_MSR_IA32_FMASK: u32 = 0xc0000084;
/// MSR: FS base address
pub const X86_MSR_IA32_FS_BASE: u32 = 0xc0000100;
/// MSR: GS base address
pub const X86_MSR_IA32_GS_BASE: u32 = 0xc0000101;
/// MSR: Kernel GS base
pub const X86_MSR_IA32_KERNEL_GS_BASE: u32 = 0xc0000102;
/// MSR: TSC aux
pub const X86_MSR_IA32_TSC_AUX: u32 = 0xc0000103;
/// MSR: Enable/disable HWP
pub const X86_MSR_IA32_PM_ENABLE: u32 = 0x00000770;
/// MSR: HWP performance range enumeration
pub const X86_MSR_IA32_HWP_CAPABILITIES: u32 = 0x00000771;
/// MSR: Power manage control hints
pub const X86_MSR_IA32_HWP_REQUEST: u32 = 0x00000774;

// Non-architectural MSRs
/// MSR: RAPL unit multipliers
pub const X86_MSR_RAPL_POWER_UNIT: u32 = 0x00000606;
/// MSR: Package power limits
pub const X86_MSR_PKG_POWER_LIMIT: u32 = 0x00000610;
/// MSR: Package power limit PL1 clamp
pub const X86_MSR_PKG_POWER_LIMIT_PL1_CLAMP: u32 = 1 << 16;
/// MSR: Package power limit PL1 enable
pub const X86_MSR_PKG_POWER_LIMIT_PL1_ENABLE: u32 = 1 << 15;
/// MSR: Package energy status
pub const X86_MSR_PKG_ENERGY_STATUS: u32 = 0x00000611;
/// MSR: Package power range info
pub const X86_MSR_PKG_POWER_INFO: u32 = 0x00000614;
/// MSR: DRAM RAPL power limit control
pub const X86_MSR_DRAM_POWER_LIMIT: u32 = 0x00000618;
/// MSR: DRAM energy status
pub const X86_MSR_DRAM_ENERGY_STATUS: u32 = 0x00000619;
/// MSR: PP0 RAPL power limit control
pub const X86_MSR_PP0_POWER_LIMIT: u32 = 0x00000638;
/// MSR: PP0 energy status
pub const X86_MSR_PP0_ENERGY_STATUS: u32 = 0x00000639;
/// MSR: PP1 RAPL power limit control
pub const X86_MSR_PP1_POWER_LIMIT: u32 = 0x00000640;
/// MSR: PP1 energy status
pub const X86_MSR_PP1_ENERGY_STATUS: u32 = 0x00000641;
/// MSR: Platform energy counter
pub const X86_MSR_PLATFORM_ENERGY_COUNTER: u32 = 0x0000064d;
/// MSR: Platform power limit control
pub const X86_MSR_PLATFORM_POWER_LIMIT: u32 = 0x0000065c;

// EFLAGS/RFLAGS bits
/// EFLAGS: Carry flag
pub const X86_FLAGS_CF: u64 = 1 << 0;
/// EFLAGS: Parity flag
pub const X86_FLAGS_PF: u64 = 1 << 2;
/// EFLAGS: Auxiliary flag
pub const X86_FLAGS_AF: u64 = 1 << 4;
/// EFLAGS: Zero flag
pub const X86_FLAGS_ZF: u64 = 1 << 6;
/// EFLAGS: Sign flag
pub const X86_FLAGS_SF: u64 = 1 << 7;
/// EFLAGS: Trap flag
pub const X86_FLAGS_TF: u64 = 1 << 8;
/// EFLAGS: Interrupt flag
pub const X86_FLAGS_IF: u64 = 1 << 9;
/// EFLAGS: Direction flag
pub const X86_FLAGS_DF: u64 = 1 << 10;
/// EFLAGS: Overflow flag
pub const X86_FLAGS_OF: u64 = 1 << 11;
/// EFLAGS: Status mask
pub const X86_FLAGS_STATUS_MASK: u64 = 0xfff;
/// EFLAGS: IOPL mask
pub const X86_FLAGS_IOPL_MASK: u64 = 3 << 12;
/// EFLAGS: IOPL shift
pub const X86_FLAGS_IOPL_SHIFT: u64 = 12;
/// EFLAGS: Nested task flag
pub const X86_FLAGS_NT: u64 = 1 << 14;
/// EFLAGS: Resume flag
pub const X86_FLAGS_RF: u64 = 1 << 16;
/// EFLAGS: Virtual 8086 mode
pub const X86_FLAGS_VM: u64 = 1 << 17;
/// EFLAGS: Alignment check
pub const X86_FLAGS_AC: u64 = 1 << 18;
/// EFLAGS: Virtual interrupt flag
pub const X86_FLAGS_VIF: u64 = 1 << 19;
/// EFLAGS: Virtual interrupt pending
pub const X86_FLAGS_VIP: u64 = 1 << 20;
/// EFLAGS: ID flag
pub const X86_FLAGS_ID: u64 = 1 << 21;
/// EFLAGS: Reserved bits that must be 1
pub const X86_FLAGS_RESERVED_ONES: u64 = 0x2;
/// EFLAGS: Reserved bits
pub const X86_FLAGS_RESERVED: u64 = 0xffc0802a;
/// EFLAGS: User-modifiable flags
pub const X86_FLAGS_USER: u64 = X86_FLAGS_CF
    | X86_FLAGS_PF
    | X86_FLAGS_AF
    | X86_FLAGS_ZF
    | X86_FLAGS_SF
    | X86_FLAGS_TF
    | X86_FLAGS_DF
    | X86_FLAGS_OF
    | X86_FLAGS_NT
    | X86_FLAGS_AC
    | X86_FLAGS_ID;

// Debug Register (DR) flags
/// DR6: Breakpoint 0 condition detected
pub const X86_DR6_B0: u64 = 1 << 0;
/// DR6: Breakpoint 1 condition detected
pub const X86_DR6_B1: u64 = 1 << 1;
/// DR6: Breakpoint 2 condition detected
pub const X86_DR6_B2: u64 = 1 << 2;
/// DR6: Breakpoint 3 condition detected
pub const X86_DR6_B3: u64 = 1 << 3;
/// DR6: Debug register access detected
pub const X86_DR6_BD: u64 = 1 << 13;
/// DR6: Single step
pub const X86_DR6_BS: u64 = 1 << 14;
/// DR6: Task switch
pub const X86_DR6_BT: u64 = 1 << 15;

// NOTE: DR6 is used as a read-only status registers, and it is not writeable through userspace.
//       Any bits attempted to be written will be ignored.
/// DR6: User-modifiable bits mask
pub const X86_DR6_USER_MASK: u64 = X86_DR6_B0
    | X86_DR6_B1
    | X86_DR6_B2
    | X86_DR6_B3
    | X86_DR6_BD
    | X86_DR6_BS
    | X86_DR6_BT;

/// DR6: Bits writable by user
/// Only bits in X86_DR6_USER_MASK are writeable.
/// Bits 12 and 32:63 must be written with 0, the rest as 1s.
pub const X86_DR6_MASK: u64 = 0xffff0ff0;

// DR7 flags
/// DR7: Local breakpoint 0 enable
pub const X86_DR7_L0: u64 = 1 << 0;
/// DR7: Global breakpoint 0 enable
pub const X86_DR7_G0: u64 = 1 << 1;
/// DR7: Local breakpoint 1 enable
pub const X86_DR7_L1: u64 = 1 << 2;
/// DR7: Global breakpoint 1 enable
pub const X86_DR7_G1: u64 = 1 << 3;
/// DR7: Local breakpoint 2 enable
pub const X86_DR7_L2: u64 = 1 << 4;
/// DR7: Global breakpoint 2 enable
pub const X86_DR7_G2: u64 = 1 << 5;
/// DR7: Local breakpoint 3 enable
pub const X86_DR7_L3: u64 = 1 << 6;
/// DR7: Global breakpoint 3 enable
pub const X86_DR7_G3: u64 = 1 << 7;
/// DR7: Local exact breakpoint enable
pub const X86_DR7_LE: u64 = 1 << 8;
/// DR7: Global exact breakpoint enable
pub const X86_DR7_GE: u64 = 1 << 9;
/// DR7: General detect enable
pub const X86_DR7_GD: u64 = 1 << 13;
/// DR7: Read/write 0 field
pub const X86_DR7_RW0: u64 = 3 << 16;
/// DR7: Length 0 field
pub const X86_DR7_LEN0: u64 = 3 << 18;
/// DR7: Read/write 1 field
pub const X86_DR7_RW1: u64 = 3 << 20;
/// DR7: Length 1 field
pub const X86_DR7_LEN1: u64 = 3 << 22;
/// DR7: Read/write 2 field
pub const X86_DR7_RW2: u64 = 3 << 24;
/// DR7: Length 2 field
pub const X86_DR7_LEN2: u64 = 3 << 26;
/// DR7: Read/write 3 field
pub const X86_DR7_RW3: u64 = 3 << 28;
/// DR7: Length 3 field
pub const X86_DR7_LEN3: u64 = 3 << 30;

// NOTE1: Even though the GD bit is writable, we disable it for the write_state syscall because it
//        complicates a lot the reasoning about how to access the registers. This is because
//        enabling this bit would make any other access to debug registers to issue an exception.
//        New syscalls should be define to lock/unlock debug registers.
// NOTE2: LE/GE bits are normally ignored, but the manual recommends always setting it to 1 in
//        order to be backwards compatible. Hence they are not writable from userspace.
/// DR7: User-modifiable bits mask
pub const X86_DR7_USER_MASK: u64 = X86_DR7_L0
    | X86_DR7_G0
    | X86_DR7_L1
    | X86_DR7_G1
    | X86_DR7_L2
    | X86_DR7_G2
    | X86_DR7_L3
    | X86_DR7_G3
    | X86_DR7_RW0
    | X86_DR7_LEN0
    | X86_DR7_RW1
    | X86_DR7_LEN1
    | X86_DR7_RW2
    | X86_DR7_LEN2
    | X86_DR7_RW3
    | X86_DR7_LEN3;

/// DR7: Bits that must have specific values (bit 10 must be set, 11:12, 14:15 and 32:63 cleared)
pub const X86_DR7_MASK: u64 = (1 << 10) | X86_DR7_LE | X86_DR7_GE;

use crate::kernel::thread::Thread;
use crate::rustux::types::*;
use alloc::vec::Vec;

/// Indices of xsave feature states; state components are
/// enumerated in Intel Vol 1 section 13.1
pub mod xsave_state_index {
    /// x87 FPU state
    pub const X87: u32 = 0;
    /// SSE state
    pub const SSE: u32 = 1;
    /// AVX state
    pub const AVX: u32 = 2;
    /// MPX bounds registers
    pub const MPX_BNDREG: u32 = 3;
    /// MPX bounds configuration
    pub const MPX_BNDCSR: u32 = 4;
    /// AVX-512 opmask registers
    pub const AVX512_OPMASK: u32 = 5;
    /// AVX-512 lower ZMM high registers
    pub const AVX512_LOWERZMM_HIGH: u32 = 6;
    /// AVX-512 higher ZMM registers
    pub const AVX512_HIGHERZMM: u32 = 7;
    /// Processor Trace state
    pub const PT: u32 = 8;
    /// Protection Key state
    pub const PKRU: u32 = 9;
}

/// Bit masks for xsave feature states
pub mod xsave_state_bit {
    use super::xsave_state_index;
    
    /// x87 FPU state bit
    pub const X87: u64 = 1 << xsave_state_index::X87;
    /// SSE state bit
    pub const SSE: u64 = 1 << xsave_state_index::SSE;
    /// AVX state bit
    pub const AVX: u64 = 1 << xsave_state_index::AVX;
    /// MPX bounds registers bit
    pub const MPX_BNDREG: u64 = 1 << xsave_state_index::MPX_BNDREG;
    /// MPX bounds configuration bit
    pub const MPX_BNDCSR: u64 = 1 << xsave_state_index::MPX_BNDCSR;
    /// AVX-512 opmask registers bit
    pub const AVX512_OPMASK: u64 = 1 << xsave_state_index::AVX512_OPMASK;
    /// AVX-512 lower ZMM high registers bit
    pub const AVX512_LOWERZMM_HIGH: u64 = 1 << xsave_state_index::AVX512_LOWERZMM_HIGH;
    /// AVX-512 higher ZMM registers bit
    pub const AVX512_HIGHERZMM: u64 = 1 << xsave_state_index::AVX512_HIGHERZMM;
    /// Processor Trace state bit
    pub const PT: u64 = 1 << xsave_state_index::PT;
    /// Protection Key state bit
    pub const PKRU: u64 = 1 << xsave_state_index::PKRU;
}

/// Maximum buffer size needed for xsave and variants
pub const X86_MAX_EXTENDED_REGISTER_SIZE: usize = 1024;

/// Extended register feature types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86ExtendedRegisterFeature {
    /// x87 FPU registers
    X87,
    /// SSE registers (XMM)
    Sse,
    /// AVX registers (YMM)
    Avx,
    /// MPX registers (bounds)
    Mpx,
    /// AVX-512 registers (ZMM)
    Avx512,
    /// Processor Trace
    Pt,
    /// Protection Keys
    Pkru,
}

/// Legacy area in the xsave buffer for x87 and SSE state
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct X86XsaveLegacyArea {
    /// FPU control word
    pub fcw: u16,
    /// FPU status word
    pub fsw: u16,
    /// Abridged FPU tag word (not the same as the FTW register, see Intel manual sec 10.5.1.1)
    pub ftw: u8,
    /// Reserved
    pub reserved: u8,
    /// FPU opcode
    pub fop: u16,
    /// FPU instruction pointer
    pub fip: u64,
    /// FPU data pointer
    pub fdp: u64,
    /// SSE control status register
    pub mxcsr: u32,
    /// SSE control status register mask
    pub mxcsr_mask: u32,

    /// x87/MMX state
    /// For x87 each "st" entry has the low 80 bits used for register contents.
    /// For MMX, the low 64 bits are used. Higher bits are unused.
    pub st: [X87MmxReg; 8],

    /// SSE registers
    pub xmm: [XmmReg; 16],
}

/// x87/MMX register
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct X87MmxReg {
    /// Lower 64 bits (used for MMX)
    pub low: u64,
    /// Higher 64 bits (used for x87 extended precision)
    pub high: u64,
}

/// SSE/XMM register
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct XmmReg {
    /// Lower 64 bits
    pub low: u64,
    /// Higher 64 bits
    pub high: u64,
}

/// Debug register state
#[repr(C)]
#[derive(Debug, Clone)]
pub struct X86DebugState {
    /// Debug address registers DR0-DR3
    pub dr: [u64; 4],
    /// Debug status register DR6
    pub dr6: u64,
    /// Debug control register DR7
    pub dr7: u64,
}

/// Initialize extended register support
///
/// Identify which extended registers are supported and initialize
/// the FPU if present.
///
/// # Safety
///
/// This function is unsafe as it modifies CPU state directly.
pub unsafe fn x86_extended_register_init() {
    sys_x86_extended_register_init();
}

/// Enable an extended register feature
///
/// # Arguments
///
/// * `feature` - The feature to enable
///
/// # Returns
///
/// `true` if the feature was successfully enabled, `false` otherwise
///
/// # Safety
///
/// This function is unsafe as it modifies CPU state directly.
/// It is currently assumed that if a feature is enabled on one CPU,
/// the caller will ensure it is enabled on all CPUs.
pub unsafe fn x86_extended_register_enable_feature(feature: X86ExtendedRegisterFeature) -> bool {
    sys_x86_extended_register_enable_feature(feature)
}

/// Get the size of the extended register state
///
/// # Returns
///
/// The size in bytes of the extended register state
pub fn x86_extended_register_size() -> usize {
    unsafe { sys_x86_extended_register_size() }
}

/// Initialize an extended register state buffer
///
/// # Arguments
///
/// * `buffer` - A buffer of at least X86_MAX_EXTENDED_REGISTER_SIZE bytes, 64-byte aligned
///
/// # Safety
///
/// The provided buffer must be properly aligned (64-byte boundary) and sized.
pub unsafe fn x86_extended_register_init_state(buffer: &mut [u8]) {
    assert!(buffer.len() >= X86_MAX_EXTENDED_REGISTER_SIZE, "Buffer too small for extended register state");
    sys_x86_extended_register_init_state(buffer.as_mut_ptr() as *mut core::ffi::c_void);
}

/// Save current extended register state to buffer
///
/// # Arguments
///
/// * `buffer` - A buffer previously initialized with `x86_extended_register_init_state`
///
/// # Safety
///
/// The provided buffer must have been previously initialized with `x86_extended_register_init_state`.
pub unsafe fn x86_extended_register_save_state(buffer: &mut [u8]) {
    assert!(buffer.len() >= X86_MAX_EXTENDED_REGISTER_SIZE, "Buffer too small for extended register state");
    sys_x86_extended_register_save_state(buffer.as_mut_ptr() as *mut core::ffi::c_void);
}

/// Restore extended register state from buffer
///
/// # Arguments
///
/// * `buffer` - A buffer containing a saved state from `x86_extended_register_save_state`
///
/// # Safety
///
/// The provided buffer must contain a valid saved state.
pub unsafe fn x86_extended_register_restore_state(buffer: &[u8]) {
    assert!(buffer.len() >= X86_MAX_EXTENDED_REGISTER_SIZE, "Buffer too small for extended register state");
    sys_x86_extended_register_restore_state(buffer.as_ptr() as *const core::ffi::c_void);
}

/// Handle extended register context switching
///
/// # Arguments
///
/// * `old_thread` - Thread being switched out
/// * `new_thread` - Thread being switched in
///
/// # Safety
///
/// This function is unsafe as it modifies CPU state directly.
pub unsafe fn x86_extended_register_context_switch(old_thread: *mut Thread, new_thread: *mut Thread) {
    sys_x86_extended_register_context_switch(old_thread, new_thread);
}

/// Configure Processor Trace state for threads
///
/// # Arguments
///
/// * `threads` - Whether to enable PT for threads
///
/// # Safety
///
/// This function is unsafe as it modifies system-wide processor trace settings.
pub unsafe fn x86_set_extended_register_pt_state(threads: bool) {
    sys_x86_set_extended_register_pt_state(threads);
}

/// Read the XCR extended control register
///
/// # Arguments
///
/// * `reg` - Register number to read
///
/// # Returns
///
/// The 64-bit value of the specified XCR register
pub unsafe fn x86_read_xcr(reg: u32) -> u64 {
    // TODO: Implement XCR register read
    0
}

// FFI declarations for extended register functions
extern "C" {
    fn sys_x86_extended_register_init();
    fn sys_x86_extended_register_size() -> usize;
    fn sys_x86_extended_register_enable_feature(feature: X86ExtendedRegisterFeature) -> bool;
    fn sys_x86_extended_register_init_state(buffer: *mut core::ffi::c_void);
    fn sys_x86_extended_register_save_state(buffer: *mut core::ffi::c_void);
    fn sys_x86_extended_register_restore_state(buffer: *const core::ffi::c_void);
    fn sys_x86_extended_register_context_switch(old_thread: *mut Thread, new_thread: *mut Thread);
    fn sys_x86_set_extended_register_pt_state(threads: bool);
}
