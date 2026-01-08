// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux Microkernel - Main Entry Point
//!
//! This is the main entry point for the Rustux microkernel.
//! The actual kernel initialization happens in the kernel module.

#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![feature(asm)]
#![feature(asm_experimental_arch)]
#![feature(register_tool)]
#![register_tool(no_sanitize)]
#![register_tool(no_return)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_unsafe)]
#![allow(static_mut_refs)]
#![allow(unused_results)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(anonymous_parameters)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(asm_sub_register)]
#![allow(clashing_extern_declarations)]
#![allow(unreachable_code)]
#![allow(unreachable_patterns)]
#![allow(unused_parens)]
#![allow(unused_mut)]
#![allow(dropping_copy_types)]
#![allow(non_snake_case)]
#![allow(ambiguous_glob_reexports)]
#![allow(unused_macros)]
#![allow(elided_lifetimes_in_paths)]
#![allow(warnings)] // Suppress all remaining warnings

extern crate alloc;

use core::panic::PanicInfo;

// ============================================================================
// Kernel Constants for Linker Script
// ============================================================================

/// Kernel base address (identity mapped at 1MB for QEMU boot)
#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub static KERNEL_BASE: u64 = 0x0010_0000;

/// Boot header size
#[no_mangle]
pub static BOOT_HEADER_SIZE: u64 = 0x50;

/// Maximum number of CPUs (as a constant for use in const contexts)
pub const SMP_MAX_CPUS: u64 = 64;

/// Maximum number of CPUs (as a static for C FFI)
#[no_mangle]
pub static SMP_MAX_CPUS_STATIC: u64 = 64;

// Extern reference to multiboot header to ensure it's linked
#[link_section = ".multiboot"]
extern "C" {
    #[link_name = "multiboot_header"]
    static MULTIBOOT_HEADER: [u8; 12];
}

// Common types
mod rustux;

// Debug support
mod debug;

// Trace support (re-exports debug macros)
mod trace;

// Error codes
mod err;

// Bit manipulation utilities
mod bits;

// Compatibility modules (LK, Platform, FBL)
mod lk;
mod platform;
mod fbl;

// Sys module for compatibility with code using crate::sys::types
pub mod sys {
    // types submodule for crate::sys::types::* imports
    pub mod types {
        pub use crate::rustux::types::*;

        // C-style type aliases for compatibility
        pub use crate::rustux::VAddr as vaddr_t;
        pub use crate::rustux::PAddr as paddr_t;
        pub use crate::rustux::Size as size_t;
        pub use crate::rustux::SSize as ssize_t;
        pub use crate::rustux::Status as rx_status_t;
        pub use crate::rustux::UIntPtr as uintptr_t;
        pub use crate::rustux::VAddr as rx_vaddr_t;
        pub use crate::rustux::types::err::*;

        // Generic address type
        pub type addr_t = u64;
    }

    /// Exception type alias
    pub type rx_excp_type_t = u32;

    /// Thread state types for debugger
    #[repr(C)]
    pub struct rx_thread_state_general_regs_t {
        pub r: [u64; 30],
        pub lr: u64,
        pub pc: u64,
        pub sp: u64,
        pub cpsr: u32,
        pub padding: u32,
    }

    #[repr(C)]
    pub struct rx_thread_state_vector_regs_t {
        pub v: [VectorReg; 32],
        pub fpsr: u32,
        pub fpcr: u32,
        pub padding: u32,
    }

    /// Vector register (split into low/high 64-bit parts)
    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    pub struct VectorReg {
        pub low: u64,
        pub high: u64,
    }

    #[repr(C)]
    pub struct rx_thread_state_fp_regs_t {
        pub d: [u64; 32],
    }

    #[repr(C)]
    pub struct rx_thread_state_debug_regs_t {
        pub hw_bps_count: u32,
        pub padding: u32,
        pub hw_bps: [Arm64HwBreakpoint; 16],
        pub bvr: [u64; 16],
        pub bcr: [u64; 16],
        pub wvr: [u64; 16],
        pub wcr: [u64; 16],
    }

    /// ARM64 hardware breakpoint
    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    pub struct Arm64HwBreakpoint {
        pub dbgbcr: u32,
        pub dbgbvr: u64,
    }

    // Also re-export types directly at sys level
    pub use crate::rustux::types::*;
    pub use crate::rustux::types::err::*;

