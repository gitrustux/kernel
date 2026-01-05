// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Standard library functions
//!
//! This module provides general purpose functions including
//! memory allocation, string conversion, and process control.

#![no_std]

use super::{c_int, c_uint, size_t};
use core::ptr;

/// Integer division result
#[repr(C)]
pub struct div_t {
    pub quot: c_int,
    pub rem: c_int,
}

/// Long integer division result
#[repr(C)]
pub struct ldiv_t {
    pub quot: c_long,
    pub rem: c_long,
}

pub type c_long = i64;
pub type c_ulong = u64;

/// Allocate memory
///
/// TODO: Implement proper memory allocator
/// For now, this is a stub that always returns null
#[no_mangle]
pub extern "C" fn malloc(size: size_t) -> *mut u8 {
    if size == 0 {
        return ptr::null_mut();
    }

    // TODO: Implement malloc using syscalls to allocate VMOs
    // For now, return null
    ptr::null_mut()
}

/// Allocate zero-initialized memory
#[no_mangle]
pub extern "C" fn calloc(nmemb: size_t, size: size_t) -> *mut u8 {
    if nmemb == 0 || size == 0 {
        return ptr::null_mut();
    }

    let total = nmemb.saturating_mul(size);
    let ptr = malloc(total);

    if !ptr.is_null() {
        unsafe {
            // Zero the memory
            let mut i = 0;
            while i < total {
                *ptr.add(i) = 0;
                i += 1;
            }
        }
    }

    ptr
}

/// Reallocate memory
#[no_mangle]
pub extern "C" fn realloc(ptr: *mut u8, size: size_t) -> *mut u8 {
    if ptr.is_null() {
        return malloc(size);
    }

    if size == 0 {
        free(ptr);
        return ptr::null_mut();
    }

    // TODO: Implement proper realloc
    // For now, return null
    ptr::null_mut()
}

/// Free memory
#[no_mangle]
pub extern "C" fn free(ptr: *mut u8) {
    // TODO: Implement free
    let _ = ptr;
}

/// Convert string to integer
#[no_mangle]
pub unsafe extern "C" fn atoi(s: *const i8) -> c_int {
    if s.is_null() {
        return 0;
    }

    strtol(s, ptr::null_mut(), 10) as c_int
}

/// Convert string to long integer
#[no_mangle]
pub unsafe extern "C" fn atol(s: *const i8) -> c_long {
    if s.is_null() {
        return 0;
    }

    strtol(s, ptr::null_mut(), 10)
}

/// Convert string to long long integer
#[no_mangle]
pub unsafe extern "C" fn atoll(s: *const i8) -> i64 {
    if s.is_null() {
        return 0;
    }

    strtoll(s, ptr::null_mut(), 10)
}

/// Convert string to unsigned long integer
#[no_mangle]
pub unsafe extern "C" fn strtoul(
    s: *const i8,
    endptr: *mut *mut i8,
    base: c_int,
) -> c_ulong {
    if s.is_null() {
        return 0;
    }

    // Skip leading whitespace
    let mut i = 0;
    while *s.add(i) == b' ' as i8 || *s.add(i) == b'\t' as i8 || *s.add(i) == b'\n' as i8 {
        i += 1;
    }

    // Handle sign
    let mut neg = false;
    if *s.add(i) == b'-' as i8 {
        neg = true;
        i += 1;
    } else if *s.add(i) == b'+' as i8 {
        i += 1;
    }

    // Detect base
    let mut detected_base = base;
    if detected_base == 0 {
        if *s.add(i) == b'0' as i8 {
            if *s.add(i + 1) == b'x' as i8 || *s.add(i + 1) == b'X' as i8 {
                detected_base = 16;
                i += 2;
            } else {
                detected_base = 8;
                i += 1;
            }
        } else {
            detected_base = 10;
        }
    }

    // Convert digits
    let mut result: c_ulong = 0;
    while *s.add(i) != 0 {
        let c = *s.add(i);
        let digit = if c >= b'0' as i8 && c <= b'9' as i8 {
            (c - b'0' as i8) as c_ulong
        } else if c >= b'a' as i8 && c <= b'f' as i8 {
            (c - b'a' as i8 + 10) as c_ulong
        } else if c >= b'A' as i8 && c <= b'F' as i8 {
            (c - b'A' as i8 + 10) as c_ulong
        } else {
            break;
        };

        if digit >= detected_base as c_ulong {
            break;
        }

        result = result.saturating_mul(detected_base as c_ulong);
        result = result.saturating_add(digit);

        i += 1;
    }

    // Update endptr
    if !endptr.is_null() {
        *endptr = s.add(i) as *mut i8;
    }

    if neg {
        (result as i64).wrapping_neg() as c_ulong
    } else {
        result
    }
}

