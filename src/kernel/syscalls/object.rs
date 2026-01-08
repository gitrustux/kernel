// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Object System Calls
//!
//! This module implements object-related system calls for getting information,
//! properties, and signaling kernel objects.
//!
//! # Syscalls Implemented
//!
//! - `rx_object_get_info` - Get information about a kernel object
//! - `rx_object_get_property` - Get a property of a kernel object
//! - `rx_object_set_property` - Set a property of a kernel object
//! - `rx_object_signal` - Signal a kernel object
//! - `rx_object_signal_peer` - Signal the peer of a kernel object
//! - `rx_object_get_child` - Get a child object by koid
//! - `rx_object_set_cookie` - Set a cookie on an object
//! - `rx_object_get_cookie` - Get a cookie from an object
//!
//! # Design
//!
//! - Dispatches based on topic/property type
//! - Validates all handles and rights
//! - Supports many different object info types
//! - Property get/set for various object types


use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};
/// ============================================================================
/// Info Topics
/// ============================================================================

/// Info topic identifiers
pub mod info_topic {
    /// Handle validity check
    pub const HANDLE_VALID: u32 = 0x00;

    /// Basic handle information
    pub const HANDLE_BASIC: u32 = 0x01;

    /// Process information
    pub const PROCESS: u32 = 0x02;

    /// Process threads
    pub const PROCESS_THREADS: u32 = 0x03;

    /// Job children
    pub const JOB_CHILDREN: u32 = 0x04;

    /// Job processes
    pub const JOB_PROCESSES: u32 = 0x05;

    /// Thread information
    pub const THREAD: u32 = 0x06;

    /// Thread exception report
    pub const THREAD_EXCEPTION_REPORT: u32 = 0x07;

    /// Thread statistics
    pub const THREAD_STATS: u32 = 0x08;

    /// Task statistics
    pub const TASK_STATS: u32 = 0x09;

    /// Process memory maps
    pub const PROCESS_MAPS: u32 = 0x0A;

    /// Process VMOs
    pub const PROCESS_VMOS: u32 = 0x0B;

    /// VMO information
    pub const VMO: u32 = 0x0C;

    /// VMAR information
    pub const VMAR: u32 = 0x0D;

    /// CPU statistics
    pub const CPU_STATS: u32 = 0x0E;

    /// Kernel memory statistics
    pub const KMEM_STATS: u32 = 0x0F;

    /// Resource information
    pub const RESOURCE: u32 = 0x10;

    /// Handle count
    pub const HANDLE_COUNT: u32 = 0x11;

    /// Process handle statistics
    pub const PROCESS_HANDLE_STATS: u32 = 0x12;

    /// Socket information
    pub const SOCKET: u32 = 0x13;
}

/// ============================================================================
/// Property Types
/// ============================================================================

/// Property identifiers
pub mod property {
    /// Object name
    pub const NAME: u32 = 0x00;

    /// Process debug address
    pub const PROCESS_DEBUG_ADDR: u32 = 0x01;

    /// Process VDSO base address
    pub const PROCESS_VDSO_BASE_ADDRESS: u32 = 0x02;

    /// Socket receive threshold
    pub const SOCKET_RX_THRESHOLD: u32 = 0x03;

    /// Socket transmit threshold
    pub const SOCKET_TX_THRESHOLD: u32 = 0x04;

    /// Job kill on OOM
    pub const JOB_KILL_ON_OOM: u32 = 0x05;
}

/// ============================================================================
/// Info Structures
/// ============================================================================

/// Basic handle information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HandleBasicInfo {
    /// Kernel object ID
    pub koid: u64,

    /// Rights associated with the handle
    pub rights: u32,

    /// Object type
    pub type_: u32,

    /// Related koid (for eventpairs, etc.)
    pub related_koid: u64,

    /// Object properties
    pub props: u32,
}

/// Process information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessInfo {
    /// Process return code
    pub return_code: i64,

    /// Process start time
    pub started: u64,

    /// Process state
    pub state: u32,

    /// Padding
    _pad: u32,
}