    // C-style type aliases at sys level too
    pub use crate::rustux::VAddr as vaddr_t;
    pub use crate::rustux::PAddr as paddr_t;
    pub use crate::rustux::Size as size_t;
    pub use crate::rustux::SSize as ssize_t;
    pub use crate::rustux::Status as rx_status_t;
    pub use crate::rustux::UIntPtr as uintptr_t;
    pub use crate::rustux::VAddr as rx_vaddr_t;

    /// Generic address type (for compatibility)
    pub type addr_t = u64;
}

// Utility modules (stubs for C++ library compatibility)
mod bitmap;
mod rand;
mod reg;

// Define macros at crate root for availability in all modules
#[macro_export]
macro_rules! println {
    () => {
        $crate::kernel::debug::print_internal("\n");
    };
    ($fmt:expr) => {
        let _ = core::fmt::Write::write_fmt(&mut $crate::kernel::debug::LogWriter, format_args!($fmt));
        $crate::kernel::debug::print_internal("\n");
    };
    ($fmt:expr, $($arg:tt)*) => {
        let _ = core::fmt::Write::write_fmt(&mut $crate::kernel::debug::LogWriter, format_args!($fmt, $($arg)*));
        $crate::kernel::debug::print_internal("\n");
    };
}

#[macro_export]
macro_rules! print {
    ($fmt:expr) => {
        let _ = core::fmt::Write::write_fmt(&mut $crate::kernel::debug::LogWriter, format_args!($fmt));
    };
    ($fmt:expr, $($arg:tt)*) => {
        let _ = core::fmt::Write::write_fmt(&mut $crate::kernel::debug::LogWriter, format_args!($fmt, $($arg)*));
    };
}

#[macro_export]
macro_rules! ltrace {
    ($($arg:tt)*) => {
        if false {
            $crate::println!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::println!($($arg)*);
    };
}

#[macro_export]
macro_rules! static_assert {
    ($cond:expr) => {
        const _: [(); 0 - !$cond as usize] = [];
    };
    ($cond:expr, $msg:expr) => {
        const _: [(); 0 - !$cond as usize] = [$msg; 0];
    };
}

#[macro_export]
macro_rules! goto_err {
    () => {
        unreachable!("goto_err called - refactor to use Result/Option");
    };
    ($label:ident) => {
        unreachable!("goto_err called - refactor to use Result/Option");
    };
}

#[macro_export]
macro_rules! static_command {
    ($name:expr, $help:expr, $cmd:expr) => {
        const _STATIC_CMD_NAME: &'static str = $name;
        const _STATIC_CMD_HELP: &'static str = $help;
        const _STATIC_CMD_CMD: &'static str = $cmd;
    };
    ($name:expr, $help:expr) => {
        const _STATIC_CMD_NAME: &'static str = $name;
        const _STATIC_CMD_HELP: &'static str = $help;
    };
}

#[macro_export]
macro_rules! const_assert_eq {
    ($left:expr, $right:expr) => {
        const _: [(); 0 - ($left != $right) as usize] = [];
    };
}

#[macro_export]
macro_rules! bits {
    ($val:expr, $high:expr, $low:expr) => {
        (($val >> $low) & ((1 << ($high - $low + 1)) - 1))
    };
}

#[macro_export]
macro_rules! bit {
    ($val:expr, $bit:expr) => {
        (($val >> $bit) & 1)
    };
}

#[macro_export]
macro_rules! ARM64_TLBI {
    ($op:ident, $val:expr) => {
        unsafe {
            core::arch::asm!(concat!("tlbi ", stringify!($op), ", {}"), in(reg) $val, options(nostack));
        }
    };
}

#[macro_export]
macro_rules! CPU_STATS_INC {
    ($name:ident) => {
        // Stub for CPU statistics increment
        // TODO: Implement per-CPU statistics tracking
    };
}

// ============================================================================
// LK Compatibility Constants
// ============================================================================

/// LK debug level
pub const LK_DEBUGLEVEL: u32 = 0;

// Kernel modules
mod kernel;

// Re-export commonly used modules at crate level for compatibility
pub use kernel::arch;
pub use kernel::vm;
pub use kernel::lib;
pub use kernel::exception;
pub use kernel::user_copy;
pub use kernel::mmu;

/// Kernel entry point
///
/// This function is called by the bootloader after setting up
/// a basic execution environment.
#[no_mangle]
pub extern "C" fn kmain() -> ! {
    // Initialize the kernel
    kernel::init();

    // If we reach here, something went wrong
    loop {
        core::hint::spin_loop();
    }
}

/// Panic handler
///
/// This function is called when the kernel encounters a panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Exception handling personality function
///
/// This is required by the compiler even though we don't use exceptions.
#[no_mangle]
pub extern "C" fn rust_eh_personality() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
