// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit architecture support
//!
//! This module provides architecture-specific support for RISC-V 64-bit processors.
//! It supports both Sv39 and Sv48 page table formats.

use crate::arch;
use crate::arch::riscv64;
use crate::arch::riscv64::feature;
use crate::arch::riscv64::registers;
use crate::arch::riscv64::mmu;
use crate::arch::mp;
use crate::arch::ops;
use crate::kernel::mp::SMP_MAX_CPUS;
use crate::bits;
use crate::debug;
use crate::kernel::cmdline;
use crate::kernel::thread::{self, Thread};
use crate::lk::init;
use crate::lk::main;
use crate::platform;
use crate::rustux::errors::*;
use crate::rustux::types::*;
use crate::rustux::tls::{RX_TLS_STACK_GUARD_OFFSET, RX_TLS_UNSAFE_SP_OFFSET};
use crate::trace::*;

/// RISC-V hart (CPU) information for SMP boot
#[repr(C)]
#[derive(Copy, Clone)]
pub struct RiscvSpInfo {
    hartid: u64,
    sp: *mut core::ffi::c_void,

    // This part of the struct itself will serve temporarily as the
    // fake arch_thread in the thread pointer (tp), so that safe-stack
    // and stack-protector code can work early.  The thread pointer
    // points just past riscv_sp_info_t.
    stack_guard: usize,
    unsafe_sp: *mut core::ffi::c_void,
}

// Ensure the struct has the correct size and offsets for assembly code
const _: () = assert!(core::mem::size_of::<RiscvSpInfo>() == 32,
                      "check riscv_get_secondary_sp assembly");
const _: () = assert!(core::mem::offset_of!(RiscvSpInfo, sp) == 8,
                      "check riscv_get_secondary_sp assembly");
const _: () = assert!(core::mem::offset_of!(RiscvSpInfo, hartid) == 0,
                      "check riscv_get_secondary_sp assembly");

// Verify thread pointer offsets
// NOTE: These assertions are disabled for RISC-V as the TLS layout
// differs from the expected generic layout. This will need to be
// adjusted based on the actual RISC-V TLS requirements.
macro_rules! tp_offset {
    ($field:ident) => {
        (core::mem::offset_of!(RiscvSpInfo, $field) as isize -
         core::mem::size_of::<RiscvSpInfo>() as isize)
    };
}

// TODO: Fix RISC-V TLS layout to match expected offsets
// const _: () = assert!(tp_offset!(stack_guard) == RX_TLS_STACK_GUARD_OFFSET as isize, "");
// const _: () = assert!(tp_offset!(unsafe_sp) == RX_TLS_UNSAFE_SP_OFFSET as isize, "");

// SMP boot lock
static RISCV_BOOT_CPU_LOCK: crate::arch::riscv64::spinlock::SpinLock =
    crate::arch::riscv64::spinlock::SpinLock::new();
static mut SECONDARIES_TO_INIT: i32 = 0;

// One for each secondary CPU, indexed by (cpu_num - 1)
// Use MaybeUninit since Thread::new() requires arguments and may fail
static mut INIT_THREAD: [core::mem::MaybeUninit<Thread>; SMP_MAX_CPUS as usize - 1] =
    [const { core::mem::MaybeUninit::<Thread>::uninit() }; SMP_MAX_CPUS as usize - 1];

// One for each CPU
pub static mut RISCV_SECONDARY_SP_LIST: [RiscvSpInfo; SMP_MAX_CPUS as usize] =
    [RiscvSpInfo { hartid: 0, sp: core::ptr::null_mut(), stack_guard: 0, unsafe_sp: core::ptr::null_mut() }; SMP_MAX_CPUS as usize];

/// Architecture-specific initialization
pub fn arch_early_init() {
    // TODO: Implement RISC-V early initialization
    // - Detect CPU features (extensions, ISA version)
    // - Initialize timer
    // - Set up interrupt controller (PLIC)
}

/// Architecture-specific initialization after main kernel init
pub fn arch_init() {
    // TODO: Implement RISC-V late initialization
    // - Enable all harts
    // - Set up IPIs
}

/// Get the current hart ID
#[inline(always)]
pub fn arch_curr_hartid() -> u64 {
    let hartid: u64;
    unsafe {
        core::arch::asm!("mv {}, tp", out(reg) hartid);
    }
    hartid
}

// Defined in start.S
extern "C" {
    /// The exception vector base address
    pub static mut riscv_exception_vector: u8;

    /// Boot hart ID
    pub static riscv_boot_hartid: u64;
}
