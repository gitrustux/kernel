// Copyright 2025 Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::kernel::thread::get_current_thread;
use crate::vm::vm::is_user_address_range;

/// Copy data from user space to kernel space
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers and performs
/// a data copy operation that could cause memory corruption if used incorrectly.
///
/// # Arguments
///
/// * `dst` - Destination address in kernel space
/// * `src` - Source address in user space
/// * `len` - Number of bytes to copy
///
/// # Returns
///
/// * `RX_OK` if the copy was successful
/// * Error code otherwise
#[no_mangle]
pub unsafe extern "C" fn arch_copy_from_user(dst: *mut u8, src: *const u8, len: usize) -> rx_status_t {
    // The assembly code just does memcpy with fault handling. This is
    // the security check that an address from the user is actually a
    // valid userspace address so users can't access kernel memory.
    if !is_user_address_range(src as vaddr_t, len) {
        return RX_ERR_INVALID_ARGS;
    }

    _arm64_user_copy(
        dst as *mut core::ffi::c_void,
        src as *const core::ffi::c_void,
        len,
        &mut get_current_thread().arch.data_fault_resume,
    )
}

/// Copy data from kernel space to user space
///
/// # Safety
///
/// This function is unsafe because it dereferences raw pointers and performs
/// a data copy operation that could cause memory corruption if used incorrectly.
///
/// # Arguments
///
/// * `dst` - Destination address in user space
/// * `src` - Source address in kernel space
/// * `len` - Number of bytes to copy
///
/// # Returns
///
/// * `RX_OK` if the copy was successful
/// * Error code otherwise
#[no_mangle]
pub unsafe extern "C" fn arch_copy_to_user(dst: *mut u8, src: *const u8, len: usize) -> rx_status_t {
    if !is_user_address_range(dst as vaddr_t, len) {
        return RX_ERR_INVALID_ARGS;
    }

    _arm64_user_copy(
        dst as *mut core::ffi::c_void,
        src as *const core::ffi::c_void,
        len,
        &mut get_current_thread().arch.data_fault_resume,
    )
}

// Type definitions that would be imported from other modules
type vaddr_t = u64;
type rx_status_t = i32;

// Constants that would be imported from other modules
const RX_OK: i32 = 0;
const RX_ERR_INVALID_ARGS: i32 = -10;

// External function declarations
extern "C" {
    fn _arm64_user_copy(
        dst: *mut core::ffi::c_void,
        src: *const core::ffi::c_void,
        len: usize,
        fault_return: *mut *mut core::ffi::c_void,
    ) -> rx_status_t;
}

// ============================================================================
// Public API (with arm64_ prefix)
// ============================================================================

/// Copy data to user space (arm64_ prefix variant)
pub fn arm64_copy_to_user(dst: *mut u8, src: *const u8, len: usize) -> rx_status_t {
    unsafe { arch_copy_to_user(dst, src, len) }
}

/// Copy data from user space (arm64_ prefix variant)
pub fn arm64_copy_from_user(dst: *mut u8, src: *const u8, len: usize) -> rx_status_t {
    unsafe { arch_copy_from_user(dst, src, len) }
}