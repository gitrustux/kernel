// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Exception System Calls
//!
//! This module implements exception-related system calls.
//!
//! # Syscalls Implemented
//!
//! - `rx_task_bind_exception_port` - Bind exception port to task
//! - `rx_task_resume_from_exception` - Resume task from exception
//!
//! # Design
//!
//! Exception handling allows debuggers and crash reporters to receive
//! notifications when processes/threads encounter exceptions.

#![no_std]

use crate::kernel::object::{Handle, HandleTable, ObjectType, Rights};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Exception Constants
/// ============================================================================

/// Exception port options
pub mod exception_port_options {
    /// No special options
    pub const NONE: u32 = 0;

    /// This is a debugger exception port
    pub const DEBUGGER: u32 = 0x01;
}

/// Resume from exception options
pub mod resume_options {
    /// No special options
    pub const NONE: u32 = 0;

    /// Try next exception handler
    pub const TRY_NEXT: u32 = 0x01;
}

/// ============================================================================
/// Exception Port
/// ============================================================================

/// Maximum number of exception ports in the system
const MAX_EXCEPTION_PORTS: usize = 128;

/// Next exception port ID counter
static mut NEXT_EXCEPTION_PORT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new exception port ID
fn alloc_exception_port_id() -> u64 {
    unsafe { NEXT_EXCEPTION_PORT_ID.fetch_add(1, Ordering::Relaxed) }
}

/// Exception port types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionPortType {
    /// Thread exception port
    Thread = 0,

    /// Process exception port
    Process = 1,

    /// Job exception port
    Job = 2,

    /// Job debugger exception port
    JobDebugger = 3,

    /// Process debugger exception port
    Debugger = 4,
}

/// Exception port object
///
/// Represents a binding between a task (job/process/thread) and a port
/// for exception delivery.
pub struct ExceptionPort {
    /// Exception port ID
    id: u64,

    /// Exception port type
    port_type: ExceptionPortType,

    /// Port handle for exception delivery
    port_handle: u32,

    /// User-provided key
    key: u64,

    /// Target task handle
    target_handle: u32,

    /// Whether the port is bound
    bound: AtomicBool,
}

impl ExceptionPort {
    /// Create a new exception port
    pub fn new(
        port_type: ExceptionPortType,
        port_handle: u32,
        key: u64,
        target_handle: u32,
    ) -> Self {
        let id = alloc_exception_port_id();
        log_debug!(
            "ExceptionPort::new: id={} type={:?} port={:#x} key={:#x} target={:#x}",
            id,
            port_type,
            port_handle,
            key,
            target_handle
        );

        Self {
            id,
            port_type,
            port_handle,
            key,
            target_handle,
            bound: AtomicBool::new(false),
        }
    }

    /// Get exception port ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get exception port type
    pub fn port_type(&self) -> ExceptionPortType {
        self.port_type
    }

    /// Get port handle
    pub fn port_handle(&self) -> u32 {
        self.port_handle
    }

    /// Get key
    pub fn key(&self) -> u64 {
        self.key
    }

    /// Get target handle
    pub fn target_handle(&self) -> u32 {
        self.target_handle
    }

    /// Check if bound
    pub fn is_bound(&self) -> bool {
        self.bound.load(Ordering::Acquire)
    }

    /// Mark as bound
    pub fn bind(&self) {
        self.bound.store(true, Ordering::Release);
    }

    /// Unbind the exception port
    pub fn unbind(&self) {
        self.bound.store(false, Ordering::Release);
    }
}

/// ============================================================================
/// Syscall: Task Bind Exception Port
/// ============================================================================

