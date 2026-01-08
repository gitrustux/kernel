// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit Architecture Implementation
//!
//! This module provides the RISC-V-specific implementation of the
//! Architecture Abstraction Layer (AAL).

#![no_std]

// Core architecture modules
pub mod aal;
pub mod arch;
pub mod debugger;
pub mod exceptions_c;
pub mod feature;
pub mod fpu;
pub mod mmu;
pub mod mp;
pub mod page_table;
pub mod periphmap;
pub mod plic;
pub mod registers;
pub mod spinlock;
pub mod thread;
pub mod user_copy_c;

// Include directory for public definitions
pub mod include;

// Boot-related modules
pub mod boot_mmu;

// Re-exports
pub use aal::Riscv64Arch;
