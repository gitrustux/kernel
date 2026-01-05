// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! System Call Interface
//!
//! This module provides the unified system call ABI for the Rustux kernel.
//! The syscall ABI is stable across all architectures (ARM64, AMD64, RISC-V).
//!
//! # Design Rules
//!
//! - **Stability**: Syscall numbers & semantics frozen across architectures
//! - **Object-based**: All operations on handles with rights
//! - **Deterministic**: Same inputs → same outputs → same errors
//! - **No arch leakage**: CPU differences hidden below ABI
//!
//! # Calling Convention
//!
//! | Architecture | Syscall Instruction | Arg Registers | Return |
//! |--------------|---------------------|---------------|--------|
//! | ARM64 | `svc #0` | x0-x6 | x0 |
//! | AMD64 | `syscall` | rdi, rsi, rdx, r10, r8, r9 | rax |
//! | RISC-V | `ecall` | a0-a6 | a0 |
//!
//! # Error Return Convention
//!
//! ```text
//! Success: return value in r0/rax/a0 (positive or zero)
//! Failure: return negative error code
//! ```

#![no_std]

use crate::rustux::types::*;
use crate::rustux::types::err::*;

// Import logging macros
use crate::{log_debug, log_error, log_info, log_trace};

// Syscall implementations
pub mod vmo;
pub mod channel;
pub mod event;
pub mod timer;
pub mod futex;
pub mod task;
pub mod vmar;
pub mod system;
pub mod system_arm64;
pub mod system_riscv64;
pub mod system_x86;
pub mod object;
pub mod object_wait;
pub mod handle_ops;
pub mod wrapper;
pub mod port;
pub mod fifo;
pub mod socket;
pub mod hypervisor;
pub mod pager;
pub mod resource;
pub mod test;
pub mod profile;
pub mod debug;
pub mod exceptions;
pub mod rustux;
pub mod ddk_x86;
pub mod ddk_arm64;
pub mod ddk;
pub mod ddk_pci;

/// ============================================================================
/// Syscall Numbers (Stable v1)
/// ============================================================================

/// System call numbers
///
/// These numbers are frozen as part of the stable ABI v1.
/// DO NOT change existing numbers - only append new syscalls.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SyscallNumber {
    // Process & Thread (0x001-0x00F)

    /// Create new process under job
    rx_process_create = 0x01,

    /// Begin process execution
    rx_process_start = 0x02,

    /// Create thread in process
    rx_thread_create = 0x03,

    /// Begin thread execution
    rx_thread_start = 0x04,

    /// Terminate thread
    rx_thread_exit = 0x05,

    /// Terminate process
    rx_process_exit = 0x06,

    /// Close handle
    rx_handle_close = 0x07,

    // Memory / VMO (0x010-0x01F)

    /// Create virtual memory object
    rx_vmo_create = 0x10,

    /// Read from VMO
    rx_vmo_read = 0x11,

    /// Write to VMO
    rx_vmo_write = 0x12,

    /// COW clone VMO
    rx_vmo_clone = 0x13,

    /// Map VMO into address space
    rx_vmar_map = 0x14,

    /// Unmap region
    rx_vmar_unmap = 0x15,

    /// Change protection
    rx_vmar_protect = 0x16,

    // IPC & Sync (0x020-0x02F)

    /// Create message channel
    rx_channel_create = 0x20,

    /// Write message + handles
    rx_channel_write = 0x21,

    /// Read message + handles
    rx_channel_read = 0x22,

    /// Create event object
    rx_event_create = 0x23,

    /// Create event pair
    rx_eventpair_create = 0x24,

    /// Signal object
    rx_object_signal = 0x25,

    /// Wait on single object
    rx_object_wait_one = 0x26,

    /// Wait on multiple objects
    rx_object_wait_many = 0x27,

    // Jobs & Handles (0x030-0x03F)

    /// Create job under parent
    rx_job_create = 0x30,

    /// Duplicate handle with rights
    rx_handle_duplicate = 0x31,

    /// Transfer handle to process
    rx_handle_transfer = 0x32,

    // Time (0x040-0x04F)

    /// Get monotonic/realtime
    rx_clock_get = 0x40,

    /// Create timer
    rx_timer_create = 0x41,

    /// Arm timer
    rx_timer_set = 0x42,

    /// Cancel timer
    rx_timer_cancel = 0x43,

    /// Unknown/invalid syscall number
    Unknown = 0xFFFF,
}

