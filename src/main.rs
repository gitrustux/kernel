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

/// Maximum number of CPUs
#[no_mangle]
pub static SMP_MAX_CPUS: u64 = 64;

// Extern reference to multiboot header to ensure it's linked
#[link_section = ".multiboot"]
extern "C" {
    #[link_name = "multiboot_header"]
    static MULTIBOOT_HEADER: [u8; 12];
}

// Common types
mod rustux;

// Compatibility modules (LK, Platform, FBL)
mod lk;
mod platform;
mod fbl;

// Kernel modules
mod kernel;

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
