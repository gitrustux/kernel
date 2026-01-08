// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Common type aliases used throughout the kernel

#![no_std]

/// Virtual address type
pub type VAddr = usize;

/// Physical address type
pub type PAddr = u64;

/// Size type
pub type Size = usize;

/// Signed size type
pub type SSize = isize;

/// Offset type
pub type Offset = isize;

/// Error code type (negative values indicate errors)
pub type Status = i32;

/// Handle type
pub type Handle = u32;

/// Thread ID type
pub type Tid = u64;

/// Process ID type
pub type Pid = u64;

/// CPU ID type
pub type CpuId = u32;

/// IRQ number type
pub type Irq = u32;

/// Vector number type
pub type Vector = u32;

/// Unsigned pointer-sized integer
pub type UIntPtr = usize;

/// Signed pointer-sized integer
pub type SIntPtr = isize;

/// Time value in nanoseconds
pub type Nanoseconds = u64;

/// Time value in microseconds
pub type Microseconds = u64;

/// Time value in milliseconds
pub type Milliseconds = u64;

/// Result type for kernel operations
pub type Result<T = ()> = core::result::Result<T, Status>;

/// Common status codes
pub mod status {
    use super::Status;

    pub const OK: Status = 0;
    pub const ERR: Status = -1;
    pub const ERR_INVALID_ARGS: Status = -2;
    pub const ERR_BAD_HANDLE: Status = -3;
    pub const ERR_BAD_STATE: Status = -4;
    pub const ERR_NOT_SUPPORTED: Status = -5;
    pub const ERR_NO_MEMORY: Status = -6;
    pub const ERR_TIMED_OUT: Status = -7;
    pub const ERR_NOT_FOUND: Status = -8;
    pub const ERR_ALREADY_EXISTS: Status = -9;
    pub const ERR_ACCESS_DENIED: Status = -10;
    pub const ERR_IO: Status = -11;
    pub const ERR_INTERNAL: Status = -12;

    /// Additional error codes (Rustux/Zircon specific)
    pub const ERR_NEXT: Status = -13;
    pub const ERR_STOP: Status = -14;
    pub const ERR_NO_RESOURCES: Status = -15;
    pub const ERR_NOT_ENOUGH_BUFFER: Status = -16;
    pub const ERR_OUT_OF_RANGE: Status = -17;

    /// Legacy ZX error codes
    pub const ZX_ERR_BAD_STATE: Status = -20;
    pub const ZX_ERR_NOT_SUPPORTED: Status = -23;
    pub const ZX_ERR_NO_MEMORY: Status = -25;
    pub const ZX_ERR_TIMED_OUT: Status = -29;
    pub const ZX_ERR_ACCESS_DENIED: Status = -30;
    pub const ZX_ERR_IO: Status = -40;
    pub const ZX_ERR_INTERNAL: Status = -50;
}

/// Rustux error type (alias for Status)
pub type RxError = Status;

/// Rustux status type (alias for Status)
pub type RxStatus = Status;

/// Legacy status type (alias for Status)
pub type rx_status_t = Status;

/// Trait for status codes that can be checked for success
pub trait StatusTrait {
    /// Check if the status code indicates success
    fn is_ok(&self) -> bool;

    /// Check if the status code indicates success
    fn is_error(&self) -> bool {
        !self.is_ok()
    }
}

/// Implement StatusTrait for i32 (Status type is i32)
impl StatusTrait for i32 {
    fn is_ok(&self) -> bool {
        *self == 0
    }
}


/// Common error values
pub mod err {
    use super::Status;

