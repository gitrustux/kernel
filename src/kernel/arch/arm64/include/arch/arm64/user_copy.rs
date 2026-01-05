// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::rustux::compiler::*;
use crate::rustux::types::*;

/// This is the same as memcpy, except that it takes the additional
/// argument of &current_thread()->arch.data_fault_resume, where it
/// temporarily stores the fault recovery PC for bad page faults to user
/// addresses during the call. arch_copy_from_user and arch_copy_to_user
/// should be the only callers of this.
pub unsafe fn _arm64_user_copy(
    dst: *mut core::ffi::c_void,
    src: *const core::ffi::c_void,
    len: size_t,
    fault_return: *mut *mut core::ffi::c_void,
) -> rx_status_t;