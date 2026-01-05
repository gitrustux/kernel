// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! AMD64 (x86-64) Architecture Implementation
//!
//! This module provides the AMD64-specific implementation of the
//! Architecture Abstraction Layer (AAL).

#![no_std]

// FFI bridge to C code (sys_x86.c)
pub mod ffi;

// Core architecture modules
pub mod aal;
pub mod apic;
pub mod arch;
pub mod asm;
pub mod bootstrap16;
pub mod cache;
pub mod debugger;
pub mod descriptor;
pub mod faults;
pub mod feature;
pub mod interrupts;
pub mod ioport;
pub mod mmu;
pub mod mp;
pub mod ops;
pub mod page_tables;
pub mod registers;
pub mod smp;
// pub mod syscalls;  // TODO: Implement syscalls module
pub mod timer;
pub mod tsc;
pub mod uspace_entry;

// Sub-modules
// pub mod hypervisor;  // TODO: Implement hypervisor module

// Include directory contains public API re-exports
pub mod include {
    pub mod arch {
        pub mod amd64 {
            // These are re-exports from the include directory
            // Note: Can't glob import from here due to circular dependency
            // pub use crate::kernel::arch::amd64::include::arch::amd64::*;
        }

        pub mod arch_ops;
        pub mod arch_thread;
        pub mod aspace;
        pub mod spinlock;
        pub mod current_thread;
        pub mod defines;
        // pub mod asm_macros;  // Assembly macros, not Rust code
        pub mod hypervisor;
    }
}

// Re-export commonly used items
pub use aal::Amd64Arch;
pub use ffi::*;
pub use arch::*;
pub use asm::*;
pub use cache::*;
pub use debugger::*;
pub use faults::*;
pub use interrupts::*;
pub use ops::*;
pub use smp::*;
pub use timer::*;
pub use uspace_entry::*;

// Types from iframe
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86Iframe {
    // General registers
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rsp: u64,

    // Segment registers
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,

    // Special registers
    pub rflags: u64,
    pub rip: u64,
    pub user_ss: u64,
    pub user_cs: u64,
}

// Thread context for AMD64
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86ThreadStateGeneralRegs {
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rsp: u64,
    pub rip: u64,
    pub rflags: u64,
}

// Early initialization
pub fn arch_early_init() {
    unsafe {
        // Initialize MMU early
        ffi::sys_x86_mmu_early_init();

        // Initialize extended registers (SSE/AVX)
        ffi::sys_x86_extended_register_init();

        // Initialize CPU features
        ffi::sys_x86_feature_init();
    }
}

// Main initialization
pub fn arch_init() {
    unsafe {
        // Initialize per-CPU data
        ffi::sys_x86_init_percpu(0);

        // Initialize main MMU
        ffi::sys_x86_mmu_init();

        // Initialize per-CPU MMU
        ffi::sys_x86_mmu_percpu_init();

        // Initialize PAT
        ffi::sys_x86_mmu_mem_type_init();

        // Initialize processor trace (optional)
        // ffi::sys_x86_processor_trace_init();
    }
}
