// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Handle Operations
//!
//! This module implements handle-related system calls.
//!
//! # Syscalls Implemented
//!
//! - `rx_handle_close` - Close a single handle
//! - `rx_handle_close_many` - Close multiple handles
//! - `rx_handle_duplicate` - Duplicate a handle
//! - `rx_handle_replace` - Replace a handle

#![no_std]

use crate::kernel::object::{Handle, HandleOwner, HandleTable, Rights};
use crate::kernel::usercopy::{copy_from_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Handle Operations
/// ============================================================================

/// Invalid handle value
pub const ZX_HANDLE_INVALID: u32 = 0;

/// Special rights value meaning "same rights"
pub const ZX_RIGHT_SAME_RIGHTS: u32 = 0x80000000;

/// Close a single handle
///
/// Closing the "never a handle" invalid handle is not an error.
/// It's like free(NULL).
///
/// # Arguments
///
/// * `handle_value` - Handle to close
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_handle_close_impl(handle_value: u32) -> SyscallRet {
    log_debug!("sys_handle_close: handle={:#x}", handle_value);

    // Closing the invalid handle is not an error
    if handle_value == ZX_HANDLE_INVALID {
        return ok_to_ret(0);
    }

    // Get the current process's handle table
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    // Remove the handle
    match handle_table.remove(handle_value) {
        Some(_handle) => {
            log_debug!("sys_handle_close: success");
            ok_to_ret(0)
        }
        None => {
            log_error!("sys_handle_close: bad handle");
            err_to_ret(RX_ERR_BAD_HANDLE)
        }
    }
}

/// Close multiple handles
///
/// # Arguments
///
/// * `handles` - User pointer to array of handles
/// * `num_handles` - Number of handles to close
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_handle_close_many_impl(handles: usize, num_handles: usize) -> SyscallRet {
    log_debug!(
        "sys_handle_close_many: handles={:#x} num_handles={}",
        handles, num_handles
    );

    if num_handles == 0 {
        return ok_to_ret(0);
    }

    // Limit the number of handles to prevent abuse
    if num_handles > 1024 {
        log_error!("sys_handle_close_many: too many handles");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Copy handles from user
    let mut handle_buf = alloc::vec![0u32; num_handles];
    let user_ptr = UserPtr::<u32>::new(handles);
    unsafe {
        if let Err(err) = copy_from_user(handle_buf.as_mut_ptr(), user_ptr, num_handles) {
            log_error!("sys_handle_close_many: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Get the current process's handle table
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    // Remove each handle
    let mut removed = 0;
    for &handle_value in &handle_buf {
        if handle_value == ZX_HANDLE_INVALID {
            continue;
        }

        match handle_table.remove(handle_value) {
            Some(_) => removed += 1,
            None => {
                // Continue removing other handles even if one fails
                log_debug!("sys_handle_close_many: bad handle {:#x}", handle_value);
            }
        }
    }

    log_debug!("sys_handle_close_many: removed {} handles", removed);
    ok_to_ret(0)
}

/// Helper function for handle duplicate and replace
///
/// # Arguments
///
/// * `is_replace` - Whether to replace (true) or duplicate (false)
/// * `handle_value` - Source handle
/// * `rights` - Rights for the new handle
/// * `handle_out` - User pointer to store the new handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
fn handle_dup_replace_impl(
    is_replace: bool,
    handle_value: u32,
    rights: u32,
    handle_out: usize,
) -> SyscallRet {
    log_debug!(
        "handle_dup_replace: is_replace={} handle={:#x} rights={:#x}",
        is_replace,
        handle_value,
        rights
    );

    // Get the current process's handle table
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    // Get the source handle
    let source = match handle_table.get(handle_value) {
        Some(h) => h.clone(),
        None => {
            log_error!("handle_dup_replace: bad handle");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Check duplicate rights if not replacing
    if !is_replace {
        if !source.has_right(Rights::DUPLICATE) {
            log_error!("handle_dup_replace: access denied");
            return err_to_ret(RX_ERR_ACCESS_DENIED);
        }
    }

    // Determine the rights for the new handle
    let new_rights = if rights == ZX_RIGHT_SAME_RIGHTS {
        source.rights()
    } else {
        // Check if requested rights are a subset of source rights
        let source_rights = source.rights();
        if (source_rights & rights) != rights {
            log_error!("handle_dup_replace: invalid rights");
            if is_replace {
                handle_table.remove(handle_value);
            }
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
        rights
    };

    // If replacing, remove the old handle first
    if is_replace {
        handle_table.remove(handle_value);
    }

    // Create the new handle
    let new_handle = Handle::new(source.object().clone(), new_rights);
    let new_handle_value = match handle_table.add(new_handle) {
        Ok(val) => val,
        Err(err) => {
            log_error!("handle_dup_replace: failed to add handle: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Write the new handle value to user
    let user_ptr = UserPtr::<u32>::new(handle_out);
    unsafe {
        if let Err(err) = crate::kernel::usercopy::copy_to_user(
            user_ptr,
            &new_handle_value as *const u32,
            1,
        ) {
            log_error!("handle_dup_replace: copy_to_user failed: {:?}", err);
            // Clean up the new handle
            let _ = handle_table.remove(new_handle_value);
            return err_to_ret(err.into());
        }
    }

    log_debug!("handle_dup_replace: success new_handle={:#x}", new_handle_value);
    ok_to_ret(0)
}

/// Duplicate a handle
///
/// Creates a new handle referring to the same kernel object with the specified rights.
/// The original handle remains valid.
///
/// # Arguments
///
/// * `handle_value` - Source handle
/// * `rights` - Rights for the new handle (or ZX_RIGHT_SAME_RIGHTS)
/// * `handle_out` - User pointer to store the new handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_handle_duplicate_impl(
    handle_value: u32,
    rights: u32,
    handle_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_handle_duplicate: handle={:#x} rights={:#x}",
        handle_value,
        rights
    );
    handle_dup_replace_impl(false, handle_value, rights, handle_out)
}

/// Replace a handle
///
/// Creates a new handle referring to the same kernel object with the specified rights,
/// and closes the original handle.
///
/// # Arguments
///
/// * `handle_value` - Source handle (will be closed)
/// * `rights` - Rights for the new handle (or ZX_RIGHT_SAME_RIGHTS)
/// * `handle_out` - User pointer to store the new handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_handle_replace_impl(
    handle_value: u32,
    rights: u32,
    handle_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_handle_replace: handle={:#x} rights={:#x}",
        handle_value,
        rights
    );
    handle_dup_replace_impl(true, handle_value, rights, handle_out)
}

/// ============================================================================
/// Syscall: Handle Transfer
/// ============================================================================

/// Transfer a handle to another process via channel write
///
/// This syscall is used when writing a handle to a channel. It validates
/// that the handle can be transferred and prepares it for the receiving process.
///
/// # Arguments
///
/// * `handle_value` - Handle to transfer
/// * `new_rights` - Rights for the handle in the receiving process
/// * `options` - Transfer options (must be 0)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_handle_transfer_impl(
    handle_value: u32,
    new_rights: u32,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_handle_transfer: handle={:#x} rights={:#x} options={:#x}",
        handle_value,
        new_rights,
        options
    );

    // Validate options
    if options != 0 {
        log_error!("sys_handle_transfer: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Invalid handle cannot be transferred
    if handle_value == ZX_HANDLE_INVALID {
        log_error!("sys_handle_transfer: invalid handle");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Get the current process's handle table
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    // Get the source handle
    let source = match handle_table.get(handle_value) {
        Some(h) => h.clone(),
        None => {
            log_error!("sys_handle_transfer: bad handle");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Check if handle has transfer right
    if !source.has_right(Rights::TRANSFER) {
        log_error!("sys_handle_transfer: access denied (no TRANSFER right)");
        return err_to_ret(RX_ERR_ACCESS_DENIED);
    }

    // Determine the rights for the transferred handle
    let final_rights = if new_rights == ZX_RIGHT_SAME_RIGHTS {
        source.rights()
    } else {
        // Check if requested rights are a subset of source rights
        let source_rights = source.rights();
        if (source_rights & new_rights) != new_rights {
            log_error!("sys_handle_transfer: invalid rights");
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
        new_rights
    };

    // Remove the handle from current process (it's being transferred)
    handle_table.remove(handle_value);

    // Note: The actual transfer happens via channel_write
    // This syscall just validates and removes the handle from the sender
    // The channel write mechanism will add it to the receiving process

    log_debug!(
        "sys_handle_transfer: success handle={:#x} final_rights={:#x}",
        handle_value,
        final_rights
    );

    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get handle operations statistics
pub fn get_stats() -> HandleOpsStats {
    HandleOpsStats {
        total_close: 0,     // TODO: Track
        total_dup: 0,       // TODO: Track
        total_replace: 0,   // TODO: Track
    }
}

/// Handle operations statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HandleOpsStats {
    /// Total close operations
    pub total_close: u64,

    /// Total duplicate operations
    pub total_dup: u64,

    /// Total replace operations
    pub total_replace: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the handle operations subsystem
pub fn init() {
    log_info!("Handle operations subsystem initialized");
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_close_invalid() {
        let result = sys_handle_close_impl(ZX_HANDLE_INVALID);
        assert!(result >= 0);
    }

    #[test]
    fn test_handle_same_rights_const() {
        assert_eq!(ZX_RIGHT_SAME_RIGHTS, 0x80000000);
    }

    #[test]
    fn test_handle_invalid_const() {
        assert_eq!(ZX_HANDLE_INVALID, 0);
    }
}