/// Convert string to signed long integer
#[no_mangle]
pub unsafe extern "C" fn strtol(
    s: *const i8,
    endptr: *mut *mut i8,
    base: c_int,
) -> c_long {
    if s.is_null() {
        return 0;
    }

    // Skip leading whitespace
    let mut i = 0;
    while *s.add(i) == b' ' as i8 || *s.add(i) == b'\t' as i8 || *s.add(i) == b'\n' as i8 {
        i += 1;
    }

    // Handle sign
    let mut neg = false;
    if *s.add(i) == b'-' as i8 {
        neg = true;
        i += 1;
    } else if *s.add(i) == b'+' as i8 {
        i += 1;
    }

    // Detect base
    let mut detected_base = base;
    if detected_base == 0 {
        if *s.add(i) == b'0' as i8 {
            if *s.add(i + 1) == b'x' as i8 || *s.add(i + 1) == b'X' as i8 {
                detected_base = 16;
                i += 2;
            } else {
                detected_base = 8;
                i += 1;
            }
        } else {
            detected_base = 10;
        }
    }

    // Convert digits
    let mut result: c_long = 0;
    while *s.add(i) != 0 {
        let c = *s.add(i);
        let digit = if c >= b'0' as i8 && c <= b'9' as i8 {
            (c - b'0' as i8) as c_long
        } else if c >= b'a' as i8 && c <= b'f' as i8 {
            (c - b'a' as i8 + 10) as c_long
        } else if c >= b'A' as i8 && c <= b'F' as i8 {
            (c - b'A' as i8 + 10) as c_long
        } else {
            break;
        };

        if digit >= detected_base as c_long {
            break;
        }

        result = result.saturating_mul(detected_base as c_long);
        result = result.saturating_add(digit);

        i += 1;
    }

    // Update endptr
    if !endptr.is_null() {
        *endptr = s.add(i) as *mut i8;
    }

    if neg {
        result.wrapping_neg()
    } else {
        result
    }
}

/// Convert string to signed long long integer
#[no_mangle]
pub unsafe extern "C" fn strtoll(
    s: *const i8,
    endptr: *mut *mut i8,
    base: c_int,
) -> i64 {
    strtol(s, endptr, base) as i64
}

/// Convert integer to string
#[no_mangle]
pub unsafe extern "C" fn itoa(value: c_int, str: *mut i8, radix: c_int) -> *mut i8 {
    if str.is_null() || radix < 2 || radix > 36 {
        return ptr::null_mut();
    }

    let mut i = 0;
    let mut val = value;

    // Handle negative numbers
    if val < 0 && radix == 10 {
        *str.add(i) = b'-' as i8;
        i += 1;
        val = -val;
    }

    // Convert digits in reverse
    let mut start = i;
    if val == 0 {
        *str.add(i) = b'0' as i8;
        i += 1;
    } else {
        while val > 0 {
            let digit = (val % radix) as u8;
            *str.add(i) = if digit < 10 {
                b'0' + digit
            } else {
                b'a' + digit - 10
            } as i8;
            val /= radix;
            i += 1;
        }
    }

    // Reverse the string
    let mut end = i - 1;
    while start < end {
        let tmp = *str.add(start);
        *str.add(start) = *str.add(end);
        *str.add(end) = tmp;
        start += 1;
        end -= 1;
    }

    // Null-terminate
    *str.add(i) = 0;

    str
}

/// Convert unsigned integer to string
#[no_mangle]
pub unsafe extern "C" fn utoa(value: c_uint, str: *mut i8, radix: c_int) -> *mut i8 {
    if str.is_null() || radix < 2 || radix > 36 {
        return ptr::null_mut();
    }

    let mut i = 0;
    let mut val = value;

    // Convert digits in reverse
    let mut start = i;
    if val == 0 {
        *str.add(i) = b'0' as i8;
        i += 1;
    } else {
        while val > 0 {
            let digit = (val % radix as u32) as u8;
            *str.add(i) = if digit < 10 {
                b'0' + digit
            } else {
                b'a' + digit - 10
            } as i8;
            val /= radix as u32;
            i += 1;
        }
    }

    // Reverse the string
    let mut end = i - 1;
    while start < end {
        let tmp = *str.add(start);
        *str.add(start) = *str.add(end);
        *str.add(end) = tmp;
        start += 1;
        end -= 1;
    }

    // Null-terminate
    *str.add(i) = 0;

    str
}

/// Perform integer division
#[no_mangle]
pub extern "C" fn div(numer: c_int, denom: c_int) -> div_t {
    div_t {
        quot: numer / denom,
        rem: numer % denom,
    }
}

/// Perform long integer division
#[no_mangle]
pub extern "C" fn ldiv(numer: c_long, denom: c_long) -> ldiv_t {
    ldiv_t {
        quot: numer / denom,
        rem: numer % denom,
    }
}

/// Get absolute value
#[no_mangle]
pub extern "C" fn abs(x: c_int) -> c_int {
    if x < 0 { -x } else { x }
}

/// Get long absolute value
#[no_mangle]
pub extern "C" fn labs(x: c_long) -> c_long {
    if x < 0 { -x } else { x }
}

/// Generate random number
#[no_mangle]
pub extern "C" fn rand() -> c_int {
    // TODO: Implement proper random number generator
    // For now, use a simple linear congruential generator
    static mut SEED: u32 = 1;

    unsafe {
        SEED = SEED.wrapping_mul(1103515245).wrapping_add(12345);
        ((SEED >> 16) & 0x7FFF) as c_int
    }
}

/// Seed random number generator
#[no_mangle]
pub extern "C" fn srand(seed: c_uint) {
    static mut RAND_SEED: u32 = 1;

    unsafe {
        if seed != 0 {
            RAND_SEED = seed;
        } else {
            RAND_SEED = 1;
        }
    }
}

/// Abort the process
#[no_mangle]
pub extern "C" fn abort() -> ! {
    super::exit(134);
}
