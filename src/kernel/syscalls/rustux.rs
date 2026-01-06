// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Zircon Compatibility System Calls
//!
//! This module implements Zircon-compatible system calls.
//!
//! # Syscalls Implemented
//!
//! - `rx_nanosleep` - Sleep for a duration
//! - `rx_clock_get` - Get clock time
//! - `rx_clock_get_new` - Get clock time (new version)
//! - `rx_clock_get_monotonic` - Get monotonic clock
//! - `rx_clock_adjust` - Adjust clock
//! - `rx_event_create` - Create event
//! - `rx_eventpair_create` - Create event pair
//! - `rx_debuglog_create` - Create debug log
//! - `rx_debuglog_write` - Write to debug log
//! - `rx_debuglog_read` - Read from debug log
//! - `rx_cprng_draw_once` - Draw random bytes
//! - `rx_cprng_add_entropy` - Add entropy to PRNG

#![no_std]

use crate::kernel::object::{Handle, HandleTable, ObjectType, Rights};
use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Constants
/// ============================================================================

/// Maximum CPRNG draw size
const MAX_CPRNG_DRAW: usize = 256;

/// Maximum CPRNG seed size
const MAX_CPRNG_SEED: usize = 256;

/// Maximum debug log record size
const DLOG_MAX_RECORD: usize = 256;

/// Maximum debug log data size
const DLOG_MAX_DATA: usize = 256;

/// ============================================================================
/// Clock Constants
/// ============================================================================

/// Clock IDs
pub mod clock_id {
    /// Monotonic clock
    pub const MONOTONIC: u32 = 0;

    /// UTC clock
    pub const UTC: u32 = 1;

    /// Thread clock
    pub const THREAD: u32 = 2;
}

/// ============================================================================
/// Debug Log Constants
/// ============================================================================

/// Debug log options
pub mod dlog_options {
    /// No options
    pub const NONE: u32 = 0;

    /// Log is readable
    pub const READABLE: u32 = 0x01;
}

/// Debug log flags mask
pub const DLOG_FLAGS_MASK: u32 = 0x01;

/// ============================================================================
/// UTC Offset
/// ============================================================================

/// UTC offset in nanoseconds
/// This is used by pvclock - if logic changes here, update pvclock too
static UTC_OFFSET: AtomicI64 = AtomicI64::new(0);

/// ============================================================================
/// PRNG State
/// ============================================================================

/// Simple XORShift PRNG state
static mut PRNG_STATE: u64 = 0x0123456789ABCDEF;

/// Initialize PRNG (should be called with true entropy)
fn prng_init(seed: u64) {
    unsafe {
        PRNG_STATE = seed;
    }
}

/// Draw random bytes from PRNG
fn prng_draw(buffer: &mut [u8]) {
    for chunk in buffer.chunks_mut(8) {
        unsafe {
            // XORShift64*
            let mut x = PRNG_STATE;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            PRNG_STATE = x;
            let rnd = x.wrapping_mul(0x2545F4914F6CDD1D);

            // Copy bytes to buffer
            let bytes = rnd.to_ne_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                if i < chunk.len() {
                    chunk[i] = b;
                }
            }
        }
    }
}

/// Add entropy to PRNG
fn prng_add_entropy(entropy: &[u8]) {
    let mut seed = unsafe { PRNG_STATE };
    for &byte in entropy.iter() {
        seed = seed.wrapping_mul(31).wrapping_add(byte as u64);
    }
    unsafe {
        PRNG_STATE = seed;
    }
}

/// ============================================================================
/// Syscall: Nanosleep
/// ============================================================================

/// Sleep for a duration
///
/// # Arguments
///
/// * `deadline` - Deadline in nanoseconds (absolute time)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_nanosleep_impl(deadline: i64) -> SyscallRet {
    log_debug!("sys_nanosleep: deadline={}", deadline);

    if deadline <= 0 {
        // Just yield
        // TODO: Implement thread_yield()
        log_debug!("sys_nanosleep: yielding");
        return ok_to_ret(0);
    }

    // TODO: Implement thread_sleep_interruptible
    // For now, just log
    log_info!("sys_nanosleep: sleeping until {} (stub)", deadline);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Clock Get
/// ============================================================================