/// Helper function to unbind exception port from object
///
/// # Arguments
///
/// * `obj_handle` - Object handle (job/process/thread)
/// * `debugger` - Whether this is a debugger port
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
fn object_unbind_exception_port(obj_handle: u32, debugger: bool) -> SyscallRet {
    log_debug!(
        "object_unbind_exception_port: obj={:#x} debugger={}",
        obj_handle,
        debugger
    );

    // Get the object from handle
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    let handle = match handle_table.get(obj_handle) {
        Some(h) => h.clone(),
        None => {
            log_error!("object_unbind_exception_port: bad handle");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Check object type and unbind
    match handle.object_type() {
        ObjectType::Job => {
            log_debug!("object_unbind_exception_port: unbinding from job");
            // TODO: Implement job exception port unbinding
            ok_to_ret(0)
        }

        ObjectType::Process => {
            log_debug!("object_unbind_exception_port: unbinding from process");
            // TODO: Implement process exception port unbinding
            ok_to_ret(0)
        }

        ObjectType::Thread => {
            if debugger {
                log_error!("object_unbind_exception_port: debugger port not allowed on thread");
                return err_to_ret(RX_ERR_INVALID_ARGS);
            }
            log_debug!("object_unbind_exception_port: unbinding from thread");
            // TODO: Implement thread exception port unbinding
            ok_to_ret(0)
        }

        _ => {
            log_error!("object_unbind_exception_port: wrong type");
            err_to_ret(RX_ERR_WRONG_TYPE)
        }
    }
}

/// Helper function to bind exception port to task
///
/// # Arguments
///
/// * `obj_handle` - Object handle (job/process/thread)
/// * `port_handle` - Port handle for exception delivery
/// * `key` - User-provided key
/// * `debugger` - Whether this is a debugger port
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
fn task_bind_exception_port_impl(
    obj_handle: u32,
    port_handle: u32,
    key: u64,
    debugger: bool,
) -> SyscallRet {
    log_debug!(
        "task_bind_exception_port_impl: obj={:#x} port={:#x} key={:#x} debugger={}",
        obj_handle,
        port_handle,
        key,
        debugger
    );

    // Get the object from handle
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    let handle = match handle_table.get(obj_handle) {
        Some(h) => h.clone(),
        None => {
            log_error!("task_bind_exception_port_impl: bad handle");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Determine exception port type
    let port_type = match (handle.object_type(), debugger) {
        (ObjectType::Job, true) => ExceptionPortType::JobDebugger,
        (ObjectType::Job, false) => ExceptionPortType::Job,
        (ObjectType::Process, true) => ExceptionPortType::Debugger,
        (ObjectType::Process, false) => ExceptionPortType::Process,
        (ObjectType::Thread, false) => ExceptionPortType::Thread,
        (ObjectType::Thread, true) => {
            log_error!("task_bind_exception_port_impl: debugger port not allowed on thread");
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
        _ => {
            log_error!("task_bind_exception_port_impl: wrong type");
            return err_to_ret(RX_ERR_WRONG_TYPE);
        }
    };

    // Create exception port
    let eport = Arc::new(ExceptionPort::new(
        port_type,
        port_handle,
        key,
        obj_handle,
    ));

    // Bind exception port to target
    // TODO: Implement actual binding
    eport.bind();

    log_debug!("task_bind_exception_port_impl: success port_id={}", eport.id());
    ok_to_ret(0)
}

/// Bind exception port to task syscall handler
///
/// # Arguments
///
/// * `handle` - Object handle (job/process/thread)
/// * `port` - Port handle (or ZX_HANDLE_INVALID to unbind)
/// * `key` - User-provided key
/// * `options` - Options (DEBUGGER flag)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_task_bind_exception_port_impl(
    handle: u32,
    port: u32,
    key: u64,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_task_bind_exception_port: handle={:#x} port={:#x} key={:#x} options={:#x}",
        handle,
        port,
        key,
        options
    );

    // Validate options
    if options & !exception_port_options::DEBUGGER != 0 {
        log_error!("sys_task_bind_exception_port: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    let debugger = (options & exception_port_options::DEBUGGER) != 0;

    // Invalid port means unbind
    if port == 0 {
        object_unbind_exception_port(handle, debugger)
    } else {
        task_bind_exception_port_impl(handle, port, key, debugger)
    }
}

/// ============================================================================
/// Syscall: Task Resume From Exception
/// ============================================================================

/// Resume task from exception syscall handler
///
/// # Arguments
///
/// * `handle` - Thread handle
/// * `port` - Port handle
/// * `options` - Options (TRY_NEXT flag)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_task_resume_from_exception_impl(
    handle: u32,
    port: u32,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_task_resume_from_exception: handle={:#x} port={:#x} options={:#x}",
        handle,
        port,
        options
    );

    // Validate options
    if options != resume_options::NONE && options != resume_options::TRY_NEXT {
        log_error!("sys_task_resume_from_exception: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Get thread from handle
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    let thread_handle = match handle_table.get(handle) {
        Some(h) => h.clone(),
        None => {
            log_error!("sys_task_resume_from_exception: bad thread handle");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Check object type
    if thread_handle.object_type() != ObjectType::Thread {
        log_error!("sys_task_resume_from_exception: handle is not a thread");
        return err_to_ret(RX_ERR_WRONG_TYPE);
    }

    // TODO: Implement actual exception resume
    if options & resume_options::TRY_NEXT != 0 {
        log_debug!("sys_task_resume_from_exception: mark as not handled (try next)");
        // thread->MarkExceptionNotHandled(eport.get());
    } else {
        log_debug!("sys_task_resume_from_exception: mark as handled");
        // thread->MarkExceptionHandled(eport.get());
    }

    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get exception subsystem statistics
pub fn get_stats() -> ExceptionStats {
    ExceptionStats {
        total_exception_ports: 0, // TODO: Track
        total_binds: 0,           // TODO: Track
        total_unbinds: 0,         // TODO: Track
        total_resumes: 0,         // TODO: Track
    }
}

/// Exception subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ExceptionStats {
    /// Total number of exception ports
    pub total_exception_ports: usize,

    /// Total number of bind operations
    pub total_binds: u64,

    /// Total number of unbind operations
    pub total_unbinds: u64,

    /// Total number of resume operations
    pub total_resumes: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the exception syscall subsystem
pub fn init() {
    log_info!("Exception syscall subsystem initialized");
    log_info!("  Max exception ports: {}", MAX_EXCEPTION_PORTS);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_port_options() {
        assert_eq!(exception_port_options::NONE, 0);
        assert_eq!(exception_port_options::DEBUGGER, 0x01);
    }

    #[test]
    fn test_resume_options() {
        assert_eq!(resume_options::NONE, 0);
        assert_eq!(resume_options::TRY_NEXT, 0x01);
    }

    #[test]
    fn test_exception_port_type() {
        assert_eq!(ExceptionPortType::Thread as u32, 0);
        assert_eq!(ExceptionPortType::Process as u32, 1);
        assert_eq!(ExceptionPortType::Job as u32, 2);
        assert_eq!(ExceptionPortType::JobDebugger as u32, 3);
        assert_eq!(ExceptionPortType::Debugger as u32, 4);
    }

    #[test]
    fn test_exception_port_new() {
        let eport = ExceptionPort::new(
            ExceptionPortType::Process,
            42,
            0x1234,
            100,
        );

        assert_eq!(eport.port_handle(), 42);
        assert_eq!(eport.key(), 0x1234);
        assert_eq!(eport.target_handle(), 100);
        assert!(!eport.is_bound());
    }

    #[test]
    fn test_exception_port_bind_unbind() {
        let eport = ExceptionPort::new(
            ExceptionPortType::Thread,
            1,
            0,
            2,
        );

        assert!(!eport.is_bound());

        eport.bind();
        assert!(eport.is_bound());

        eport.unbind();
        assert!(!eport.is_bound());
    }
}