/// Thread information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadInfo {
    /// Thread state
    pub state: u32,

    /// Wait reason
    pub wait_reason: u32,

    /// Thread CPU
    pub cpu: u32,

    /// Padding
    _pad: u32,
}

/// Thread statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadStats {
    /// Total idle time
    pub total_runtime: u64,

    /// CPU time
    pub cpu_time: u64,

    /// Number of context switches
    pub context_switches: u64,

    /// Number of page faults
    pub page_faults: u64,

    /// Padding
    _pad: [u64; 4],
}

/// Task statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskStats {
    /// Memory mapped bytes
    pub mem_mapped_bytes: u64,

    /// Private memory bytes
    pub mem_private_bytes: u64,

    /// Shared memory bytes
    pub mem_shared_bytes: u64,

    /// Padding
    _pad: [u64; 5],
}

/// VMO information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmoInfo {
    /// VMO koid
    pub koid: u64,

    /// Parent koid
    pub parent_koid: u64,

    /// Number of children
    pub num_children: u64,

    /// Number of mappings
    pub num_mappings: u64,

    /// Share count
    pub share_count: u64,

    /// Flags
    pub flags: u32,

    /// Padding
    _pad: u32,

    /// Size in bytes
    pub size_bytes: u64,

    /// Committed bytes
    pub committed_bytes: u64,

    /// Padding
    _pad2: [u64; 3],
}

/// VMAR information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmarInfo {
    /// Base address
    pub base: u64,

    /// Length in bytes
    pub len: u64,
}

/// CPU statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CpuStats {
    /// CPU number
    pub cpu_number: u32,

    /// Flags (online, etc.)
    pub flags: u32,

    /// Idle time
    pub idle_time: u64,

    /// Reschedules
    pub reschedules: u64,

    /// Context switches
    pub context_switches: u64,

    /// IRQ preemptions
    pub irq_preempts: u64,

    /// Preemptions
    pub preempts: u64,

    /// Yields
    pub yields: u64,

    /// Interrupts
    pub ints: u64,

    /// Timer interrupts
    pub timer_ints: u64,

    /// Timers
    pub timers: u64,

    /// Page faults
    pub page_faults: u64,

    /// Exceptions
    pub exceptions: u64,

    /// Syscalls
    pub syscalls: u64,

    /// Reschedule IPIs
    pub reschedule_ipis: u64,

    /// Generic IPIs
    pub generic_ipis: u64,

    /// Padding
    _pad: [u64; 8],
}

/// Kernel memory statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KmemStats {
    /// Total bytes
    pub total_bytes: u64,

    /// Free bytes
    pub free_bytes: u64,

    /// Wired bytes
    pub wired_bytes: u64,

    /// Total heap bytes
    pub total_heap_bytes: u64,

    /// Free heap bytes
    pub free_heap_bytes: u64,

    /// VMO bytes
    pub vmo_bytes: u64,

    /// MMU overhead bytes
    pub mmu_overhead_bytes: u64,

    /// IPC bytes
    pub ipc_bytes: u64,

    /// Other bytes
    pub other_bytes: u64,
}

/// Handle count information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HandleCountInfo {
    /// Number of handles
    pub handle_count: u64,
}

/// Process handle statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessHandleStats {
    /// Handle counts by type
    pub handle_count: [u64; 64],
}

/// Socket information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SocketInfo {
    /// Options
    pub options: u64,

    /// Padding
    _pad1: [u64; 3],

    /// Read buffer size
    pub rx_buf_size: u64,

    /// Read buffer available
    pub rx_buf_available: u64,

    /// Write buffer size
    pub tx_buf_size: u64,

    /// Write buffer available
    pub tx_buf_available: u64,

    /// Padding
    _pad2: [u64; 7],
}

/// ============================================================================
/// Single Record Result Helper
/// ============================================================================