impl SyscallNumber {
    /// Convert from raw number
    pub const fn from_raw(n: u32) -> Self {
        // This is a simplified conversion
        // In a real implementation, we'd have a match statement
        // but const fn limits us
        if n <= 0x43 {
            // Valid syscall number (simplified)
            Self::from_raw_unchecked(n)
        } else {
            Self::Unknown
        }
    }

    const fn from_raw_unchecked(n: u32) -> Self {
        unsafe { core::mem::transmute(n) }
    }

    /// Get the syscall name
    pub const fn name(&self) -> &'static str {
        match self {
            Self::rx_process_create => "rx_process_create",
            Self::rx_process_start => "rx_process_start",
            Self::rx_thread_create => "rx_thread_create",
            Self::rx_thread_start => "rx_thread_start",
            Self::rx_thread_exit => "rx_thread_exit",
            Self::rx_process_exit => "rx_process_exit",
            Self::rx_handle_close => "rx_handle_close",
            Self::rx_vmo_create => "rx_vmo_create",
            Self::rx_vmo_read => "rx_vmo_read",
            Self::rx_vmo_write => "rx_vmo_write",
            Self::rx_vmo_clone => "rx_vmo_clone",
            Self::rx_vmar_map => "rx_vmar_map",
            Self::rx_vmar_unmap => "rx_vmar_unmap",
            Self::rx_vmar_protect => "rx_vmar_protect",
            Self::rx_channel_create => "rx_channel_create",
            Self::rx_channel_write => "rx_channel_write",
            Self::rx_channel_read => "rx_channel_read",
            Self::rx_event_create => "rx_event_create",
            Self::rx_eventpair_create => "rx_eventpair_create",
            Self::rx_object_signal => "rx_object_signal",
            Self::rx_object_wait_one => "rx_object_wait_one",
            Self::rx_object_wait_many => "rx_object_wait_many",
            Self::rx_job_create => "rx_job_create",
            Self::rx_handle_duplicate => "rx_handle_duplicate",
            Self::rx_handle_transfer => "rx_handle_transfer",
            Self::rx_clock_get => "rx_clock_get",
            Self::rx_timer_create => "rx_timer_create",
            Self::rx_timer_set => "rx_timer_set",
            Self::rx_timer_cancel => "rx_timer_cancel",
            Self::Unknown => "unknown",
        }
    }
}

/// ============================================================================
/// Syscall Arguments
/// ============================================================================

/// System call arguments
///
/// This structure holds the arguments passed to a system call.
/// The layout is designed to match the calling conventions:
/// - ARM64: x0-x5 → args[0-5], syscall number in x8
/// - AMD64: rdi,rsi,rdx,r10,r8,r9 → args[0-5], syscall number in rax
/// - RISC-V: a0-a5 → args[0-5], syscall number in a7
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallArgs {
    /// Syscall number
    pub number: u32,

    /// Arguments (up to 6)
    pub args: [usize; 6],
}

impl SyscallArgs {
    /// Create new syscall arguments
    pub const fn new(number: u32, args: [usize; 6]) -> Self {
        Self { number, args }
    }

    /// Get argument at index
    pub const fn arg(&self, index: usize) -> usize {
        if index < 6 {
            self.args[index]
        } else {
            0
        }
    }
}

/// ============================================================================
/// Syscall Return Values
/// ============================================================================

/// System call return value
///
/// Success: positive or zero value
/// Failure: negative error code
pub type SyscallRet = isize;

/// Convert error code to negative return value
#[inline]
pub const fn err_to_ret(err: Status) -> SyscallRet {
    -(err as SyscallRet)
}

/// Convert success value to return value
#[inline]
pub const fn ok_to_ret(val: usize) -> SyscallRet {
    val as SyscallRet
}

/// ============================================================================
/// Syscall Dispatcher
/// ============================================================================

