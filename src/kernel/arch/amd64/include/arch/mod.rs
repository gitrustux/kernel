// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture-specific includes

#![no_std]

// Re-export amd64 module
pub mod amd64;

// Declare other modules in the arch directory
pub mod arch_ops;
pub mod arch_thread;
pub mod aspace;
pub mod spinlock;
pub mod current_thread;
pub mod defines;
// pub mod asm_macros;  // Assembly macros, not Rust code
pub mod hypervisor;
