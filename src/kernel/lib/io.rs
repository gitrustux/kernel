// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Console I/O
//!
//! This module provides console and serial output functions for the kernel.
//! It supports callback registration for multiple output targets.

#![no_std]

extern crate alloc;

use alloc::collections::LinkedList;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Line buffer length for thread-local buffering
pub const THREAD_LINEBUFFER_LENGTH: usize = 256;

/// Print callback structure
pub struct PrintCallback {
    /// Print function
    pub print: Option<extern "C" fn(&PrintCallback, &str, usize)>,
    /// User data
    pub user_data: usize,
}

impl PrintCallback {
    /// Create a new print callback
    pub fn new(
        print: Option<extern "C" fn(&PrintCallback, &str, usize)>,
        user_data: usize,
    ) -> Self {
        Self { print, user_data }
    }
}

/// Serial output spin lock
static SERIAL_SPIN_LOCK: Mutex<()> = Mutex::new(());

/// Print callbacks spin lock
static PRINT_SPIN_LOCK: Mutex<()> = Mutex::new(());

/// List of registered print callbacks
static PRINT_CALLBACKS: Mutex<LinkedList<Arc<PrintCallback>>> = Mutex::new(LinkedList::new());

/// Write to the serial port
///
/// # Arguments
///
/// * `str` - String to write
/// * `len` - Length of string
///
/// # Safety
///
/// This function is called from C code and must maintain C ABI compatibility.
pub fn kernel_serial_write(str: &str, len: usize) {
    let _lock = SERIAL_SPIN_LOCK.lock();

    // Write out the serial port
    platform_dputs_irq(str, len);
}

/// Write to the kernel console
///
/// # Arguments
///
/// * `str` - String to write
/// * `len` - Length of string
///
/// This writes to all registered print callbacks (loggers).
pub fn kernel_console_write(str: &str, len: usize) {
    let callbacks = PRINT_CALLBACKS.lock();

    for cb in callbacks.iter() {
        if let Some(print_fn) = cb.print {
            print_fn(cb, str, len);
        }
    }
}

/// Write to standard output (both console and serial)
///
/// # Arguments
///
/// * `str` - String to write
/// * `len` - Length of string
pub fn kernel_stdout_write(str: &str, len: usize) {
    // Try to write to debug log first
    if !dlog_bypass() {
        if dlog_write(0, str, len) {
            return;
        }
    }

    // Fall back to console and serial
    kernel_console_write(str, len);
    kernel_serial_write(str, len);
}

/// Register a print callback
///
/// # Arguments
///
/// * `cb` - Print callback to register
pub fn register_print_callback(cb: Arc<PrintCallback>) {
    let _lock = PRINT_SPIN_LOCK.lock();
    let mut callbacks = PRINT_CALLBACKS.lock();
    callbacks.push_back(cb);
}

/// Unregister a print callback
///
/// # Arguments
///
/// * `cb` - Print callback to unregister
pub fn unregister_print_callback(_cb: &PrintCallback) {
    let _lock = PRINT_SPIN_LOCK.lock();
    // TODO: Implement callback removal
    // For now, this is a stub since our LinkedList doesn't support removal
}

/// Printf output function
///
/// This is the standard output function used by printf-style formatting.
///
/// # Arguments
///
/// * `s` - String to write
/// * `len` - Length of string
/// * `_state` - User state (unused)
///
/// # Returns
///
/// Number of characters written
pub fn printf_output_func(s: &str, len: usize, _state: Option<*mut u8>) -> i32 {
    kernel_stdout_write(s, len);
    len as i32
}

/// Platform-specific serial output
///
/// # Arguments
///
/// * `str` - String to write
/// * `len` - Length of string
fn platform_dputs_irq(str: &str, len: usize) {
    // TODO: Implement platform-specific serial output
    let _ = (str, len);
    // For now, this is a stub
    // In a real implementation, this would write to the platform's UART
}

/// Check if debug log bypass is enabled
fn dlog_bypass() -> bool {
    // TODO: Implement debug log bypass check
    false
}

/// Write to debug log
///
/// # Arguments
///
/// * `_flags` - Flags (unused)
/// * `str` - String to write
/// * `len` - Length of string
///
/// # Returns
///
/// true on success, false on failure
fn dlog_write(_flags: u32, str: &str, len: usize) -> bool {
    // TODO: Implement debug log write
    let _ = (str, len);
    // For now, this is a stub
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(THREAD_LINEBUFFER_LENGTH, 256);
    }

    #[test]
    fn test_print_callback() {
        extern "C" fn test_callback(_cb: &PrintCallback, _str: &str, _len: usize) {
            // Test callback
        }

        let cb = PrintCallback::new(Some(test_callback), 0);
        assert!(cb.print.is_some());
        assert_eq!(cb.user_data, 0);
    }

    #[test]
    fn test_register_callback() {
        extern "C" fn test_callback(_cb: &PrintCallback, _str: &str, _len: usize) {}

        let cb = Arc::new(PrintCallback::new(Some(test_callback), 0));
        register_print_callback(cb);

        let callbacks = PRINT_CALLBACKS.lock();
        assert_eq!(callbacks.len(), 1);
    }

    #[test]
    fn test_printf_output() {
        let result = printf_output_func("test", 4, None);
        assert_eq!(result, 4);
    }
}