/// System call dispatcher
///
/// This function is called from the architecture-specific syscall entry point.
/// It validates the syscall number and dispatches to the appropriate handler.
///
/// # Arguments
///
/// * `args` - System call arguments
///
/// # Returns
///
/// System call return value (positive/zero for success, negative for error)
///
/// # Calling Convention
///
/// This function uses the C ABI and is callable from assembly.
/// It must be marked `no_mangle` so assembly code can find it.
#[no_mangle]
pub extern "C" fn syscall_dispatch(args: SyscallArgs) -> SyscallRet {
    let num = SyscallNumber::from_raw(args.number);

    log_trace!(
        "syscall: num={} ({}) args=[{:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}]",
        args.number,
        num.name(),
        args.args[0],
        args.args[1],
        args.args[2],
        args.args[3],
        args.args[4],
        args.args[5]
    );

    // Dispatch to handler
    match num {
        // Process & Thread
        SyscallNumber::rx_process_create => sys_process_create(args),
        SyscallNumber::rx_process_start => sys_process_start(args),
        SyscallNumber::rx_thread_create => sys_thread_create(args),
        SyscallNumber::rx_thread_start => sys_thread_start(args),
        SyscallNumber::rx_thread_exit => sys_thread_exit(args),
        SyscallNumber::rx_process_exit => sys_process_exit(args),
        SyscallNumber::rx_handle_close => sys_handle_close(args),

        // Memory / VMO
        SyscallNumber::rx_vmo_create => sys_vmo_create(args),
        SyscallNumber::rx_vmo_read => sys_vmo_read(args),
        SyscallNumber::rx_vmo_write => sys_vmo_write(args),
        SyscallNumber::rx_vmo_clone => sys_vmo_clone(args),
        SyscallNumber::rx_vmar_map => sys_vmar_map(args),
        SyscallNumber::rx_vmar_unmap => sys_vmar_unmap(args),
        SyscallNumber::rx_vmar_protect => sys_vmar_protect(args),

        // IPC & Sync
        SyscallNumber::rx_channel_create => sys_channel_create(args),
        SyscallNumber::rx_channel_write => sys_channel_write(args),
        SyscallNumber::rx_channel_read => sys_channel_read(args),
        SyscallNumber::rx_event_create => sys_event_create(args),
        SyscallNumber::rx_eventpair_create => sys_eventpair_create(args),
        SyscallNumber::rx_object_signal => sys_object_signal(args),
        SyscallNumber::rx_object_wait_one => sys_object_wait_one(args),
        SyscallNumber::rx_object_wait_many => sys_object_wait_many(args),

        // Jobs & Handles
        SyscallNumber::rx_job_create => sys_job_create(args),
        SyscallNumber::rx_handle_duplicate => sys_handle_duplicate(args),
        SyscallNumber::rx_handle_transfer => sys_handle_transfer(args),

        // Time
        SyscallNumber::rx_clock_get => sys_clock_get(args),
        SyscallNumber::rx_timer_create => sys_timer_create(args),
        SyscallNumber::rx_timer_set => sys_timer_set(args),
        SyscallNumber::rx_timer_cancel => sys_timer_cancel(args),

        SyscallNumber::Unknown => {
            log_error!("Unknown syscall: {}", args.number);
            err_to_ret(RX_ERR_NOT_SUPPORTED)
        }
    }
}

/// ============================================================================
/// Syscall Handler Implementations
/// ============================================================================

/// Stub for syscall handlers not yet implemented
macro_rules! syscall_stub {
    ($name:ident) => {
        fn $name(args: SyscallArgs) -> SyscallRet {
            log_debug!("syscall: {} (stub)", stringify!($name));
            err_to_ret(RX_ERR_NOT_SUPPORTED)
        }
    };
}

// Process & Thread syscalls
fn sys_process_create(args: SyscallArgs) -> SyscallRet {
    let job = args.arg(0) as u32;
    let name = args.arg(1);
    let name_len = args.arg(2);
    let options = args.arg(3) as u32;
    task::sys_process_create_impl(job, name, name_len, options)
}

fn sys_process_start(args: SyscallArgs) -> SyscallRet {
    let process = args.arg(0) as u32;
    let thread = args.arg(1) as u32;
    let entry = args.arg(2) as u64;
    let stack = args.arg(3) as u64;
    task::sys_process_start_impl(process, thread, entry, stack)
}

fn sys_thread_create(args: SyscallArgs) -> SyscallRet {
    let process = args.arg(0) as u32;
    let name = args.arg(1);
    let name_len = args.arg(2);
    let options = args.arg(3) as u32;
    task::sys_thread_create_impl(process, name, name_len, options)
}

fn sys_thread_start(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let entry = args.arg(1) as u64;
    let stack = args.arg(2) as u64;
    let arg1 = args.arg(3) as u64;
    let arg2 = args.arg(4) as u64;
    task::sys_thread_start_impl(handle, entry, stack, arg1, arg2)
}

fn sys_thread_exit(args: SyscallArgs) -> SyscallRet {
    let code = args.arg(0) as i64;
    task::sys_thread_exit_impl(code)
}

fn sys_process_exit(args: SyscallArgs) -> SyscallRet {
    let code = args.arg(0) as i64;
    task::sys_process_exit_impl(code)
}

fn sys_handle_close(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    // TODO: Implement handle close
    log_debug!("sys_handle_close: handle={}", handle);
    ok_to_ret(0)
}

