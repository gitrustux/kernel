// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Debug System Calls
//!
//! This module implements debug-related system calls.
//!
//! # Syscalls Implemented
//!
//! - `rx_debug_read` - Read from debug console
//! - `rx_debug_write` - Write to debug console
//! - `rx_debug_send_command` - Send command to debug console
//! - `rx_ktrace_read` - Read kernel trace data
//! - `rx_ktrace_control` - Control kernel tracing
//! - `rx_ktrace_write` - Write to kernel trace
//! - `rx_mtrace_control` - Control memory tracing


use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Debug Constants
/// ============================================================================

/// Maximum debug write size
const MAX_DEBUG_WRITE_SIZE: usize = 256;

/// Maximum name length
const MAX_NAME_LEN: usize = 32;

/// ============================================================================
/// KTrace Constants
/// ============================================================================

/// KTrace actions
pub mod ktrace_action {
    /// New probe
    pub const NEW_PROBE: u32 = 0;

    /// Start tracing
    pub const START: u32 = 1;

    /// Stop tracing
    pub const STOP: u32 = 2;

    /// Reset trace
    pub const RESET: u32 = 3;
}

/// ============================================================================
/// MTrace Constants
/// ============================================================================

/// MTrace kinds
pub mod mtrace_kind {
    /// Hardware tracing
    pub const HARDWARE: u32 = 0;

    /// CPU tracing
    pub const CPU: u32 = 1;

    /// Memory tracing
    pub const MEMORY: u32 = 2;
}

/// ============================================================================
/// Syscall: Debug Read
/// ============================================================================

