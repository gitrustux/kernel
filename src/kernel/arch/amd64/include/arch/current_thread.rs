// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Current thread access for x86_64
//!
//! This module provides functions for accessing the current thread on x86_64
//! platforms, using the GS segment register that points to per-CPU data.

use crate::kernel::arch::amd64::mp::PERCPU_CURRENT_THREAD_OFFSET;
use crate::kernel::thread::Thread;
use core::ptr::NonNull;

/// Get a reference to the current thread
///
/// This function reads directly from GS rather than via the per-CPU structure
/// to ensure atomicity. This prevents race conditions that could occur if
/// a context switch happened between reading the per-CPU pointer and reading
/// the current_thread field.
///
/// # Returns
///
/// A reference to the current thread
///
/// # Safety
///
/// This function is unsafe because:
/// 1. It reads directly from CPU-specific memory
/// 2. It assumes the GS segment is properly set up
/// 3. It assumes there is a valid Thread at the expected location
#[inline]
pub unsafe fn get_current_thread() -> &'static mut Thread {
    // Read thread pointer from GS segment
    let thread_ptr = x86_read_gs_offset64(PERCPU_CURRENT_THREAD_OFFSET) as *mut Thread;
    
    // SAFETY: The pointer is guaranteed to be valid as long as the GS segment
    // is properly set up, which is a requirement for using this function.
    // Since we're in kernel mode, the thread can't be deallocated while it's
    // marked as current.
    &mut *thread_ptr
}

/// Set the current thread
///
/// This function writes directly to GS rather than via the per-CPU structure
/// to ensure atomicity. This prevents race conditions during thread switching.
///
/// # Arguments
///
/// * `thread` - Pointer to the thread to set as current
///
/// # Safety
///
/// This function is unsafe because:
/// 1. It writes directly to CPU-specific memory
/// 2. It assumes the GS segment is properly set up
/// 3. The thread must remain valid for the duration it's marked as current
#[inline]
pub unsafe fn set_current_thread(thread: *mut Thread) {
    // Write thread pointer to GS segment
    x86_write_gs_offset64(PERCPU_CURRENT_THREAD_OFFSET, thread as u64);
}

// Foreign function declarations for accessing the GS segment
extern "C" {
    #[link_name = "x86_read_gs_offset64"]
    fn sys_x86_read_gs_offset64(offset: u32) -> u64;
    #[link_name = "x86_write_gs_offset64"]
    fn sys_x86_write_gs_offset64(offset: u32, value: u64);
    #[link_name = "x86_read_gs_offset32"]
    fn sys_x86_read_gs_offset32(offset: u32) -> u32;
    #[link_name = "x86_write_gs_offset32"]
    fn sys_x86_write_gs_offset32(offset: u32, value: u32);
}

/// Read a 32-bit value from the GS segment at the specified offset
///
/// # Arguments
///
/// * `offset` - Offset into the GS segment
///
/// # Returns
///
/// The 32-bit value at the specified offset
///
/// # Safety
///
/// This function is unsafe because it performs an untyped read from the GS segment.
#[inline]
pub unsafe fn x86_read_gs_offset32(offset: u32) -> u32 {
    sys_x86_read_gs_offset32(offset)
}

/// Write a 32-bit value to the GS segment at the specified offset
///
/// # Arguments
///
/// * `offset` - Offset into the GS segment
/// * `value` - Value to write
///
/// # Safety
///
/// This function is unsafe because it performs an untyped write to the GS segment.
#[inline]
pub unsafe fn x86_write_gs_offset32(offset: u32, value: u32) {
    sys_x86_write_gs_offset32(offset, value)
}

/// Read a 64-bit value from the GS segment at the specified offset
///
/// # Arguments
///
/// * `offset` - Offset into the GS segment
///
/// # Returns
///
/// The 64-bit value at the specified offset
///
/// # Safety
///
/// This function is unsafe because it performs an untyped read from the GS segment.
#[inline]
pub unsafe fn x86_read_gs_offset64(offset: u32) -> u64 {
    sys_x86_read_gs_offset64(offset)
}

/// Write a 64-bit value to the GS segment at the specified offset
///
/// # Arguments
///
/// * `offset` - Offset into the GS segment
/// * `value` - Value to write
///
/// # Safety
///
/// This function is unsafe because it performs an untyped write to the GS segment.
#[inline]
pub unsafe fn x86_write_gs_offset64(offset: u32, value: u64) {
    sys_x86_write_gs_offset64(offset, value)
}