// Memory / VMO syscalls
fn sys_vmo_create(args: SyscallArgs) -> SyscallRet {
    let size = args.arg(0);
    let options = args.arg(1) as u32;
    vmo::sys_vmo_create_impl(size, options)
}

fn sys_vmo_read(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let user_ptr = args.arg(1);
    let offset = args.arg(2);
    let len = args.arg(3);
    vmo::sys_vmo_read_impl(handle, user_ptr, offset, len)
}

fn sys_vmo_write(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let user_ptr = args.arg(1);
    let offset = args.arg(2);
    let len = args.arg(3);
    vmo::sys_vmo_write_impl(handle, user_ptr, offset, len)
}

fn sys_vmo_clone(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let offset = args.arg(1);
    let size = args.arg(2);
    vmo::sys_vmo_clone_impl(handle, offset, size)
}
fn sys_vmar_map(args: SyscallArgs) -> SyscallRet {
    let vmar = args.arg(0) as u32;
    let options = args.arg(1) as u32;
    let vmar_offset = args.arg(2) as u64;
    let vmo = args.arg(3) as u32;
    let vmo_offset = args.arg(4) as u64;
    let len = args.arg(5) as u64;
    let mapped_addr = args.arg(6);
    vmar::sys_vmar_map_impl(vmar, options, vmar_offset, vmo, vmo_offset, len, mapped_addr)
}

fn sys_vmar_unmap(args: SyscallArgs) -> SyscallRet {
    let vmar = args.arg(0) as u32;
    let addr = args.arg(1) as u64;
    let len = args.arg(2) as u64;
    vmar::sys_vmar_unmap_impl(vmar, addr, len)
}

fn sys_vmar_protect(args: SyscallArgs) -> SyscallRet {
    let vmar = args.arg(0) as u32;
    let options = args.arg(1) as u32;
    let addr = args.arg(2) as u64;
    let len = args.arg(3) as u64;
    vmar::sys_vmar_protect_impl(vmar, options, addr, len)
}

// IPC & Sync syscalls
fn sys_channel_create(args: SyscallArgs) -> SyscallRet {
    let options = args.arg(0) as u32;
    channel::sys_channel_create_impl(options)
}

fn sys_channel_write(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let options = args.arg(1) as u32;
    let user_data = args.arg(2);
    let data_size = args.arg(3);
    let user_handles = args.arg(4);
    let handle_count = args.arg(5);
    channel::sys_channel_write_impl(handle, options, user_data, data_size, user_handles, handle_count)
}

fn sys_channel_read(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let options = args.arg(1) as u32;
    let user_data = args.arg(2);
    let data_capacity = args.arg(3);
    let user_handles = args.arg(4);
    let handles_capacity = args.arg(5);
    channel::sys_channel_read_impl(handle, options, user_data, data_capacity, user_handles, handles_capacity)
}
fn sys_event_create(args: SyscallArgs) -> SyscallRet {
    let options = args.arg(0) as u32;
    event::sys_event_create_impl(options)
}

fn sys_eventpair_create(args: SyscallArgs) -> SyscallRet {
    event::sys_eventpair_create_impl()
}

fn sys_object_signal(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let options = args.arg(1) as u32;
    event::sys_object_signal_impl(handle, options)
}

fn sys_object_wait_one(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let signals = args.arg(1) as u64;
    let deadline = args.arg(2) as u64;
    let observed_out = args.arg(3);
    object_wait::sys_object_wait_one_impl(handle, signals, deadline, observed_out)
}

fn sys_object_wait_many(args: SyscallArgs) -> SyscallRet {
    let user_items = args.arg(0);
    let count = args.arg(1);
    let deadline = args.arg(2) as u64;
    object_wait::sys_object_wait_many_impl(user_items, count, deadline)
}

// Jobs & Handles syscalls
fn sys_job_create(args: SyscallArgs) -> SyscallRet {
    let parent = args.arg(0) as u32;
    let options = args.arg(1) as u32;
    task::sys_job_create_impl(parent, options)
}

fn sys_handle_duplicate(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let rights = args.arg(1) as u32;
    let out = args.arg(2);
    handle_ops::sys_handle_duplicate_impl(handle, rights, out)
}

fn sys_handle_transfer(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let rights = args.arg(1) as u32;
    let options = args.arg(2) as u32;
    handle_ops::sys_handle_transfer_impl(handle, rights, options)
}