/// Get clock time (legacy syscall)
///
/// # Arguments
///
/// * `clock_id` - Clock ID (MONOTONIC, UTC, THREAD)
///
/// # Returns
///
/// Clock time in nanoseconds
pub fn sys_clock_get_impl(clock_id: u32) -> SyscallRet {
    log_debug!("sys_clock_get: clock_id={}", clock_id);

    match clock_id {
        clock_id::MONOTONIC => {
            let time = crate::kernel::timer::current_time();
            ok_to_ret(time as usize)
        }

        clock_id::UTC => {
            let time = crate::kernel::timer::current_time() as i64 + UTC_OFFSET.load(Ordering::Relaxed);
            ok_to_ret(time as usize)
        }

        clock_id::THREAD => {
            // TODO: Get thread runtime
            // let time = ThreadDispatcher::GetCurrent()->runtime_ns();
            let time = 0i64;
            ok_to_ret(time as usize)
        }

        _ => {
            log_error!("sys_clock_get: invalid clock_id {}", clock_id);
            // TODO: figure out the best option here
            ok_to_ret(0)
        }
    }
}

/// Get clock time (new version)
///
/// # Arguments
///
/// * `clock_id` - Clock ID (MONOTONIC, UTC, THREAD)
/// * `time_out` - User pointer to store time
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_clock_get_new_impl(clock_id: u32, time_out: usize) -> SyscallRet {
    log_debug!("sys_clock_get_new: clock_id={}", clock_id);

    let time = match clock_id {
        clock_id::MONOTONIC => crate::kernel::timer::current_time() as i64,

        clock_id::UTC => {
            crate::kernel::timer::current_time() as i64 + UTC_OFFSET.load(Ordering::Relaxed)
        }

        clock_id::THREAD => {
            // TODO: Get thread runtime
            0i64
        }

        _ => {
            log_error!("sys_clock_get_new: invalid clock_id {}", clock_id);
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    // Write time to user
    let user_ptr = UserPtr::<u8>::new(time_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &time as *const i64 as *const u8, 8) {
            log_error!("sys_clock_get_new: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_clock_get_new: time={}", time);
    ok_to_ret(0)
}

/// Get monotonic clock time
///
/// # Returns
///
/// Monotonic clock time in nanoseconds
pub fn sys_clock_get_monotonic_impl() -> SyscallRet {
    let time = crate::kernel::timer::current_time();
    ok_to_ret(time as usize)
}

/// ============================================================================
/// Syscall: Clock Adjust
/// ============================================================================

/// Adjust clock
///
/// # Arguments
///
/// * `rsrc_handle` - Root resource handle
/// * `clock_id` - Clock ID
/// * `offset` - Offset in nanoseconds
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_clock_adjust_impl(
    rsrc_handle: u32,
    clock_id: u32,
    offset: i64,
) -> SyscallRet {
    log_debug!(
        "sys_clock_adjust: rsrc={:#x} clock_id={} offset={}",
        rsrc_handle,
        clock_id,
        offset
    );

    // TODO: Validate resource handle

    match clock_id {
        clock_id::MONOTONIC => {
            log_error!("sys_clock_adjust: cannot adjust monotonic clock");
            err_to_ret(RX_ERR_ACCESS_DENIED)
        }

        clock_id::UTC => {
            UTC_OFFSET.store(offset, Ordering::Release);
            log_info!("sys_clock_adjust: UTC offset set to {}", offset);
            ok_to_ret(0)
        }

        _ => {
            log_error!("sys_clock_adjust: invalid clock_id {}", clock_id);
            err_to_ret(RX_ERR_INVALID_ARGS)
        }
    }
}

/// ============================================================================
/// Syscall: Event Create
/// ============================================================================

/// Create an event
///
/// # Arguments
///
/// * `options` - Options (must be 0)
/// * `event_out` - User pointer to store event handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_event_create_impl(options: u32, event_out: usize) -> SyscallRet {
    log_debug!("sys_event_create: options={:#x}", options);

    if options != 0 {
        log_error!("sys_event_create: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Check job policy

    // Create event
    // TODO: Implement actual event creation
    let event_handle = 42u32; // Placeholder

    // Write handle to user
    let user_ptr = UserPtr::<u8>::new(event_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &event_handle as *const u32 as *const u8, 4) {
            log_error!("sys_event_create: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_event_create: success handle={:#x}", event_handle);
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Event Pair Create
/// ============================================================================

/// Create an event pair
///
/// # Arguments
///
/// * `options` - Options (must be 0)
/// * `out0` - User pointer to store first event handle
/// * `out1` - User pointer to store second event handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_eventpair_create_impl(
    options: u32,
    out0: usize,
    out1: usize,
) -> SyscallRet {
    log_debug!("sys_eventpair_create: options={:#x}", options);

    if options != 0 {
        log_error!("sys_eventpair_create: unsupported options");
        return err_to_ret(RX_ERR_NOT_SUPPORTED);
    }

    // TODO: Check job policy

    // Create event pair
    // TODO: Implement actual eventpair creation
    let handle0 = 100u32; // Placeholder
    let handle1 = 101u32; // Placeholder

    // Write handles to user
    let user_ptr0 = UserPtr::<u8>::new(out0);
    let user_ptr1 = UserPtr::<u8>::new(out1);

    unsafe {
        if let Err(err) = copy_to_user(user_ptr0, &handle0 as *const u32 as *const u8, 4) {
            log_error!("sys_eventpair_create: copy_to_user out0 failed: {:?}", err);
            return err_to_ret(err.into());
        }

        if let Err(err) = copy_to_user(user_ptr1, &handle1 as *const u32 as *const u8, 4) {
            log_error!("sys_eventpair_create: copy_to_user out1 failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!(
        "sys_eventpair_create: success handles={:#x}, {:#x}",
        handle0,
        handle1
    );
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Debug Log Create
/// ============================================================================

/// Create a debug log
///
/// # Arguments
///
/// * `rsrc` - Root resource handle (can be INVALID)
/// * `options` - Options
/// * `out` - User pointer to store log handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_debuglog_create_impl(
    rsrc: u32,
    options: u32,
    out: usize,
) -> SyscallRet {
    log_debug!("sys_debuglog_create: rsrc={:#x} options={:#x}", rsrc, options);

    // TODO: Validate resource handle (if not INVALID)

    // Create log dispatcher
    // TODO: Implement actual log creation
    let log_handle = 200u32; // Placeholder

    // By default log objects are write-only
    let mut rights = Rights::WRITE;
    if options & dlog_options::READABLE != 0 {
        rights |= Rights::READ;
    }

    // Write handle to user
    let user_ptr = UserPtr::<u8>::new(out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &log_handle as *const u32 as *const u8, 4) {
            log_error!("sys_debuglog_create: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_debuglog_create: success handle={:#x}", log_handle);
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Debug Log Write
/// ============================================================================

/// Write to debug log
///
/// # Arguments
///
/// * `log_handle` - Log handle
/// * `options` - Options
/// * `ptr` - User pointer to buffer
/// * `len` - Length of buffer
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_debuglog_write_impl(
    log_handle: u32,
    options: u32,
    ptr: usize,
    len: usize,
) -> SyscallRet {
    log_debug!(
        "sys_debuglog_write: log={:#x} options={:#x} len={}",
        log_handle,
        options,
        len
    );

    if len > DLOG_MAX_DATA {
        log_error!("sys_debuglog_write: len too large");
        return err_to_ret(RX_ERR_OUT_OF_RANGE);
    }

    if options & !DLOG_FLAGS_MASK != 0 {
        log_error!("sys_debuglog_write: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Copy data from user
    let mut buf = alloc::vec![0u8; len];
    let user_ptr = UserPtr::<u8>::new(ptr);

    unsafe {
        if let Err(err) = copy_from_user(buf.as_mut_ptr(), user_ptr, len) {
            log_error!("sys_debuglog_write: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Convert to string and log
    let str_result = alloc::str::from_utf8(&buf);
    match str_result {
        Ok(s) => {
            log_info!("LOG: {}", s);
        }
        Err(_) => {
            log_info!("LOG: {:x?}", buf);
        }
    }

    // TODO: Implement actual log write
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Debug Log Read
/// ============================================================================

/// Read from debug log
///
/// # Arguments
///
/// * `log_handle` - Log handle
/// * `options` - Options
/// * `ptr` - User pointer to buffer
/// * `len` - Length of buffer
///
/// # Returns
///
/// * On success: Number of bytes read
/// * On error: Negative error code
pub fn sys_debuglog_read_impl(
    log_handle: u32,
    options: u32,
    ptr: usize,
    len: usize,
) -> SyscallRet {
    log_debug!(
        "sys_debuglog_read: log={:#x} options={:#x} len={}",
        log_handle,
        options,
        len
    );

    if options != 0 {
        log_error!("sys_debuglog_read: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Implement actual log read
    // For now, return 0 bytes read
    log_debug!("sys_debuglog_read: success, read 0 bytes");
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: CPRNG Draw
/// ============================================================================

/// Draw random bytes from CPRNG
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
pub fn sys_cprng_draw_once_impl(buffer: usize, len: usize) -> SyscallRet {
    log_debug!("sys_cprng_draw_once: len={}", len);

    if len > MAX_CPRNG_DRAW {
        log_error!("sys_cprng_draw_once: len too large");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Draw random bytes
    let mut kernel_buf = alloc::vec![0u8; len];
    prng_draw(&mut kernel_buf);

    // Copy to user
    let user_ptr = UserPtr::<u8>::new(buffer);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, kernel_buf.as_ptr(), len) {
            log_error!("sys_cprng_draw_once: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Zero the kernel copy (for security)
    kernel_buf.fill(0);

    log_debug!("sys_cprng_draw_once: success, drew {} bytes", len);
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: CPRNG Add Entropy
/// ============================================================================

/// Add entropy to CPRNG
///
/// # Arguments
///
/// * `buffer` - User pointer to entropy buffer
/// * `buffer_size` - Size of entropy buffer
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_cprng_add_entropy_impl(buffer: usize, buffer_size: usize) -> SyscallRet {
    log_debug!("sys_cprng_add_entropy: buffer_size={}", buffer_size);

    if buffer_size > MAX_CPRNG_SEED {
        log_error!("sys_cprng_add_entropy: buffer_size too large");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Copy entropy from user
    let mut kernel_buf = alloc::vec![0u8; buffer_size];
    let user_ptr = UserPtr::<u8>::new(buffer);

    unsafe {
        if let Err(err) = copy_from_user(kernel_buf.as_mut_ptr(), user_ptr, buffer_size) {
            log_error!("sys_cprng_add_entropy: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Add entropy to PRNG
    prng_add_entropy(&kernel_buf);

    // Zero the kernel copy (for security)
    kernel_buf.fill(0);

    log_debug!("sys_cprng_add_entropy: success, added {} bytes", buffer_size);
    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get Zircon compatibility statistics
pub fn get_stats() -> ZirconStats {
    ZirconStats {
        total_nanosleep: 0,    // TODO: Track
        total_clock_get: 0,    // TODO: Track
        total_prng_draw: 0,    // TODO: Track
        total_prng_entropy: 0, // TODO: Track
        utc_offset: UTC_OFFSET.load(Ordering::Relaxed),
    }
}

/// Zircon compatibility statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ZirconStats {
    /// Total nanosleep operations
    pub total_nanosleep: u64,

    /// Total clock get operations
    pub total_clock_get: u64,

    /// Total PRNG draw operations
    pub total_prng_draw: u64,

    /// Total PRNG entropy additions
    pub total_prng_entropy: u64,

    /// Current UTC offset
    pub utc_offset: i64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the Zircon compatibility subsystem
pub fn init() {
    log_info!("Zircon compatibility subsystem initialized");
    log_info!("  Max CPRNG draw: {}", MAX_CPRNG_DRAW);
    log_info!("  Max CPRNG seed: {}", MAX_CPRNG_SEED);

    // Initialize PRNG with simple seed
    // TODO: Use true entropy from hardware
    prng_init(0x0123456789ABCDEF);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_id_consts() {
        assert_eq!(clock_id::MONOTONIC, 0);
        assert_eq!(clock_id::UTC, 1);
        assert_eq!(clock_id::THREAD, 2);
    }

    #[test]
    fn test_dlog_options_consts() {
        assert_eq!(dlog_options::NONE, 0);
        assert_eq!(dlog_options::READABLE, 0x01);
    }

    #[test]
    fn test_utc_offset() {
        UTC_OFFSET.store(1000, Ordering::Release);
        assert_eq!(UTC_OFFSET.load(Ordering::Acquire), 1000);
    }

    #[test]
    fn test_prng_draw() {
        let mut buf = [0u8; 16];
        prng_draw(&mut buf);
        // Should have some non-zero values
        let has_nonzero = buf.iter().any(|&b| b != 0);
        assert!(has_nonzero);
    }

    #[test]
    fn test_prng_add_entropy() {
        let entropy = [1u8, 2, 3, 4, 5];
        let state_before = unsafe { PRNG_STATE };
        prng_add_entropy(&entropy);
        let state_after = unsafe { PRNG_STATE };
        assert_ne!(state_before, state_after);
    }

    #[test]
    fn test_nanosleep_non_positive() {
        let result = sys_nanosleep_impl(0);
        assert!(result >= 0);

        let result = sys_nanosleep_impl(-1);
        assert!(result >= 0);
    }
}
