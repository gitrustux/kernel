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