// Time syscalls
fn sys_clock_get(args: SyscallArgs) -> SyscallRet {
    let clock_id = args.arg(0) as u32;
    // Return current time in nanoseconds
    // TODO: Implement proper clock
    if clock_id == 0 {
        // CLOCK_MONOTONIC
        let time = 0; // Placeholder
        ok_to_ret(time as usize)
    } else {
        err_to_ret(RX_ERR_INVALID_ARGS)
    }
}

fn sys_timer_create(args: SyscallArgs) -> SyscallRet {
    let options = args.arg(0) as u32;
    let clock_id = args.arg(1) as u32;
    timer::sys_timer_create_impl(options, clock_id)
}

fn sys_timer_set(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    let deadline = args.arg(1) as u64;
    let slack = args.arg(2) as i64;
    timer::sys_timer_set_impl(handle, deadline, slack)
}

fn sys_timer_cancel(args: SyscallArgs) -> SyscallRet {
    let handle = args.arg(0) as u32;
    timer::sys_timer_cancel_impl(handle)
}

/// ============================================================================
/// Architecture-Specific Entry Points
/// ============================================================================

/// ARM64 syscall entry
///
/// Called from exceptions.S with `svc #0` instruction.
/// Arguments in x0-x5, syscall number in x8.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn arm64_syscall_entry(
    x0: usize,
    x1: usize,
    x2: usize,
    x3: usize,
    x4: usize,
    x5: usize,
    x8: u32,
) -> SyscallRet {
    let args = SyscallArgs::new(x8, [x0, x1, x2, x3, x4, x5]);
    syscall_dispatch(args)
}

/// AMD64 syscall entry
///
/// Called from entry.S with `syscall` instruction.
/// Arguments in rdi,rsi,rdx,r10,r8,r9, syscall number in rax.
#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn amd64_syscall_entry(
    rdi: usize,
    rsi: usize,
    rdx: usize,
    r10: usize,
    r8: usize,
    r9: usize,
    rax: u32,
) -> SyscallRet {
    let args = SyscallArgs::new(rax, [rdi, rsi, rdx, r10, r8, r9]);
    syscall_dispatch(args)
}

/// RISC-V syscall entry
///
/// Called from exceptions.S with `ecall` instruction.
/// Arguments in a0-a5, syscall number in a7.
#[cfg(target_arch = "riscv64")]
#[no_mangle]
pub extern "C" fn riscv_syscall_entry(
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a7: u32,
) -> SyscallRet {
    let args = SyscallArgs::new(a7, [a0, a1, a2, a3, a4, a5]);
    syscall_dispatch(args)
}

/// ============================================================================
/// Syscall Statistics
/// ============================================================================

/// Syscall statistics
#[repr(C)]
#[derive(Debug)]
pub struct SyscallStats {
    /// Total syscalls dispatched
    pub total_calls: u64,

    /// Syscalls by number
    pub by_number: [u64; 256],
}

/// Global syscall statistics
static mut SYSCALL_STATS: SyscallStats = SyscallStats {
    total_calls: 0,
    by_number: [0; 256],
};

/// Record a syscall invocation
fn record_syscall(num: u32) {
    unsafe {
        SYSCALL_STATS.total_calls += 1;
        if (num as usize) < 256 {
            SYSCALL_STATS.by_number[num as usize] += 1;
        }
    }
}

/// Get syscall statistics
pub fn get_syscall_stats() -> SyscallStats {
    unsafe { SYSCALL_STATS }
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the syscall subsystem
pub fn init() {
    log_info!("Syscall subsystem initialized");
    log_info!("  ABI version: 1 (stable)");
    log_info!("  Syscalls defined: {}", 0x43); // Last syscall number
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_number_conversion() {
        let num = SyscallNumber::from_raw(0x01);
        assert_eq!(num as u32, 0x01);
        assert_eq!(num.name(), "rx_process_create");

        let unknown = SyscallNumber::from_raw(0xFFFF);
        assert_eq!(unknown, SyscallNumber::Unknown);
    }

    #[test]
    fn test_syscall_args() {
        let args = SyscallArgs::new(0x10, [1, 2, 3, 4, 5, 6]);
        assert_eq!(args.number, 0x10);
        assert_eq!(args.arg(0), 1);
        assert_eq!(args.arg(5), 6);
        assert_eq!(args.arg(10), 0); // Out of range
    }

    #[test]
    fn test_ret_conversions() {
        assert_eq!(ok_to_ret(42), 42);
        assert_eq!(err_to_ret(RX_ERR_NO_MEMORY), -(RX_ERR_NO_MEMORY as SyscallRet));
    }
}
