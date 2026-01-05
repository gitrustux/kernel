// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux C Library (libc-rx)
//!
//! This is a minimal C-compatible standard library implementation for Rustux.
//! It provides the commonly used C functions for compatibility with existing code.
//!
//! # Components
//!
//! - **string** - String manipulation functions (memcpy, strlen, etc.)
//! - **stdio** - Standard I/O functions (printf, FILE*, etc.)
//! - **stdlib** - Standard library functions (malloc, free, atoi, etc.)
//! - **unistd** - POSIX-standard functions (read, write, etc.)

#![no_std]
#![feature(c_variadic)]
#![feature(ffi_asm)]

pub mod string;
pub mod stdio;
pub mod stdlib;
pub mod unistd;

// Re-export commonly used C types
pub use stdio::{FILE, stdin, stdout, stderr};

// C-compatible types
#[repr(C)]
pub struct c_int;
pub type c_void = core::ffi::c_void;
pub type c_char = i8;
pub type c_schar = i8;
pub type c_uchar = u8;
pub type c_short = i16;
pub type c_ushort = u16;
pub type c_int = i32;
pub type c_uint = u32;
pub type c_long = i64;
pub type c_ulong = u64;
pub type c_longlong = i64;
pub type c_ulonglong = u64;
pub type c_float = f32;
pub type c_double = f64;
pub type size_t = usize;
pub type ssize_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type ptrdiff_t = isize;
pub type clock_t = u64;
pub type time_t = i64;
pub type suseconds_t = i64;

/// NULL pointer constant
pub const NULL: *mut c_void = 0 as *mut c_void;

/// EOF constant
pub const EOF: c_int = -1;

// FFI exports for C compatibility

/// Get the last error number
#[no_mangle]
pub extern "C" fn __errno() -> *mut c_int {
    // TODO: Implement thread-local errno
    static mut ERRNO: c_int = 0;
    unsafe { core::ptr::addr_of_mut!(ERRNO) }
}

/// Exit the process
#[no_mangle]
pub extern "C" fn exit(code: c_int) -> ! {
    // TODO: Call libc-rx cleanup
    unsafe {
        libsys::Process::exit(code);
    }
}

/// Abort the process
#[no_mangle]
pub extern "C" fn abort() -> ! {
    // TODO: Dump core or something
    exit(134);
}
