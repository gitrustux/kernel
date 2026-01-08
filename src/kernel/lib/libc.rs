// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! C Library Support
//!
//! This module provides C library compatibility functions for the kernel.
//! These are stub implementations that satisfy C ABI requirements.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use crate::rustux::types::*;

/// C++ destructor function type
pub type DestructorFn = extern "C" fn(*mut u8);

/// Maximum number of atexit handlers
const MAX_ATEXIT_HANDLERS: usize = 64;

/// Atexit handler entry
struct AtexitHandler {
    destructor: DestructorFn,
    arg: *mut u8,
    dso_handle: *mut u8,
}

/// Global atexit handler table
static ATEXIT_HANDLERS: Mutex<[Option<AtexitHandler>; MAX_ATEXIT_HANDLERS]> =
    Mutex::new([None; MAX_ATEXIT_HANDLERS]);

/// Number of registered atexit handlers
static ATEXIT_COUNT: AtomicU32 = AtomicU32::new(0);

/// Register a function to be called on exit
///
/// # Arguments
///
/// * `destructor` - Function to call on exit
/// * `arg` - Argument to pass to the destructor
/// * `_dso_handle` - DSO handle (unused in kernel)
///
/// # Returns
///
/// 0 on success, non-zero on failure
///
/// # Safety
///
/// This function is called from C/C++ code and must maintain C ABI compatibility.
#[no_mangle]
pub extern "C" fn __cxa_atexit(
    destructor: Option<extern "C" fn(*mut u8)>,
    arg: *mut u8,
    _dso_handle: *mut u8,
) -> i32 {
    if destructor.is_none() {
        return -1;
    }

    let count = ATEXIT_COUNT.load(Ordering::Acquire) as usize;

    if count >= MAX_ATEXIT_HANDLERS {
        return -1;
    }

    let mut handlers = ATEXIT_HANDLERS.lock();
    handlers[count] = Some(AtexitHandler {
        destructor: destructor.unwrap(),
        arg,
        dso_handle: core::ptr::null_mut(),
    });

    ATEXIT_COUNT.store((count + 1) as u32, Ordering::Release);

    0
}

/// Call all registered atexit handlers
///
/// This is typically called during system shutdown.
pub fn __cxa_finalize() {
    let count = ATEXIT_COUNT.swap(0, Ordering::AcqRel) as usize;
    let handlers = ATEXIT_HANDLERS.lock();

    // Call handlers in reverse order (LIFO)
    for i in (0..count).rev() {
        if let Some(ref handler) = handlers[i] {
            (handler.destructor)(handler.arg);
        }
    }
}

/// Initialize libc support
pub fn init() {
    println!("libc: C library support initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn test_destructor(_arg: *mut u8) {
        // Test destructor
    }

    #[test]
    fn test_atexit_registration() {
        let result = __cxa_atexit(Some(test_destructor), core::ptr::null_mut(), core::ptr::null_mut());
        assert_eq!(result, 0);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_ATEXIT_HANDLERS, 64);
    }
}