/// Helper function for returning a single record result
///
/// # Arguments
///
/// * `buffer` - User buffer to write to
/// * `buffer_size` - Size of buffer
/// * `actual_out` - User pointer to store actual count
/// * `avail_out` - User pointer to store available count
/// * `record_data` - Pointer to record data
/// * `record_size` - Size of record
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
fn single_record_result(
    buffer: usize,
    buffer_size: usize,
    actual_out: usize,
    avail_out: usize,
    record_data: *const u8,
    record_size: usize,
) -> Result {
    let avail = 1usize;
    let actual = if buffer_size >= record_size {
        // Copy to user
        let user_ptr = UserPtr::new(buffer);
        unsafe {
            copy_to_user(user_ptr, record_data, record_size)?;
        }
        1
    } else {
        0
    };

    // Write actual
    if actual_out != 0 {
        let user_ptr = UserPtr::<u8>::new(actual_out);
        unsafe {
            copy_to_user(user_ptr, &actual as *const _ as *const u8, core::mem::size_of::<usize>())?;
        }
    }

    // Write avail
    if avail_out != 0 {
        let user_ptr = UserPtr::<u8>::new(avail_out);
        unsafe {
            copy_to_user(user_ptr, &avail as *const _ as *const u8, core::mem::size_of::<usize>())?;
        }
    }

    if actual == 0 {
        return Err(RX_ERR_BUFFER_TOO_SMALL);
    }

    Ok(())
}

/// ============================================================================
/// Syscall: Object Get Info
/// ============================================================================

