// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Standard I/O functions
//!
//! This module provides C-compatible stdio functions.

#![no_std]

use super::c_int;
use core::ffi::c_char;
use core::fmt;

/// FILE structure (opaque)
#[repr(C)]
pub struct FILE {
    /// File descriptor
    fd: c_int,
    /// Buffer
    buffer: *mut u8,
    /// Buffer position
    pos: usize,
    /// Buffer size
    size: usize,
    /// Flags
    flags: u32,
}

/// Standard input
#[no_mangle]
pub static mut stdin: *mut FILE = core::ptr::null_mut();

/// Standard output
#[no_mangle]
pub static mut stdout: *mut FILE = core::ptr::null_mut();

/// Standard error
#[no_mangle]
pub static mut stderr: *mut FILE = core::ptr::null_mut();

// Internal write function
extern "C" {
    fn sys_write(fd: c_int, buf: *const u8, count: usize) -> isize;
}

/// Write to a file descriptor
#[no_mangle]
pub unsafe extern "C" fn write(fd: c_int, buf: *const u8, count: usize) -> isize {
    if buf.is_null() || count == 0 {
        return 0;
    }

    // TODO: Implement actual write syscall
    // For now, this is a stub
    count as isize
}

/// Read from a file descriptor
#[no_mangle]
pub unsafe extern "C" fn read(fd: c_int, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() || count == 0 {
        return 0;
    }

    // TODO: Implement actual read syscall
    // For now, this is a stub
    0
}

/// Write formatted output to stdout
#[no_mangle]
pub unsafe extern "C" fn printf(format: *const c_char, args: ...) -> c_int {
    if format.is_null() {
        return -1;
    }

    let mut buffer = [0u8; 1024];
    let written = vsnprintf(buffer.as_mut_ptr(), buffer.len(), format, args);

    if written > 0 {
        // Write to stdout (fd 1)
        write(1, buffer.as_ptr(), written as usize);
    }

    written as c_int
}

/// Write formatted output to a string
#[no_mangle]
pub unsafe extern "C" fn sprintf(
    s: *mut c_char,
    format: *const c_char,
    args: ...
) -> c_int {
    if s.is_null() || format.is_null() {
        return -1;
    }

    // Write to buffer (no size limit - unsafe!)
    vsnprintf(s as *mut u8, usize::MAX, format, args) as c_int
}

/// Write formatted output to a string with size limit
#[no_mangle]
pub unsafe extern "C" fn snprintf(
    s: *mut c_char,
    n: usize,
    format: *const c_char,
    args: ...
) -> c_int {
    if s.is_null() || format.is_null() || n == 0 {
        return -1;
    }

    vsnprintf(s as *mut u8, n, format, args) as c_int
}

/// Internal formatted output function
unsafe fn vsnprintf(
    mut s: *mut u8,
    n: usize,
    format: *const c_char,
    mut args: core::ffi::VaListImpl,
) -> isize {
    let mut written = 0usize;
    let mut i = 0usize;

    // Create a simple printf writer
    struct PrintfWriter {
        ptr: *mut u8,
        remaining: usize,
        written: usize,
    }

    // Implementation would continue with full format string parsing
    // For brevity, here's a simplified version

    while *format.add(i) != 0 && written < n.saturating_sub(1) {
        if *format.add(i) == b'%' as i8 {
            i += 1;

            if *format.add(i) == 0 {
                break;
            }

            match *format.add(i) as u8 as char {
                '%' => {
                    if written < n {
                        *s.add(written) = b'%';
                        written += 1;
                    }
                }
                'd' | 'i' => {
                    // Signed integer
                    let val = args.arg::<i32>();
                    let mut buffer = [0u8; 32];
                    let len = itoa(val, &mut buffer);
                    for &byte in &buffer[..len] {
                        if written < n.saturating_sub(1) {
                            *s.add(written) = byte;
                            written += 1;
                        }
                    }
                }
                'u' => {
                    // Unsigned integer
                    let val = args.arg::<u32>();
                    let mut buffer = [0u8; 32];
                    let len = utoa(val, &mut buffer);
                    for &byte in &buffer[..len] {
                        if written < n.saturating_sub(1) {
                            *s.add(written) = byte;
                            written += 1;
                        }
                    }
                }
                'x' => {
                    // Hexadecimal (lowercase)
                    let val = args.arg::<u32>();
                    let mut buffer = [0u8; 32];
                    let len = xtoh(val, &mut buffer, false);
                    for &byte in &buffer[..len] {
                        if written < n.saturating_sub(1) {
                            *s.add(written) = byte;
                            written += 1;
                        }
                    }
                }
                'X' => {
                    // Hexadecimal (uppercase)
                    let val = args.arg::<u32>();
                    let mut buffer = [0u8; 32];
                    let len = xtoh(val, &mut buffer, true);
                    for &byte in &buffer[..len] {
                        if written < n.saturating_sub(1) {
                            *s.add(written) = byte;
                            written += 1;
                        }
                    }
                }
                'p' => {
                    // Pointer
                    let val = args.arg::<usize>() as u64;
                    let mut buffer = [0u8; 32];
                    buffer[0] = b'0';
                    buffer[1] = b'x';
                    let len = xtoh64(val, &mut buffer[2..], false) + 2;
                    for &byte in &buffer[..len] {
                        if written < n.saturating_sub(1) {
                            *s.add(written) = byte;
                            written += 1;
                        }
                    }
                }
                's' => {
                    // String
                    let str_ptr = args.arg::<*const c_char>();
                    if !str_ptr.is_null() {
                        let mut j = 0usize;
                        while *str_ptr.add(j) != 0 && written < n.saturating_sub(1) {
                            *s.add(written) = *str_ptr.add(j) as u8;
                            written += 1;
                            j += 1;
                        }
                    } else {
                        // Print "(null)"
                        let null_str = b"(null)";
                        for &byte in null_str {
                            if written < n.saturating_sub(1) {
                                *s.add(written) = byte;
                                written += 1;
                            }
                        }
                    }
                }
                'c' => {
                    // Character
                    let val = args.arg::<i32>() as u8;
                    if written < n.saturating_sub(1) {
                        *s.add(written) = val;
                        written += 1;
                    }
                }
                _ => {
                    // Unknown format specifier, just print it
                    if written < n.saturating_sub(1) {
                        *s.add(written) = b'%';
                        written += 1;
                        if written < n.saturating_sub(1) {
                            *s.add(written) = *format.add(i) as u8;
                            written += 1;
                        }
                    }
                }
            }
        } else {
            if written < n.saturating_sub(1) {
                *s.add(written) = *format.add(i) as u8;
                written += 1;
            }
        }

        i += 1;
    }

    // Null-terminate
    if written < n {
        *s.add(written) = 0;
    } else if n > 0 {
        *s.add(n - 1) = 0;
    }

    written as isize
}

