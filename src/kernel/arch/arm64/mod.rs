// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 (AArch64) Architecture Implementation
//!
//! This module provides the ARM64-specific implementation of the
//! Architecture Abstraction Layer (AAL).

#![no_std]

// Core architecture modules
pub mod aal;
pub mod arch;
pub mod aspace;
pub mod debugger;
pub mod el2_state;
pub mod exceptions;
pub mod exceptions_c;
pub mod feature;
pub mod fpu;
pub mod interrupts;
pub mod mmu;
pub mod mp;
pub mod periphmap;
pub mod registers;
pub mod spinlock;
pub mod sysreg;
pub mod thread;
pub mod timer;
pub mod user_copy_c;

// Re-exports
pub use aal::{Arm64Arch, AArch64Arch};

// Re-export user_copy_c as user_copy for compatibility
pub use user_copy_c as user_copy;

// Boot-related modules
pub mod boot_mmu;

// Include directory for architecture-specific headers
pub mod include {
    pub mod arch {
        pub mod arch_ops;
        pub mod arm64;
        pub mod arch_thread;
        pub mod aspace;
        pub mod defines;
        pub mod hypervisor;
        pub mod spinlock;
        pub mod current_thread;
        pub mod asm_macros;
    }
}

// Re-export commonly used types from include/arch/arm64.rs
pub use include::arch::arm64::{
    arm64_iframe_long,
    arm64_iframe_short,
    arm64_cache_info_t,
    arm64_cache_desc_t,
    arch_exception_context,
    ARM64_EXCEPTION_FLAG_LOWER_EL,
    ARM64_EXCEPTION_FLAG_ARM32,
    iframe_t,
    iframe,
    RiscvIframe,  // Re-export for cross-arch compatibility
};

// Import commonly used types
use crate::rustux::types::VAddr;

// Re-export commonly used types from mmu module
pub use mmu::pte_t;

// Re-export commonly used functions from arch_ops
pub use include::arch::arch_ops::{
    arch_curr_cpu_num,
    arch_is_user_address,
};

// ARM64 ISA feature flags (from ID_AA64ISFR0_EL1 register)
pub const ARM64_FEATURE_ISA_FP: u64 = 1 << 0;      // Floating point
pub const ARM64_FEATURE_ISA_ASIMD: u64 = 1 << 1;   // Advanced SIMD
pub const ARM64_FEATURE_ISA_CRC32: u64 = 1 << 2;   // CRC32
pub const ARM64_FEATURE_ISA_SHA1: u64 = 1 << 3;    // SHA1
pub const ARM64_FEATURE_ISA_SHA2: u64 = 1 << 4;    // SHA2
pub const ARM64_FEATURE_ISA_SHA256: u64 = 1 << 5;  // SHA256
pub const ARM64_FEATURE_ISA_SHA512: u64 = 1 << 6;  // SHA512
pub const ARM64_FEATURE_ISA_PMULL: u64 = 1 << 7;   // Polynomial multiply long
pub const ARM64_FEATURE_ISA_AES: u64 = 1 << 8;     // AES
pub const ARM64_FEATURE_ISA_SEV: u64 = 1 << 9;     // Scalable Vector Extension
pub const ARM64_FEATURE_ISA_SM3: u64 = 1 << 10;    // SM3
pub const ARM64_FEATURE_ISA_SM4: u64 = 1 << 11;    // SM4
pub const ARM64_FEATURE_ISA_RDM: u64 = 1 << 12;    // Round Multiply Multiply
pub const ARM64_FEATURE_ISA_DOTPROD: u64 = 1 << 13; // Dot product
pub const ARM64_FEATURE_ISA_FCMA: u64 = 1 << 14;   // Floating point complex multiply add
pub const ARM64_FEATURE_ISA_SHA3: u64 = 1 << 15;   // SHA3
pub const ARM64_FEATURE_ISA_DP: u64 = 1 << 16;     // Dot product (alternative name)
pub const ARM64_FEATURE_ISA_DPB: u64 = 1 << 17;    // Data barrier (Data Persistence)
pub const ARM64_FEATURE_ISA_SVE: u64 = 1 << 18;    // Scalable Vector Extension
pub const ARM64_FEATURE_ISA_SVE2: u64 = 1 << 19;   // Scalable Vector Extension 2
pub const ARM64_FEATURE_ISA_ATOMICS: u64 = 1 << 20; // Large System Extensions
pub const ARM64_FEATURE_ISA_LR: u64 = 1 << 21;     // LDAP0/StLR instructions
pub const ARM64_FEATURE_ISA_FP16: u64 = 1 << 22;   // Half precision floating point
pub const ARM64_FEATURE_ISA_BF16: u64 = 1 << 23;   // BFloat16