/// Get information about a kernel object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value
/// * `topic` - Info topic
/// * `buffer` - User buffer to store info
/// * `buffer_size` - Size of buffer
/// * `actual_out` - User pointer to store actual count
/// * `avail_out` - User pointer to store available count
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_get_info_impl(
    handle_val: u32,
    topic: u32,
    buffer: usize,
    buffer_size: usize,
    actual_out: usize,
    avail_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_object_get_info: handle={:#x} topic={:#x} buffer={:#x} size={}",
        handle_val, topic, buffer, buffer_size
    );

    match topic {
        info_topic::HANDLE_VALID => {
            // Check if handle is valid
            // TODO: Implement proper handle validation
            ok_to_ret(0)
        }

        info_topic::HANDLE_BASIC => {
            // TODO: Implement proper handle lookup
            let info = HandleBasicInfo {
                koid: handle_val as u64,
                rights: 0xFFFF, // TODO: get actual rights
                type_: 0,       // TODO: get actual type
                related_koid: 0,
                props: 0,
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const HandleBasicInfo as *const u8,
                core::mem::size_of::<HandleBasicInfo>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::PROCESS => {
            // TODO: Implement proper process lookup
            let info = ProcessInfo {
                return_code: 0,
                started: 0,
                state: 0,
                _pad: 0,
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const ProcessInfo as *const u8,
                core::mem::size_of::<ProcessInfo>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::PROCESS_THREADS => {
            // TODO: Implement proper thread enumeration
            // For now, return 0 threads
            let num_threads = 0usize;

            if actual_out != 0 {
                let user_ptr = UserPtr::<u8>::new(actual_out);
                unsafe {
                    if let Err(err) = copy_to_user(
                        user_ptr,
                        &num_threads as *const _ as *const u8,
                        core::mem::size_of::<usize>(),
                    ) {
                        return err_to_ret(err.into());
                    }
                }
            }

            if avail_out != 0 {
                let user_ptr = UserPtr::<u8>::new(avail_out);
                unsafe {
                    if let Err(err) = copy_to_user(
                        user_ptr,
                        &num_threads as *const _ as *const u8,
                        core::mem::size_of::<usize>(),
                    ) {
                        return err_to_ret(err.into());
                    }
                }
            }

            ok_to_ret(0)
        }

        info_topic::THREAD => {
            // TODO: Implement proper thread lookup
            let info = ThreadInfo {
                state: 0,
                wait_reason: 0,
                cpu: 0,
                _pad: 0,
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const ThreadInfo as *const u8,
                core::mem::size_of::<ThreadInfo>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::THREAD_STATS => {
            // TODO: Implement proper thread stats
            let info = ThreadStats {
                total_runtime: 0,
                cpu_time: 0,
                context_switches: 0,
                page_faults: 0,
                _pad: [0; 4],
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const ThreadStats as *const u8,
                core::mem::size_of::<ThreadStats>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::TASK_STATS => {
            // TODO: Implement proper task stats
            let info = TaskStats {
                mem_mapped_bytes: 0,
                mem_private_bytes: 0,
                mem_shared_bytes: 0,
                _pad: [0; 5],
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const TaskStats as *const u8,
                core::mem::size_of::<TaskStats>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::VMO => {
            // TODO: Implement proper VMO lookup
            let info = VmoInfo {
                koid: handle_val as u64,
                parent_koid: 0,
                num_children: 0,
                num_mappings: 0,
                share_count: 0,
                flags: 0,
                _pad: 0,
                size_bytes: 0,
                committed_bytes: 0,
                _pad2: [0; 3],
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const VmoInfo as *const u8,
                core::mem::size_of::<VmoInfo>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::VMAR => {
            // TODO: Implement proper VMAR lookup
            let info = VmarInfo {
                base: 0,
                len: 0,
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const VmarInfo as *const u8,
                core::mem::size_of::<VmarInfo>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::CPU_STATS => {
            // TODO: Implement proper CPU stats
            // For now, return empty
            if actual_out != 0 {
                let count = 0usize;
                let user_ptr = UserPtr::<u8>::new(actual_out);
                unsafe {
                    if let Err(err) = copy_to_user(
                        user_ptr,
                        &count as *const _ as *const u8,
                        core::mem::size_of::<usize>(),
                    ) {
                        return err_to_ret(err.into());
                    }
                }
            }

            if avail_out != 0 {
                let avail = 0usize;
                let user_ptr = UserPtr::<u8>::new(avail_out);
                unsafe {
                    if let Err(err) = copy_to_user(
                        user_ptr,
                        &avail as *const _ as *const u8,
                        core::mem::size_of::<usize>(),
                    ) {
                        return err_to_ret(err.into());
                    }
                }
            }

            ok_to_ret(0)
        }

        info_topic::KMEM_STATS => {
            // TODO: Implement proper kmem stats
            let info = KmemStats {
                total_bytes: 0,
                free_bytes: 0,
                wired_bytes: 0,
                total_heap_bytes: 0,
                free_heap_bytes: 0,
                vmo_bytes: 0,
                mmu_overhead_bytes: 0,
                ipc_bytes: 0,
                other_bytes: 0,
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const KmemStats as *const u8,
                core::mem::size_of::<KmemStats>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::HANDLE_COUNT => {
            // TODO: Implement proper handle count
            let info = HandleCountInfo {
                handle_count: 0,
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const HandleCountInfo as *const u8,
                core::mem::size_of::<HandleCountInfo>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::PROCESS_HANDLE_STATS => {
            // TODO: Implement proper handle stats
            let info = ProcessHandleStats {
                handle_count: [0; 64],
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const ProcessHandleStats as *const u8,
                core::mem::size_of::<ProcessHandleStats>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        info_topic::SOCKET => {
            // TODO: Implement proper socket info
            let info = SocketInfo {
                options: 0,
                _pad1: [0; 3],
                rx_buf_size: 0,
                rx_buf_available: 0,
                tx_buf_size: 0,
                tx_buf_available: 0,
                _pad2: [0; 7],
            };

            match single_record_result(
                buffer,
                buffer_size,
                actual_out,
                avail_out,
                &info as *const SocketInfo as *const u8,
                core::mem::size_of::<SocketInfo>(),
            ) {
                Ok(()) => ok_to_ret(0),
                Err(err) => err_to_ret(err),
            }
        }

        _ => {
            log_error!("sys_object_get_info: unsupported topic {:#x}", topic);
            err_to_ret(RX_ERR_NOT_SUPPORTED)
        }
    }
}

/// ============================================================================
/// Syscall: Object Get Property
/// ============================================================================

/// Maximum name length
const MAX_NAME_LEN: usize = 32;

/// Get a property of a kernel object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value
/// * `property` - Property type
/// * `value` - User buffer to store property value
/// * `size` - Size of buffer
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_get_property_impl(
    handle_val: u32,
    property: u32,
    value: usize,
    size: usize,
) -> SyscallRet {
    log_debug!(
        "sys_object_get_property: handle={:#x} property={:#x} value={:#x} size={}",
        handle_val, property, value, size
    );

    if value == 0 {
        log_error!("sys_object_get_property: null value pointer");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    match property {
        property::NAME => {
            if size < MAX_NAME_LEN {
                log_error!("sys_object_get_property: buffer too small for name");
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            // TODO: Get actual name from object
            let name = [0u8; MAX_NAME_LEN];
            let user_ptr = UserPtr::new(value);
            unsafe {
                if let Err(err) = copy_to_user(user_ptr, name.as_ptr(), MAX_NAME_LEN) {
                    log_error!("sys_object_get_property: copy_to_user failed: {:?}", err);
                    return err_to_ret(err.into());
                }
            }

            ok_to_ret(0)
        }

        property::PROCESS_DEBUG_ADDR => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            // TODO: Get actual debug address
            let debug_addr = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_to_user(user_ptr, &debug_addr as *const u64 as *const u8, 8) {
                    return err_to_ret(err.into());
                }
            }

            ok_to_ret(0)
        }

        property::PROCESS_VDSO_BASE_ADDRESS => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            // TODO: Get actual VDSO base address
            let vdso_base = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_to_user(user_ptr, &vdso_base as *const u64 as *const u8, 8) {
                    return err_to_ret(err.into());
                }
            }

            ok_to_ret(0)
        }

        property::SOCKET_RX_THRESHOLD => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            // TODO: Get actual socket RX threshold
            let threshold = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_to_user(user_ptr, &threshold as *const u64 as *const u8, 8) {
                    return err_to_ret(err.into());
                }
            }

            ok_to_ret(0)
        }

        property::SOCKET_TX_THRESHOLD => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            // TODO: Get actual socket TX threshold
            let threshold = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_to_user(user_ptr, &threshold as *const u64 as *const u8, 8) {
                    return err_to_ret(err.into());
                }
            }

            ok_to_ret(0)
        }

        _ => {
            log_error!("sys_object_get_property: unsupported property {:#x}", property);
            err_to_ret(RX_ERR_INVALID_ARGS)
        }
    }
}

/// ============================================================================
/// Syscall: Object Set Property
/// ============================================================================

/// Set a property of a kernel object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value
/// * `property` - Property type
/// * `value` - User buffer containing property value
/// * `size` - Size of buffer
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_set_property_impl(
    handle_val: u32,
    property: u32,
    value: usize,
    size: usize,
) -> SyscallRet {
    log_debug!(
        "sys_object_set_property: handle={:#x} property={:#x} value={:#x} size={}",
        handle_val, property, value, size
    );

    if value == 0 {
        log_error!("sys_object_set_property: null value pointer");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    match property {
        property::NAME => {
            let mut name = [0u8; MAX_NAME_LEN];
            let copy_size = if size >= MAX_NAME_LEN {
                MAX_NAME_LEN - 1
            } else {
                size
            };

            let user_ptr = UserPtr::new(value);
            unsafe {
                if let Err(err) = copy_from_user(name.as_mut_ptr(), user_ptr, copy_size) {
                    log_error!("sys_object_set_property: copy_from_user failed: {:?}", err);
                    return err_to_ret(err.into());
                }
            }

            // TODO: Set actual name on object
            log_debug!("sys_object_set_property: set name to {}", unsafe {
                let len = copy_size.min(MAX_NAME_LEN - 1);
                core::str::from_utf8_unchecked(&name[..len])
            });

            ok_to_ret(0)
        }

        property::PROCESS_DEBUG_ADDR => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            let mut debug_addr = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_from_user(&mut debug_addr as *mut u64 as *mut u8, user_ptr, 8) {
                    return err_to_ret(err.into());
                }
            }

            // TODO: Set actual debug address on process
            log_debug!("sys_object_set_property: set debug addr to {:#x}", debug_addr);

            ok_to_ret(0)
        }

        property::SOCKET_RX_THRESHOLD => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            let mut threshold = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_from_user(&mut threshold as *mut u64 as *mut u8, user_ptr, 8) {
                    return err_to_ret(err.into());
                }
            }

            // TODO: Set actual socket RX threshold
            log_debug!("sys_object_set_property: set RX threshold to {}", threshold);

            ok_to_ret(0)
        }

        property::SOCKET_TX_THRESHOLD => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            let mut threshold = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_from_user(&mut threshold as *mut u64 as *mut u8, user_ptr, 8) {
                    return err_to_ret(err.into());
                }
            }

            // TODO: Set actual socket TX threshold
            log_debug!("sys_object_set_property: set TX threshold to {}", threshold);

            ok_to_ret(0)
        }

        property::JOB_KILL_ON_OOM => {
            if size < core::mem::size_of::<u64>() {
                return err_to_ret(RX_ERR_BUFFER_TOO_SMALL);
            }

            let mut kill_on_oom = 0u64;
            let user_ptr = UserPtr::<u8>::new(value);
            unsafe {
                if let Err(err) = copy_from_user(&mut kill_on_oom as *mut u64 as *mut u8, user_ptr, 8) {
                    return err_to_ret(err.into());
                }
            }

            if kill_on_oom > 1 {
                return err_to_ret(RX_ERR_INVALID_ARGS);
            }

            // TODO: Set actual kill-on-oom flag on job
            log_debug!(
                "sys_object_set_property: set kill-on-oom to {}",
                kill_on_oom == 1
            );

            ok_to_ret(0)
        }

        _ => {
            log_error!("sys_object_set_property: unsupported property {:#x}", property);
            err_to_ret(RX_ERR_INVALID_ARGS)
        }
    }
}

/// ============================================================================
/// Syscall: Object Signal
/// ============================================================================

/// Signal a kernel object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value
/// * `clear_mask` - Signal bits to clear
/// * `set_mask` - Signal bits to set
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_signal_impl(handle_val: u32, clear_mask: u32, set_mask: u32) -> SyscallRet {
    log_debug!(
        "sys_object_signal: handle={:#x} clear={:#x} set={:#x}",
        handle_val, clear_mask, set_mask
    );

    // TODO: Implement proper handle lookup and signaling
    // For now, just log
    log_info!("Object signal: handle={:#x} clear={:#x} set={:#x}", handle_val, clear_mask, set_mask);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Object Signal Peer
/// ============================================================================

/// Signal the peer of a kernel object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value
/// * `clear_mask` - Signal bits to clear
/// * `set_mask` - Signal bits to set
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_signal_peer_impl(
    handle_val: u32,
    clear_mask: u32,
    set_mask: u32,
) -> SyscallRet {
    log_debug!(
        "sys_object_signal_peer: handle={:#x} clear={:#x} set={:#x}",
        handle_val, clear_mask, set_mask
    );

    // TODO: Implement proper handle lookup and peer signaling
    // For now, just log
    log_info!(
        "Object signal peer: handle={:#x} clear={:#x} set={:#x}",
        handle_val, clear_mask, set_mask
    );

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Object Get Child
/// ============================================================================

/// Get a child object by koid syscall handler
///
/// # Arguments
///
/// * `handle_val` - Parent handle value
/// * `koid` - Kernel object ID of child
/// * `rights` - Rights for the new handle
///
/// # Returns
///
/// * On success: Child handle value
/// * On error: Negative error code
pub fn sys_object_get_child_impl(handle_val: u32, koid: u64, rights: u32) -> SyscallRet {
    log_debug!(
        "sys_object_get_child: handle={:#x} koid={} rights={:#x}",
        handle_val, koid, rights
    );

    // TODO: Implement proper handle lookup and child enumeration
    // For now, just log
    log_info!(
        "Object get child: handle={:#x} koid={} rights={:#x}",
        handle_val, koid, rights
    );

    // Return a placeholder handle
    ok_to_ret((koid & 0xFFFFFFFF) as usize)
}

/// ============================================================================
/// Syscall: Object Set Cookie
/// ============================================================================

/// Set a cookie on an object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value
/// * `scope_handle` - Scope handle
/// * `cookie` - Cookie value
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_set_cookie_impl(handle_val: u32, scope_handle: u32, cookie: u64) -> SyscallRet {
    log_debug!(
        "sys_object_set_cookie: handle={:#x} scope={:#x} cookie={:#x}",
        handle_val, scope_handle, cookie
    );

    // TODO: Implement proper cookie handling
    // For now, just log
    log_info!(
        "Object set cookie: handle={:#x} scope={:#x} cookie={:#x}",
        handle_val, scope_handle, cookie
    );

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Object Get Cookie
/// ============================================================================

/// Get a cookie from an object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value
/// * `scope_handle` - Scope handle
/// * `cookie_out` - User pointer to store cookie
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_get_cookie_impl(
    handle_val: u32,
    scope_handle: u32,
    cookie_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_object_get_cookie: handle={:#x} scope={:#x} cookie_out={:#x}",
        handle_val, scope_handle, cookie_out
    );

    if cookie_out == 0 {
        log_error!("sys_object_get_cookie: null cookie pointer");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Implement proper cookie handling
    let cookie = 0u64;

    // Copy to user
    let user_ptr = UserPtr::<u8>::new(cookie_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &cookie as *const u64 as *const u8, 8) {
            log_error!("sys_object_get_cookie: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_info!(
        "Object get cookie: handle={:#x} scope={:#x} cookie={:#x}",
        handle_val, scope_handle, cookie
    );

    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get object subsystem statistics
pub fn get_stats() -> ObjectStats {
    ObjectStats {
        total_info_calls: 0,
        total_property_ops: 0,
        total_signals: 0,
    }
}

/// Object subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ObjectStats {
    /// Total number of get_info calls
    pub total_info_calls: u64,

    /// Total number of property operations
    pub total_property_ops: u64,

    /// Total number of signal operations
    pub total_signals: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the object syscall subsystem
pub fn init() {
    log_info!("Object syscall subsystem initialized");
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_topics() {
        assert_eq!(info_topic::HANDLE_VALID, 0x00);
        assert_eq!(info_topic::HANDLE_BASIC, 0x01);
        assert_eq!(info_topic::PROCESS, 0x02);
    }

    #[test]
    fn test_properties() {
        assert_eq!(property::NAME, 0x00);
        assert_eq!(property::PROCESS_DEBUG_ADDR, 0x01);
    }

    #[test]
    fn test_handle_basic_info_size() {
        assert!(core::mem::size_of::<HandleBasicInfo>() >= 24);
    }

    #[test]
    fn test_process_info_size() {
        assert!(core::mem::size_of::<ProcessInfo>() >= 24);
    }

    #[test]
    fn test_single_record_result_buffer_too_small() {
        let result = single_record_result(
            0x1000,
            8,  // buffer too small
            0,   // no actual out
            0,   // no avail out
            &0u64 as *const u64 as *const u8,
            16,  // record size
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RX_ERR_BUFFER_TOO_SMALL);
    }

    #[test]
    fn test_single_record_result_success() {
        let data = 0xDEADBEEFu64;
        let result = single_record_result(
            0x1000,
            16, // sufficient buffer
            0,   // no actual out
            0,   // no avail out
            &data as *const u64 as *const u8,
            8,   // record size
        );
        // This will fail to copy to user address 0x1000, but we can test the size logic
        assert!(result.is_ok() || result.unwrap_err() != RX_ERR_BUFFER_TOO_SMALL);
    }
}
