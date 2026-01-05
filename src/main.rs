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