// Legacy aliases for compatibility
pub const RX_ARM64_FEATURE_ISA_FP: u64 = ARM64_FEATURE_ISA_FP;
pub const RX_ARM64_FEATURE_ISA_ASIMD: u64 = ARM64_FEATURE_ISA_ASIMD;
pub const RX_ARM64_FEATURE_ISA_CRC32: u64 = ARM64_FEATURE_ISA_CRC32;
pub const RX_ARM64_FEATURE_ISA_SHA1: u64 = ARM64_FEATURE_ISA_SHA1;
pub const RX_ARM64_FEATURE_ISA_SHA2: u64 = ARM64_FEATURE_ISA_SHA2;
pub const RX_ARM64_FEATURE_ISA_PMULL: u64 = ARM64_FEATURE_ISA_PMULL;
pub const RX_ARM64_FEATURE_ISA_AES: u64 = ARM64_FEATURE_ISA_AES;
pub const RX_ARM64_FEATURE_ISA_SM3: u64 = ARM64_FEATURE_ISA_SM3;
pub const RX_ARM64_FEATURE_ISA_SM4: u64 = ARM64_FEATURE_ISA_SM4;
pub const RX_ARM64_FEATURE_ISA_RDM: u64 = ARM64_FEATURE_ISA_RDM;
pub const RX_ARM64_FEATURE_ISA_DP: u64 = ARM64_FEATURE_ISA_DP;
pub const RX_ARM64_FEATURE_ISA_DPB: u64 = ARM64_FEATURE_ISA_DPB;
pub const RX_ARM64_FEATURE_ISA_SHA3: u64 = ARM64_FEATURE_ISA_SHA3;
pub const RX_ARM64_FEATURE_ISA_ATOMICS: u64 = ARM64_FEATURE_ISA_ATOMICS;

// IRQ exit flags
pub const ARM64_IRQ_EXIT_THREAD_SIGNALED: u32 = 0x1;
pub const ARM64_IRQ_EXIT_RESCHEDULE: u32 = 0x2;

// Hardware breakpoints
pub const ARM64_MAX_HW_BREAKPOINTS: usize = 16;
pub const ARM64_MAX_HW_WATCHPOINTS: usize = 16;

// MPIDR register affinity field shifts and masks
pub const MPIDR_AFF3_SHIFT: u64 = 32;
pub const MPIDR_AFF3_MASK: u64 = 0xFF << MPIDR_AFF3_SHIFT;
pub const MPIDR_AFF2_SHIFT: u64 = 16;
pub const MPIDR_AFF2_MASK: u64 = 0xFF << MPIDR_AFF2_SHIFT;
pub const MPIDR_AFF1_SHIFT: u64 = 8;
pub const MPIDR_AFF1_MASK: u64 = 0xFF << MPIDR_AFF1_SHIFT;
pub const MPIDR_AFF0_SHIFT: u64 = 0;
pub const MPIDR_AFF0_MASK: u64 = 0xFF << MPIDR_AFF0_SHIFT;