/// Read from debug console
///
/// # Arguments
///
/// * `handle` - Resource handle (must be root resource)
/// * `buffer` - User pointer to buffer
/// * `len` - User pointer to length (in/out)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_debug_read_impl(handle: u32, buffer: usize, len: usize) -> SyscallRet {
    log_debug!(
        "sys_debug_read: handle={:#x} buffer={:#x} len={:#x}",
        handle,
        buffer,
        len
    );

    // TODO: Validate resource handle

    // Get the requested length
    let mut read_len = 0;
    let len_ptr = UserPtr::<u8>::new(len);
    unsafe {
        if let Err(err) = copy_from_user(&mut read_len as *mut usize as *mut u8, len_ptr, 1) {
            log_error!("sys_debug_read: copy_from_user len failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Limit read size
    if read_len > MAX_DEBUG_WRITE_SIZE {
        read_len = MAX_DEBUG_WRITE_SIZE;
    }

    // Read characters from console
    let mut chars_read = 0;
    let buffer_ptr = UserPtr::<u8>::new(buffer);

    for i in 0..read_len {
        // TODO: Implement actual console read
        // For now, just fill with zeros
        let c = 0u8;

        unsafe {
            if let Err(err) = copy_to_user(
                UserPtr::<u8>::new(buffer + i),
                &c as *const u8,
                1,
            ) {
                log_error!("sys_debug_read: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }

        chars_read += 1;
    }

    // Write back actual length
    unsafe {
        if let Err(err) = copy_to_user(len_ptr, &(chars_read as usize) as *const usize as *const u8, 1) {
            log_error!("sys_debug_read: copy_to_user len failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_debug_read: success, read {} bytes", chars_read);
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Debug Write
/// ============================================================================

/// Write to debug console
///
/// # Arguments
///
/// * `buffer` - User pointer to buffer
/// * `len` - Length of buffer
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_debug_write_impl(buffer: usize, len: usize) -> SyscallRet {
    log_debug!(
        "sys_debug_write: buffer={:#x} len={}",
        buffer,
        len
    );

    // Limit write size
    let write_len = len.min(MAX_DEBUG_WRITE_SIZE);

    // Allocate buffer
    let mut buf = alloc::vec![0u8; write_len];
    let user_ptr = UserPtr::<u8>::new(buffer);

    unsafe {
        if let Err(err) = copy_from_user(buf.as_mut_ptr(), user_ptr, write_len) {
            log_error!("sys_debug_write: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Convert to string and log
    let str_result = alloc::str::from_utf8(&buf);
    match str_result {
        Ok(s) => {
            log_info!("DEBUG: {}", s);
        }
        Err(_) => {
            log_info!("DEBUG: {:x?}", buf);
        }
    }

    // TODO: Write to serial console
    // dlog_serial_write(buf.as_ptr(), write_len);

    log_debug!("sys_debug_write: success");
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Debug Send Command
/// ============================================================================

/// Send command to debug console
///
/// # Arguments
///
/// * `handle` - Resource handle (must be root resource)
/// * `buffer` - User pointer to command buffer
/// * `len` - Length of command
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_debug_send_command_impl(
    handle: u32,
    buffer: usize,
    len: usize,
) -> SyscallRet {
    log_debug!(
        "sys_debug_send_command: handle={:#x} buffer={:#x} len={}",
        handle,
        buffer,
        len
    );

    // TODO: Validate resource handle

    // Limit command size
    if len > MAX_DEBUG_WRITE_SIZE {
        log_error!("sys_debug_send_command: command too long");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Allocate buffer (+2 for newline and null terminator)
    let mut buf = alloc::vec![0u8; len + 2];
    let user_ptr = UserPtr::<u8>::new(buffer);

    unsafe {
        if let Err(err) = copy_from_user(buf.as_mut_ptr(), user_ptr, len) {
            log_error!("sys_debug_send_command: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    buf[len] = b'\n';
    buf[len + 1] = 0;

    // Convert to string
    let str_result = alloc::str::from_utf8(&buf[..len]);
    match str_result {
        Ok(s) => {
            log_info!("DEBUG COMMAND: {}", s.trim());
        }
        Err(_) => {
            log_info!("DEBUG COMMAND: {:x?}", &buf[..len]);
        }
    }

    // TODO: Execute console command
    // console_run_script(buf.as_ptr());

    log_debug!("sys_debug_send_command: success");
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: KTrace Read
/// ============================================================================

/// Read kernel trace data
///
/// # Arguments
///
/// * `handle` - Resource handle (must be root resource)
/// * `data` - User pointer to data buffer
/// * `offset` - Offset in trace buffer
/// * `len` - Length to read
/// * `actual` - User pointer to store actual bytes read
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_ktrace_read_impl(
    handle: u32,
    data: usize,
    offset: u32,
    len: usize,
    actual: usize,
) -> SyscallRet {
    log_debug!(
        "sys_ktrace_read: handle={:#x} data={:#x} offset={} len={}",
        handle,
        data,
        offset,
        len
    );

    // TODO: Validate resource handle
    // TODO: Implement actual ktrace read

    // For now, return zero bytes read
    let actual_len = 0usize;
    let actual_ptr = UserPtr::<u8>::new(actual);
    unsafe {
        if let Err(err) = copy_to_user(actual_ptr, &actual_len as *const usize as *const u8, 1) {
            log_error!("sys_ktrace_read: copy_to_user actual failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_ktrace_read: success, read 0 bytes");
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: KTrace Control
/// ============================================================================

/// Control kernel tracing
///
/// # Arguments
///
/// * `handle` - Resource handle (must be root resource)
/// * `action` - Action to perform
/// * `options` - Options for the action
/// * `ptr` - User pointer to options data
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_ktrace_control_impl(
    handle: u32,
    action: u32,
    options: u32,
    ptr: usize,
) -> SyscallRet {
    log_debug!(
        "sys_ktrace_control: handle={:#x} action={} options={:#x}",
        handle,
        action,
        options
    );

    // TODO: Validate resource handle

    match action {
        ktrace_action::NEW_PROBE => {
            // Read probe name
            let mut name_buf = alloc::vec![0u8; MAX_NAME_LEN];
            let user_ptr = UserPtr::<u8>::new(ptr);

            unsafe {
                if let Err(err) = copy_from_user(
                    name_buf.as_mut_ptr(),
                    user_ptr,
                    MAX_NAME_LEN - 1,
                ) {
                    log_error!("sys_ktrace_control: copy_from_user failed: {:?}", err);
                    return err_to_ret(err.into());
                }
            }

            // Null terminate
            if let Some(pos) = name_buf.iter().position(|&c| c == 0) {
                name_buf.truncate(pos);
            }

            let name = alloc::string::String::from_utf8_lossy(&name_buf);
            log_info!("ktrace: new probe '{}'", name);

            // TODO: Implement actual ktrace control
            ok_to_ret(0)
        }

        ktrace_action::START => {
            log_info!("ktrace: start");
            ok_to_ret(0)
        }

        ktrace_action::STOP => {
            log_info!("ktrace: stop");
            ok_to_ret(0)
        }

        ktrace_action::RESET => {
            log_info!("ktrace: reset");
            ok_to_ret(0)
        }

        _ => {
            log_error!("sys_ktrace_control: invalid action {}", action);
            err_to_ret(RX_ERR_INVALID_ARGS)
        }
    }
}

/// ============================================================================
/// Syscall: KTrace Write
/// ============================================================================

/// Write to kernel trace
///
/// # Arguments
///
/// * `handle` - Resource handle (must be root resource)
/// * `event_id` - Event ID (max 0x7FF)
/// * `arg0` - Argument 0
/// * `arg1` - Argument 1
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_ktrace_write_impl(
    handle: u32,
    event_id: u32,
    arg0: u32,
    arg1: u32,
) -> SyscallRet {
    log_debug!(
        "sys_ktrace_write: handle={:#x} event_id={:#x} arg0={} arg1={}",
        handle,
        event_id,
        arg0,
        arg1
    );

    // TODO: Validate resource handle

    // Validate event ID
    if event_id > 0x7FF {
        log_error!("sys_ktrace_write: invalid event_id {:#x}", event_id);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Implement actual ktrace write
    log_debug!("ktrace: write event={:#x} args=({}, {})", event_id, arg0, arg1);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: MTrace Control
/// ============================================================================

/// Control memory tracing
///
/// # Arguments
///
/// * `handle` - Resource handle (must be root resource)
/// * `kind` - Kind of tracing
/// * `action` - Action to perform
/// * `options` - Options for the action
/// * `ptr` - User pointer to options data
/// * `size` - Size of options data
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_mtrace_control_impl(
    handle: u32,
    kind: u32,
    action: u32,
    options: u32,
    ptr: usize,
    size: usize,
) -> SyscallRet {
    log_debug!(
        "sys_mtrace_control: handle={:#x} kind={} action={} options={:#x}",
        handle,
        kind,
        action,
        options
    );

    // TODO: Validate resource handle
    // TODO: Implement actual mtrace control

    log_info!("mtrace: kind={} action={} (stub)", kind, action);
    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get debug subsystem statistics
pub fn get_stats() -> DebugStats {
    DebugStats {
        total_debug_reads: 0,    // TODO: Track
        total_debug_writes: 0,   // TODO: Track
        total_ktrace_ops: 0,     // TODO: Track
    }
}

/// Debug subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DebugStats {
    /// Total debug read operations
    pub total_debug_reads: u64,

    /// Total debug write operations
    pub total_debug_writes: u64,

    /// Total ktrace operations
    pub total_ktrace_ops: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the debug syscall subsystem
pub fn init() {
    log_info!("Debug syscall subsystem initialized");
    log_info!("  Max debug write size: {}", MAX_DEBUG_WRITE_SIZE);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_write_max_size() {
        assert_eq!(MAX_DEBUG_WRITE_SIZE, 256);
    }

    #[test]
    fn test_ktrace_action_consts() {
        assert_eq!(ktrace_action::NEW_PROBE, 0);
        assert_eq!(ktrace_action::START, 1);
        assert_eq!(ktrace_action::STOP, 2);
        assert_eq!(ktrace_action::RESET, 3);
    }

    #[test]
    fn test_mtrace_kind_consts() {
        assert_eq!(mtrace_kind::HARDWARE, 0);
        assert_eq!(mtrace_kind::CPU, 1);
        assert_eq!(mtrace_kind::MEMORY, 2);
    }

    #[test]
    fn test_debug_write_impl() {
        // Test with null buffer (should handle gracefully)
        let result = sys_debug_write_impl(0, 0);
        assert!(result >= 0);
    }
}
