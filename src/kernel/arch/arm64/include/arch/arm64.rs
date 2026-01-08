// Copyright 2025 The Rustux Authors
// Copyright (c) 2014 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::rustux::compiler::*;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use crate::kernel::thread;

// Constants from ACLE section 8.3, used as the argument for __dmb(), __dsb(), and __isb()
// in arm_acle.h. Values are the architecturally defined immediate values encoded in barrier
// instructions DMB, DSB, and ISB.
pub const ARM_MB_OSHLD: u8 = 0x1;
pub const ARM_MB_OSHST: u8 = 0x2;
pub const ARM_MB_OSH: u8 = 0x3;

pub const ARM_MB_NSHLD: u8 = 0x5;
pub const ARM_MB_NSHST: u8 = 0x6;
pub const ARM_MB_NSH: u8 = 0x7;

pub const ARM_MB_ISHLD: u8 = 0x9;
pub const ARM_MB_ISHST: u8 = 0xa;
pub const ARM_MB_ISH: u8 = 0xb;

pub const ARM_MB_LD: u8 = 0xd;
pub const ARM_MB_ST: u8 = 0xe;
pub const ARM_MB_SY: u8 = 0xf;

extern "C" {
    pub fn arm64_context_switch(old_sp: *mut VAddr, new_sp: VAddr);

    pub fn arm64_uspace_entry(
        arg1: usize,
        arg2: usize,
        pc: usize,
        sp: usize,
        kstack: VAddr,
        spsr: u32,
        mdscr: u32,
    ) -> !;
}

#[repr(C)]
pub struct arm64_cache_desc_t {
    pub ctype: u8,
    pub write_through: bool,
    pub write_back: bool,
    pub read_alloc: bool,
    pub write_alloc: bool,
    pub num_sets: u32,
    pub associativity: u32,
    pub line_size: u32,
}

#[repr(C)]
pub struct arm64_cache_info_t {
    pub inner_boundary: u8,
    pub lou_u: u8,
    pub loc: u8,
    pub lou_is: u8,
    pub level_data_type: [arm64_cache_desc_t; 7],
    pub level_inst_type: [arm64_cache_desc_t; 7],
}

/* exception handling */
#[repr(C)]
pub struct arm64_iframe_long {
    pub r: [u64; 30],
    pub lr: u64,
    pub usp: u64,
    pub elr: u64,
    pub spsr: u64,
    pub mdscr: u64,
    pub pad2: [u64; 1], // Keep structure multiple of 16-bytes for stack alignment.
}

impl Default for arm64_iframe_long {
    fn default() -> Self {
        arm64_iframe_long {
            r: [0; 30],
            lr: 0,
            usp: 0,
            elr: 0,
            spsr: 0,
            mdscr: 0,
            pad2: [0],
        }
    }
}

#[repr(C)]
pub struct arm64_iframe_short {
    pub r: [u64; 20],
    // pad the short frame out so that it has the same general shape and size as a long
    pub pad: [u64; 10],
    pub lr: u64,
    pub usp: u64,
    pub elr: u64,
    pub spsr: u64,
    pub pad2: [u64; 2],
}

const _: () = assert!(core::mem::size_of::<arm64_iframe_long>() == core::mem::size_of::<arm64_iframe_short>(),
    "arm64_iframe_long and arm64_iframe_short must have same size");

#[repr(C)]
pub struct arch_exception_context {
    pub frame: *mut arm64_iframe_long,
    pub far: u64,
    pub esr: u32,
}

pub type iframe_t = arm64_iframe_long;
pub type iframe = arm64_iframe_short;
pub type RiscvIframe = arm64_iframe_long; // Type alias for compatibility

extern "C" {
    pub fn arm64_el1_exception_base();
    pub fn arm64_el3_to_el1();
    pub fn arm64_sync_exception(iframe: *mut arm64_iframe_long, exception_flags: u32, esr: u32);
    pub fn arm64_thread_process_pending_signals(iframe: *mut arm64_iframe_long);

    pub fn platform_irq(frame: *mut iframe);
    pub fn platform_fiq(frame: *mut iframe);

    /* fpu routines */
    pub fn arm64_fpu_exception(iframe: *mut arm64_iframe_long, exception_flags: u32);
    pub fn arm64_fpu_context_switch(oldthread: *mut thread::Thread, newthread: *mut thread::Thread);

    pub fn arm64_get_boot_el() -> u64;

    pub fn arm_reset();

    /*
     * Creates a stack and sets the stack pointer for the specified secondary CPU.
     */
    pub fn arm64_create_secondary_stack(cluster: u32, cpu: u32) -> Status;

    /*
     * Frees a stack created by |arm64_create_secondary_stack|.
     */
    pub fn arm64_free_secondary_stack(cluster: u32, cpu: u32) -> Status;
}

/* used in above exception_flags arguments */
pub const ARM64_EXCEPTION_FLAG_LOWER_EL: u32 = 1 << 0;
pub const ARM64_EXCEPTION_FLAG_ARM32: u32 = 1 << 1;