// MMU ASID constants (re-exported from mmu module)
pub const MMU_ARM64_ASID_BITS: u32 = 16;
pub const MMU_ARM64_GLOBAL_ASID: u32 = 0;
pub const MMU_ARM64_MAX_USER_ASID: u32 = (1 << 16) - 2;

// ARM64 feature flags
pub const RX_HAS_CPU_FEATURES: u64 = 1;

// CPU feature register masks
pub const ARM64_MMFR0_ASIDBITS_MASK: u64 = 0xF << 4;
pub const ARM64_MMFR0_ASIDBITS_16: u64 = 0b0010 << 4;

// Exception base address marker
pub const arm64_el1_exception_base: u64 = 0;

// ARM64 ZVA (Zero Vector Area) size for cache operations
pub const arm64_zva_size: u32 = 64;

// Re-export SMP_MAX_CPUS from the kernel module
pub use crate::kernel::mp::SMP_MAX_CPUS;

// Re-export get_current_thread from thread module for convenience
pub use crate::kernel::thread::get_current_thread;

// Re-export interrupt functions from interrupts module
pub use interrupts::{
    arch_ints_disabled,
    arch_enable_ints,
    arch_disable_ints,
    arch_enable_fiqs,
    arch_save_ints,
    arch_restore_ints,
};

// Re-export arch_ops functions
pub use include::arch::arch_ops::{
    arch_clean_cache_range,
    arch_interrupt_save,
    arch_interrupt_restore,
};

// arm64_cpu_count will be provided by the mp module
// Add it as a function in the arm64 module that forwards to mp
pub use mp::{
    arm64_cpu_count,
    arm64_mp_reschedule,
    arm64_prepare_cpu_idle,
    arm64_mp_cpu_hotplug,
    arm64_mp_cpu_unplug,
};

// Re-export feature functions
pub use feature::arm64_get_features;

// ============================================================================
// Per-CPU Type
// ============================================================================

/// ARM64 per-CPU data structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct arm64_percpu {
    /// Reserved space for per-CPU data
    pub data: [u64; 64],
}

impl Default for arm64_percpu {
    fn default() -> Self {
        Self { data: [0; 64] }
    }
}

// ============================================================================
// Debug State Type
// ============================================================================

/// ARM64 hardware breakpoint
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Arm64HwBreakpoint {
    pub dbgbcr: u32,
    pub dbgbvr: u64,
}

/// ARM64 debug state (for hardware breakpoints and watchpoints)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct arm64_debug_state_t {
    /// Hardware breakpoints
    pub hw_bps: [Arm64HwBreakpoint; ARM64_MAX_HW_BREAKPOINTS],
    /// Debug breakpoint control registers
    pub bcr: [u64; ARM64_MAX_HW_BREAKPOINTS],
    /// Debug breakpoint value registers
    pub bvr: [u64; ARM64_MAX_HW_BREAKPOINTS],
    /// Debug watchpoint control registers
    pub wcr: [u64; ARM64_MAX_HW_WATCHPOINTS],
    /// Debug watchpoint value registers
    pub wvr: [u64; ARM64_MAX_HW_WATCHPOINTS],
    /// MDSCR register
    pub mdscr: u64,
}

impl Default for arm64_debug_state_t {
    fn default() -> Self {
        Self {
            hw_bps: [Arm64HwBreakpoint::default(); ARM64_MAX_HW_BREAKPOINTS],
            bcr: [0; ARM64_MAX_HW_BREAKPOINTS],
            bvr: [0; ARM64_MAX_HW_BREAKPOINTS],
            wcr: [0; ARM64_MAX_HW_WATCHPOINTS],
            wvr: [0; ARM64_MAX_HW_WATCHPOINTS],
            mdscr: 0,
        }
    }
}

// ============================================================================
// Per-CPU Data Access (Stubs)
// ============================================================================

/// Read per-CPU pointer
pub fn arm64_read_percpu_ptr<T>(_offset: usize) -> *mut T {
    // TODO: Implement per-CPU data access
    core::ptr::null_mut()
}

