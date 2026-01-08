// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture Abstraction Layer (AAL)
//!
//! This module provides architecture-specific implementations through
//! a common abstraction layer.

#![no_std]

// Architecture traits (interface)
pub mod arch_traits;

// Architecture-specific implementations
#[cfg(target_arch = "aarch64")]
pub mod arm64;

#[cfg(target_arch = "x86_64")]
pub mod amd64;

#[cfg(target_arch = "riscv64")]
pub mod riscv64;

// Re-export commonly used items from the top-level rustux module
pub use crate::rustux::types::*;

// Re-export commonly used constants from vm::layout
pub use crate::kernel::vm::layout::{PAGE_SIZE, PAGE_SIZE_SHIFT, PAGE_MASK};

// Architecture-specific kernel base addresses
#[cfg(target_arch = "x86_64")]
pub const KERNEL_BASE: u64 = 0xFFFF_8000_0000_0000;

#[cfg(target_arch = "aarch64")]
pub const KERNEL_BASE: u64 = 0xFFFF_0000_0000_0000;

#[cfg(target_arch = "riscv64")]
pub const KERNEL_BASE: u64 = 0xFFFF_0000_0000_0000;

// Kernel load offset (typically 0 for identity-mapped kernels)
pub const KERNEL_LOAD_OFFSET: u64 = 0;

/// Early architecture initialization
pub fn arch_early_init() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        amd64::arch_early_init();
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        arm64::arch_early_init();
    }

    #[cfg(target_arch = "riscv64")]
    unsafe {
        riscv64::arch_early_init();
    }
}

/// Main architecture initialization
pub fn arch_init() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        amd64::arch_init();
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        arm64::arch_init();
    }

    #[cfg(target_arch = "riscv64")]
    unsafe {
        riscv64::arch_init();
    }
}

// Re-export arch operations for compatibility
#[cfg(target_arch = "aarch64")]
pub use arm64::include::arch::arch_ops as ops;

#[cfg(target_arch = "x86_64")]
pub use amd64::include::arch::arch_ops as ops;

#[cfg(target_arch = "riscv64")]
pub use riscv64::include::arch::arch_ops as ops;

// Re-export commonly used functions from arch_ops
#[cfg(target_arch = "aarch64")]
pub use arm64::include::arch::arch_ops::{arch_curr_cpu_num};

#[cfg(target_arch = "x86_64")]
pub use amd64::include::arch::arch_ops;

#[cfg(target_arch = "riscv64")]
pub use riscv64::include::arch::arch_ops::{arch_curr_cpu_num};

// Re-export kernel modules for compatibility
pub use crate::kernel::mp;
pub use crate::kernel::exception;

// Re-export architecture-specific mmu and aspace modules
#[cfg(target_arch = "aarch64")]
pub use arm64::mmu;

#[cfg(target_arch = "x86_64")]
pub use amd64::mmu;

#[cfg(target_arch = "riscv64")]
pub use riscv64::mmu;

#[cfg(target_arch = "aarch64")]
pub use arm64::aspace;

#[cfg(target_arch = "x86_64")]
pub use amd64::include::arch::aspace;

#[cfg(target_arch = "riscv64")]
pub use riscv64::aspace;

// Re-export arm64 submodules for compatibility
// Note: interrupt module doesn't exist, only interrupts.rs
// Use interrupts module from arm64 root instead
#[cfg(target_arch = "aarch64")]
pub use arm64::interrupts;

#[cfg(target_arch = "aarch64")]
pub use arm64::exceptions;

// Re-export architecture-specific user_copy for compatibility
#[cfg(target_arch = "aarch64")]
pub use arm64::user_copy;

#[cfg(target_arch = "x86_64")]
pub use crate::kernel::arch::amd64::include::arch::amd64::user_copy;

#[cfg(target_arch = "riscv64")]
pub use riscv64::user_copy;