    pub const RX_OK: Status = super::status::OK;
    pub const RX_ERR_OK: Status = super::status::OK;
    pub const RX_ERR_ACCESS_DENIED: Status = super::status::ERR_ACCESS_DENIED;
    pub const RX_ERR_INVALID_ARGS: Status = super::status::ERR_INVALID_ARGS;
    pub const RX_ERR_NO_RESOURCES: Status = super::status::ERR_NO_RESOURCES;
    pub const RX_ERR_NOT_FOUND: Status = super::status::ERR_NOT_FOUND;
    pub const RX_ERR_IO: Status = super::status::ERR_IO;
    pub const RX_ERR_INTERNAL: Status = super::status::ERR_INTERNAL;
    pub const RX_ERR_BAD_STATE: Status = super::status::ERR_BAD_STATE;
    pub const RX_ERR_NOT_SUPPORTED: Status = super::status::ERR_NOT_SUPPORTED;
    pub const RX_ERR_ALREADY_EXISTS: Status = super::status::ERR_ALREADY_EXISTS;
    pub const RX_ERR_BAD_HANDLE: Status = super::status::ERR_BAD_HANDLE;
    pub const RX_ERR_NO_MEMORY: Status = super::status::ERR_NO_MEMORY;
    pub const RX_ERR_TIMED_OUT: Status = super::status::ERR_TIMED_OUT;
    pub const RX_ERR_OUT_OF_RANGE: Status = -17;
    pub const RX_ERR_BUFFER_TOO_SMALL: Status = super::status::ERR_NOT_ENOUGH_BUFFER;
    pub const RX_ERR_SHOULD_WAIT: Status = -18;
    pub const RX_ERR_WRONG_TYPE: Status = -19;
    pub const RX_ERR_PEER_CLOSED: Status = -20;
    pub const RX_ERR_CANCELED: Status = -21;
    pub const RX_ERR_BAD_SYSCALL: Status = -22;
    pub const RX_ERR_STOP: Status = super::status::ERR_STOP;
    pub const RX_ERR_NEXT: Status = super::status::ERR_NEXT;

    /// ============================================================================
    /// Exception types
    /// ============================================================================

    /// Exception type codes (from zircon/hypervisor/public/hypervisor.h)
    pub const ZX_EXCP_FATAL_PAGE_FAULT: u32 = 0x202;
    pub const ZX_EXCP_GENERAL: u32 = 0x500;
    pub const ZX_EXCP_SW_BREAKPOINT: u32 = 0x800;
    pub const ZX_EXCP_HW_BREAKPOINT: u32 = 0x801;
    pub const ZX_EXCP_UNALIGNED_ACCESS: u32 = 0x802;
    pub const ZX_EXCP_UNDEFINED_INSTRUCTION: u32 = 0x803;
    pub const ZX_EXCP_POLICY_ERROR: u32 = 0x804;

    /// Legacy exception type aliases
    pub const RX_EXCP_FATAL_PAGE_FAULT: u32 = ZX_EXCP_FATAL_PAGE_FAULT;
    pub const RX_EXCP_GENERAL: u32 = ZX_EXCP_GENERAL;
    pub const RX_EXCP_SW_BREAKPOINT: u32 = ZX_EXCP_SW_BREAKPOINT;
    pub const RX_EXCP_HW_BREAKPOINT: u32 = ZX_EXCP_HW_BREAKPOINT;
    pub const RX_EXCP_UNALIGNED_ACCESS: u32 = ZX_EXCP_UNALIGNED_ACCESS;
    pub const RX_EXCP_UNDEFINED_INSTRUCTION: u32 = ZX_EXCP_UNDEFINED_INSTRUCTION;
    pub const RX_EXCP_POLICY_ERROR: u32 = ZX_EXCP_POLICY_ERROR;
}

/// ============================================================================
/// Legacy Type Aliases (for compatibility with C++ kernel code)
/// ============================================================================

/// Timer handle type (legacy)
pub type timer_t = u64;

/// Thread handle type (legacy)
pub type thread_t = u64;

/// CPU mask type (legacy)
pub type cpu_mask_t = u64;

/// Port packet type (legacy)
pub type rx_port_packet_t = u64;

/// Port packet type (without _t suffix)
pub type rx_port_packet = rx_port_packet_t;

/// Exception report type (stub)
#[repr(C)]
pub struct rx_exception_report_t {
    pub data: [u64; 8],
}

/// Interrupt count for x86 (stub)
pub type X86_INT_COUNT = u32;

/// LVT masked value (stub)
pub const LVT_MASKED: u32 = 0x10000;