/// Write per-CPU pointer
pub fn arm64_write_percpu_ptr<T>(_offset: usize, _value: *mut T) {
    // TODO: Implement per-CPU data access
}

// ============================================================================
// Early/Late Initialization (Stubs)
// ============================================================================

/// Early architecture initialization
pub fn arch_early_init() {
    // TODO: Implement early init
}

/// Architecture initialization (main init function)
pub fn arch_init() {
    // TODO: Implement arch init
    arch_early_init();
}

/// Late architecture initialization
pub fn arch_late_init() {
    // TODO: Implement late init
}

/// Per-CPU early initialization
pub fn arm64_init_percpu_early() {
    // TODO: Implement per-CPU early init
}

// ============================================================================
// Debug Support (Stubs)
// ============================================================================

/// Get hardware breakpoint count
pub fn arm64_hw_breakpoint_count() -> u32 {
    // TODO: Read from hardware
    0
}

/// Validate debug state
pub fn arm64_validate_debug_state(_state: *const core::ffi::c_void) -> i32 {
    // TODO: Implement debug state validation
    0
}

// ============================================================================
// User Space Entry (Stubs)
// ============================================================================

/// Enter user space
pub fn arch_enter_uspace(_pc: u64, _sp: u64, _arg1: u64, _arg2: u64) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Return from exception to user space
pub fn arch_uspace_exception_return() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// User space entry point (stub - real implementation is in assembly)
/// This matches the signature of the C function declared in include/arch/arm64.rs
pub fn arm64_uspace_entry(
    _arg1: usize,
    _arg2: usize,
    _pc: usize,
    _sp: usize,
    _kstack: VAddr,
    _spsr: u32,
    _mdscr: u32,
) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// ============================================================================
// FPU Functions (Re-exports from fpu module)
// ============================================================================

pub use fpu::{
    arm64_fpu_init,
    arm64_fpu_save,
    arm64_fpu_restore,
    arm64_fpu_enabled,
    arm64_fpu_enable,
    arm64_fpu_disable,
    arm64_fpu_exception,
    fpstate,
    Arm64FpuState,
};

// ============================================================================
// Thread Context (Stubs)
// ============================================================================

/// ARM64 thread context type
pub type arm64_thread_context_t = u64;

/// Initialize thread
pub fn arch_thread_initialize(_thread: *mut core::ffi::c_void, _entry_point: u64, _arg: u64, _stack_top: u64) {
    // TODO: Implement thread initialization
}

/// Save thread context
pub fn arch_thread_context_save(_context: *mut arm64_thread_context_t) {
    // TODO: Implement context save
}

/// Restore thread context
pub fn arch_thread_context_restore(_context: *const arm64_thread_context_t) {
    // TODO: Implement context restore
}

/// Switch to a new thread
pub fn arch_thread_context_switch(_old: *mut arm64_thread_context_t, _new: *const arm64_thread_context_t) {
    // TODO: Implement context switch
}

// ============================================================================
// Exception Handling (Stubs)
// ============================================================================

// EL2 TLB Functions (Stubs)
// ============================================================================

/// TLBI by VMID (EL2)
pub fn arm64_el2_tlbi_vmid(_vttbr: u64) -> i32 {
    // TODO: Implement EL2 TLBI
    0
}

/// TLBI by IPA (EL2)
pub fn arm64_el2_tlbi_ipa(_ipa: u64) -> i32 {
    // TODO: Implement EL2 TLBI
    0
}

// ============================================================================
// Copy to/from User (Re-exports from user_copy_c module)
// ============================================================================

pub use user_copy_c::{
    arm64_copy_to_user,
    arm64_copy_from_user,
};

// ============================================================================
// MPID Functions (Stubs)
// ============================================================================

/// Extract MPID from MPIDR register value
pub const fn ARM64_MPID(_mpidr: u64) -> u64 {
    // TODO: Implement MPID extraction
    0
}
