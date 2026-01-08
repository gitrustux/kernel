// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux Microkernel - Core Kernel Module
//!
//! This module contains the core kernel functionality.


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

// Re-export arch submodules for compatibility
#[cfg(target_arch = "aarch64")]
pub use arch::arm64;

#[cfg(target_arch = "x86_64")]
pub use arch::amd64;

#[cfg(target_arch = "riscv64")]
pub use arch::riscv64;

// Re-export arch traits at kernel level
pub use crate::arch::arch_traits::*;

// Device drivers
pub mod dev;

// Core kernel modules (these need to be declared properly)
//
// Note: This is a minimal module declaration to allow building.
// The full module structure needs to be completed.
pub mod allocator;
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

// Re-export usercopy as user_copy for compatibility
pub use usercopy as user_copy;

// Add missing modules
pub mod exception;
pub mod mmu;
pub mod lib;
pub mod mutex;
pub mod thread_lock;
pub mod cpu;
pub mod align;
pub mod spinlock;

// Re-export vm submodules for compatibility (if they exist)
pub use vm::arch_vm_aspace;

// Re-export event and interrupt modules for compatibility
pub use sync::event;
pub use dev::interrupt;

// arch_zero_page doesn't exist yet - will need to be created or stubbed

/// Kernel initialization
///
/// This is the main initialization function called from kmain().
pub fn init() {
    // Initialize the heap allocator first
    allocator::init();

    // Early initialization
    arch::arch_early_init();

    // Platform initialization
    // TODO: Initialize platform-specific code

    // Main initialization
    arch::arch_init();

    // TODO: Initialize remaining kernel subsystems
}