/// Convert signed integer to ASCII
fn itoa(mut val: i32, buffer: &mut [u8]) -> usize {
    let mut i = 0;

    if val < 0 {
        buffer[0] = b'-';
        val = -val;
        i = 1;
    }

    let mut len = i;
    if val == 0 {
        buffer[len] = b'0';
        len += 1;
    } else {
        let mut digits = [0u8; 10];
        let mut digit_count = 0;

        while val > 0 {
            digits[digit_count] = b'0' + ((val % 10) as u8);
            val /= 10;
            digit_count += 1;
        }

        for j in (0..digit_count).rev() {
            buffer[len] = digits[j];
            len += 1;
        }
    }

    len
}

/// Convert unsigned integer to ASCII
fn utoa(mut val: u32, buffer: &mut [u8]) -> usize {
    let mut len = 0;

    if val == 0 {
        buffer[len] = b'0';
        return 1;
    }

    let mut digits = [0u8; 10];
    let mut digit_count = 0;

    while val > 0 {
        digits[digit_count] = b'0' + ((val % 10) as u8);
        val /= 10;
        digit_count += 1;
    }

    for j in (0..digit_count).rev() {
        buffer[len] = digits[j];
        len += 1;
    }

    len
}

/// Convert to hexadecimal (32-bit)
fn xtoh(mut val: u32, buffer: &mut [u8], upper: bool) -> usize {
    let hex = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };

    if val == 0 {
        buffer[0] = b'0';
        return 1;
    }

    let mut len = 0;
    let mut started = false;

    for i in (0..8).rev() {
        let digit = ((val >> (i * 4)) & 0xF) as usize;

        if digit != 0 || started {
            buffer[len] = hex[digit];
            len += 1;
            started = true;
        }
    }

    len
}

/// Convert to hexadecimal (64-bit)
fn xtoh64(mut val: u64, buffer: &mut [u8], upper: bool) -> usize {
    let hex = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };

    if val == 0 {
        buffer[0] = b'0';
        return 1;
    }

    let mut len = 0;
    let mut started = false;

    for i in (0..16).rev() {
        let digit = ((val >> (i * 4)) & 0xF) as usize;

        if digit != 0 || started {
            buffer[len] = hex[digit];
            len += 1;
            started = true;
        }
    }

    len
}

/// Put a character to stdout
#[no_mangle]
pub unsafe extern "C" fn putchar(c: c_int) -> c_int {
    let ch = c as u8;
    write(1, &ch, 1);
    c
}

/// Put a string to stdout
#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    if s.is_null() {
        return -1;
    }

    let len = super::string::strlen(s);
    write(1, s as *const u8, len);
    write(1, b"\n".as_ptr(), 1);

    0
}

/// Flush a stream
#[no_mangle]
pub unsafe extern "C" fn fflush(stream: *mut FILE) -> c_int {
    // TODO: Implement buffering
    0
}
