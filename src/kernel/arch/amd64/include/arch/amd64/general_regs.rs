// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 general register state definitions
//!
//! This module defines types and constants for representing the x86 general
//! register state during system calls, interrupts, and exceptions.

/// Tag value for no register state
pub const X86_GENERAL_REGS_NONE: u8 = 0;
/// Tag value for syscall register state
pub const X86_GENERAL_REGS_SYSCALL: u8 = 1;
/// Tag value for iframe register state
pub const X86_GENERAL_REGS_IFRAME: u8 = 2;

/// Structure used to hold the general purpose integer registers
/// when a syscall is suspended
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86SyscallGeneralRegs {
    /// RAX register
    pub rax: u64,
    /// RBX register
    pub rbx: u64,
    /// RCX register
    pub rcx: u64,
    /// RDX register
    pub rdx: u64,
    /// RSI register
    pub rsi: u64,
    /// RDI register
    pub rdi: u64,
    /// RBP register
    pub rbp: u64,
    /// RSP register
    pub rsp: u64,
    /// R8 register
    pub r8: u64,
    /// R9 register
    pub r9: u64,
    /// R10 register
    pub r10: u64,
    /// R11 register
    pub r11: u64,
    /// R12 register
    pub r12: u64,
    /// R13 register
    pub r13: u64,
    /// R14 register
    pub r14: u64,
    /// R15 register
    pub r15: u64,
    /// RIP register (instruction pointer)
    pub rip: u64,
    /// RFLAGS register
    pub rflags: u64,
}

impl X86SyscallGeneralRegs {
    /// Create a new register state with all registers set to zero
    #[inline]
    pub const fn new() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rsp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0,
            rflags: 0,
        }
    }

    /// Set the return value in the appropriate register (RAX)
    #[inline]
    pub fn set_return_value(&mut self, value: u64) {
        self.rax = value;
    }

    /// Get the return value from the appropriate register (RAX)
    #[inline]
    pub fn return_value(&self) -> u64 {
        self.rax
    }

    /// Get the syscall number from the appropriate register (RAX)
    #[inline]
    pub fn syscall_num(&self) -> u64 {
        self.rax
    }

    /// Get the first syscall argument (RDI)
    #[inline]
    pub fn arg1(&self) -> u64 {
        self.rdi
    }

    /// Get the second syscall argument (RSI)
    #[inline]
    pub fn arg2(&self) -> u64 {
        self.rsi
    }

    /// Get the third syscall argument (RDX)
    #[inline]
    pub fn arg3(&self) -> u64 {
        self.rdx
    }

    /// Get the fourth syscall argument (R10)
    #[inline]
    pub fn arg4(&self) -> u64 {
        self.r10
    }

    /// Get the fifth syscall argument (R8)
    #[inline]
    pub fn arg5(&self) -> u64 {
        self.r8
    }

    /// Get the sixth syscall argument (R9)
    #[inline]
    pub fn arg6(&self) -> u64 {
        self.r9
    }
}

// Note: x86_iframe_t is defined elsewhere in the kernel
// This struct would typically define the register state for interrupts/exceptions