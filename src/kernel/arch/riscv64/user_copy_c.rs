// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V user memory copy functions
//!
//! Provides safe functions for copying memory to/from user space,
//! with proper exception handling for page faults.

#![no_std]

use crate::debug;
use crate::rustux::types::*;

/// Error codes for user copy operations
pub const USER_COPY_OK: i32 = 0;
pub const USER_COPY_FAULT: i32 = -1;

/// Copy data from user space to kernel space
///
/// # Arguments
///
/// * `dst` - Kernel destination address
/// * `src` - User source address
/// * `len` - Number of bytes to copy
///
/// # Returns
///
/// Number of bytes copied, or negative error code on fault
///
/// # Safety
///
/// dst must point to valid kernel memory, src must be a user address
pub unsafe fn riscv_copy_from_user(dst: *mut u8, src: VAddr, len: usize) -> isize {
    // Use assembly implementation with exception handling
    extern "C" {
        fn riscv_copy_from_user_impl(dst: *mut u8, src: VAddr, len: usize) -> isize;
    }

    riscv_copy_from_user_impl(dst, src, len)
}

/// Copy data from kernel space to user space
///
/// # Arguments
///
/// * `dst` - User destination address
/// * `src` - Kernel source address
/// * `len` - Number of bytes to copy
///
/// # Returns
///
/// Number of bytes copied, or negative error code on fault
///
/// # Safety
///
/// dst must be a user address, src must point to valid kernel memory
pub unsafe fn riscv_copy_to_user(dst: VAddr, src: *const u8, len: usize) -> isize {
    extern "C" {
        fn riscv_copy_to_user_impl(dst: VAddr, src: *const u8, len: usize) -> isize;
    }

    riscv_copy_to_user_impl(dst, src, len)
}

/// Copy a string from user space to kernel space
///
/// Copies a null-terminated string from user space, with a maximum length.
///
/// # Arguments
///
/// * `dst` - Kernel destination buffer
/// * `src` - User source string
/// * `max_len` - Maximum length to copy (including null terminator)
///
/// # Returns
///
/// Number of bytes copied (including null terminator), or negative error code
///
/// # Safety
///
/// dst must point to valid kernel memory of at least max_len bytes,
/// src must be a user address
pub unsafe fn riscv_copy_string_from_user(
    dst: *mut u8,
    src: VAddr,
    max_len: usize,
) -> isize {
    let mut copied = 0;

    while copied < max_len {
        let mut byte: u8 = 0;
        let result = riscv_copy_from_user(
            &mut byte as *mut u8,
            src + copied,
            1,
        );

        if result < 0 {
            // Page fault
            return result;
        }

        // Copy the byte to destination
        dst.add(copied).write_volatile(byte);

        copied += 1;

        // Check for null terminator
        if byte == 0 {
            break;
        }
    }

    copied
}

/// Zero a range of user memory
///
/// # Arguments
///
/// * `dst` - User destination address
/// * `len` - Number of bytes to zero
///
/// # Returns
///
/// Number of bytes zeroed, or negative error code on fault
///
/// # Safety
///
/// dst must be a user address
pub unsafe fn riscv_zero_user_memory(dst: VAddr, len: usize) -> isize {
    extern "C" {
        fn riscv_zero_user_memory_impl(dst: VAddr, len: usize) -> isize;
    }

    riscv_zero_user_memory_impl(dst, len)
}

/// Verify that a user address range is accessible
///
/// # Arguments
///
/// * `addr` - Start of user address range
/// * `len` - Length of range
/// * `write` - true if checking for write access
///
/// # Returns
///
/// true if the range is accessible, false otherwise
pub unsafe fn riscv_user_access_verify(addr: VAddr, len: usize, write: bool) -> bool {
    // Simple check: verify address is in user range
    const USER_MAX: VAddr = 0x0000_0000_FFFF_FFFF;

    if addr > USER_MAX {
        return false;
    }

    if addr.wrapping_add(len) > USER_MAX {
        // Overflow or out of range
        return false;
    }

    // TODO: Check page table entries for actual access
    // For now, just check the address range
    true
}

