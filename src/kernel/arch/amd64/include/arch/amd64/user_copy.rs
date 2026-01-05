// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! User memory copy operations for x86
//!
//! This module provides low-level functions for safely copying data between
//! kernel and user memory spaces.

use crate::rustux::types::*;
use core::ffi::c_void;

/// Internal function used by arch_copy_from_user() and arch_copy_to_user()
///
/// This function should not be called directly except in the x86 usercopy
/// implementation.
///
/// # Arguments
///
/// * `dst` - Destination memory address
/// * `src` - Source memory address
/// * `len` - Number of bytes to copy
/// * `fault_return` - Return address to jump to if a fault occurs
///
/// # Returns
///
/// A status code indicating success or the type of failure
///
/// # Safety
///
/// This function is unsafe because it:
/// - Can cause memory access violations if addresses are invalid
/// - May perform unaligned memory accesses
/// - Affects the fault handling state of the system
pub unsafe fn _x86_copy_to_or_from_user(
    dst: *mut c_void,
    src: *const c_void,
    len: usize,
    fault_return: *mut *mut c_void,
) -> RxStatus {
    sys_x86_copy_to_or_from_user(dst, src, len, fault_return)
}

// Foreign function declaration for the system implementation
extern "C" {
    fn sys_x86_copy_to_or_from_user(
        dst: *mut c_void,
        src: *const c_void,
        len: usize,
        fault_return: *mut *mut c_void,
    ) -> RxStatus;
}