// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux Microkernel - Core Kernel Module
//!
//! This module contains the core kernel functionality.

#![no_std]

// Re-export commonly used types
pub use crate::rustux::types::*;

// Common type aliases for kernel convenience
pub use crate::kernel::sync::spin::SpinMutex as Mutex;
pub use crate::kernel::vm::{VmError, Result as VmResult};
pub use alloc::vec::Vec;
pub use alloc::string::String;
pub use core::sync::atomic::AtomicUsize;
pub use core::sync::atomic::AtomicU64;
pub use core::sync::atomic::AtomicBool;

// Architecture module
pub mod arch;

// Device drivers
pub mod dev;

// Core kernel modules (these need to be declared properly)
//
// Note: This is a minimal module declaration to allow building.
// The full module structure needs to be completed.
pub mod cmdline;
pub mod debug;
pub mod dpc;
pub mod hypervisor;
pub mod init;
pub mod mp;
pub mod object;
pub mod percpu;
pub mod pmm;
pub mod process;
pub mod sched;
pub mod sync;
pub mod syscalls;
pub mod thread;
pub mod timer;
pub mod usercopy;
pub mod vm;

/// Kernel initialization
///
/// This is the main initialization function called from kmain().
pub fn init() {
    // Early initialization
    arch::arch_early_init();

    // Platform initialization
    // TODO: Initialize platform-specific code

    // Main initialization
    arch::arch_init();

    // TODO: Initialize remaining kernel subsystems
}