/// Copy data between user addresses
///
/// # Arguments
///
/// * `dst` - User destination address
/// * `src` - User source address
/// * `len` - Number of bytes to copy
///
/// # Returns
///
/// Number of bytes copied, or negative error code on fault
///
/// # Safety
///
/// Both dst and src must be user addresses
pub unsafe fn riscv_copy_user_to_user(dst: VAddr, src: VAddr, len: usize) -> isize {
    // Use a temporary kernel buffer
    const TMP_BUF_SIZE: usize = 256;
    let mut tmp_buf = [0u8; TMP_BUF_SIZE];

    let mut copied = 0;

    while copied < len {
        let chunk_size = core::cmp::min(TMP_BUF_SIZE, len - copied);

        // Copy from user to kernel buffer
        let result = riscv_copy_from_user(
            tmp_buf.as_mut_ptr(),
            src + copied,
            chunk_size,
        );

        if result < 0 {
            return result;
        }

        let actual = result as usize;

        // Copy from kernel buffer to user
        let result2 = riscv_copy_to_user(
            dst + copied,
            tmp_buf.as_ptr(),
            actual,
        );

        if result2 < 0 {
            return result2;
        }

        copied += actual;

        if actual < chunk_size {
            // Partial copy, likely hit end of valid page
            break;
        }
    }

    copied as isize
}

/// Check if an address is a user address
///
/// # Arguments
///
/// * `addr` - Address to check
///
/// # Returns
///
/// true if the address is in user space
#[inline]
pub fn riscv_is_user_address(addr: VAddr) -> bool {
    addr < (1usize << 38) // User addresses are in lower 38 bits
}

/// Page fault handler for user copy operations
///
/// This is called from the assembly implementation when a page fault occurs.
///
/// # Arguments
///
/// * `addr` - The faulting address
/// * `write` - true if it was a write fault
///
/// # Safety
///
/// Must only be called from exception handler context
pub unsafe extern "C" fn riscv_user_copy_fault_handler(addr: VAddr, write: bool) {
    println!(
        "User copy fault: {} {:#x}",
        if write { "write to" } else { "read from" },
        addr
    );

    // TODO: In a full implementation, we would:
    // 1. Check if this is a valid user address with a valid VMA
    // 2. If so, handle the page fault and resume
    // 3. If not, deliver SIGSEGV to the process

    // For now, just halt (this will be improved)
    // debug::panic!("User copy fault unimplemented");
}

/// User copy context for exception handling
///
/// This structure is used to track state across page faults
/// during user copy operations.
#[repr(C)]
pub struct UserCopyContext {
    pub dst: VAddr,
    pub src: VAddr,
    pub remaining: usize,
    pub completed: usize,
    pub is_write: bool,
    pub fault_addr: VAddr,
}

impl UserCopyContext {
    pub const fn new() -> Self {
        Self {
            dst: 0,
            src: 0,
            remaining: 0,
            completed: 0,
            is_write: false,
            fault_addr: 0,
        }
    }
}

/// Get/set user copy context for current thread
///
/// This allows the exception handler to communicate with
/// the user copy implementation.
///
/// # Arguments
///
/// * `ctx` - Optional pointer to context
///
/// # Returns
///
/// Previous context pointer (if any)
///
/// # Safety
///
/// Must be called with proper synchronization
pub unsafe fn riscv_set_user_copy_context(ctx: Option<&mut UserCopyContext>) -> Option<*mut UserCopyContext> {
    // TODO: Store in per-thread data
    // For now, return None (no context)
    None
}

/// Assert that UserCopyContext is the expected size
const _: () = assert!(core::mem::size_of::<UserCopyContext>() == 40);
