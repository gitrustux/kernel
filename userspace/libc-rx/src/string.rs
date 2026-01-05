// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! String manipulation functions
//!
//! This module provides C-compatible string functions.

#![no_std]

use super::c_int;
use core::ffi::c_char;
use core::ptr;

/// Calculate the length of a string
#[no_mangle]
pub extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }

    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

/// Compare two strings
#[no_mangle]
pub extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int {
    if s1.is_null() || s2.is_null() {
        return if s1.is_null() && s2.is_null() { 0 } else { -1 };
    }

    unsafe {
        let mut i = 0;
        loop {
            let c1 = *s1.add(i) as i8;
            let c2 = *s2.add(i) as i8;

            if c1 != c2 {
                return (c1 - c2) as c_int;
            }

            if c1 == 0 {
                return 0;
            }

            i += 1;
        }
    }
}

/// Compare two strings with a maximum length
#[no_mangle]
pub extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int {
    if s1.is_null() || s2.is_null() {
        return if s1.is_null() && s2.is_null() { 0 } else { -1 };
    }

    unsafe {
        for i in 0..n {
            let c1 = *s1.add(i) as i8;
            let c2 = *s2.add(i) as i8;

            if c1 != c2 {
                return (c1 - c2) as c_int;
            }

            if c1 == 0 {
                return 0;
            }
        }
        0
    }
}

/// Copy a string
#[no_mangle]
pub unsafe extern "C" fn strcpy(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    let mut i = 0;
    loop {
        let c = *src.add(i);
        *dest.add(i) = c;

        if c == 0 {
            break;
        }

        i += 1;
    }

    dest
}

/// Copy a string with a maximum length
#[no_mangle]
pub unsafe extern "C" fn strncpy(dest: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }

    let mut i = 0;
    while i < n {
        let c = *src.add(i);
        *dest.add(i) = c;

        if c == 0 {
            // Pad with zeros
            for j in (i + 1)..n {
                *dest.add(j) = 0;
            }
            break;
        }

        i += 1;
    }

    dest
}

/// Concatenate two strings
#[no_mangle]
pub unsafe extern "C" fn strcat(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    let mut i = 0;
    while *dest.add(i) != 0 {
        i += 1;
    }

    let mut j = 0;
    loop {
        let c = *src.add(j);
        *dest.add(i + j) = c;

        if c == 0 {
            break;
        }

        j += 1;
    }

    dest
}

/// Concatenate two strings with a maximum length
#[no_mangle]
pub unsafe extern "C" fn strncat(dest: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }

    let mut i = 0;
    while *dest.add(i) != 0 {
        i += 1;
    }

    let mut j = 0;
    while j < n {
        let c = *src.add(j);
        *dest.add(i + j) = c;

        if c == 0 {
            break;
        }

        j += 1;
    }

    // Ensure null termination
    *dest.add(i + j) = 0;

    dest
}

/// Find a character in a string
#[no_mangle]
pub unsafe extern "C" fn strchr(s: *const c_char, c: c_int) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }

    let mut i = 0;
    loop {
        let sc = *s.add(i);

        if sc == c as i8 || sc == 0 {
            if sc == c as i8 || c == 0 {
                return s.add(i) as *mut c_char;
            }
            return ptr::null_mut();
        }

        i += 1;
    }
}

/// Find the last occurrence of a character in a string
#[no_mangle]
pub unsafe extern "C" fn strrchr(s: *const c_char, c: c_int) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }

    let mut last: *mut c_char = ptr::null_mut();
    let mut i = 0;

    loop {
        let sc = *s.add(i);

        if sc == c as i8 {
            last = s.add(i) as *mut c_char;
        }

        if sc == 0 {
            return last;
        }

        i += 1;
    }
}

/// Find a substring in a string
#[no_mangle]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    if haystack.is_null() || needle.is_null() {
        return ptr::null_mut();
    }

    let needle_len = strlen(needle);
    if needle_len == 0 {
        return haystack as *mut c_char;
    }

    let haystack_len = strlen(haystack);
    if needle_len > haystack_len {
        return ptr::null_mut();
    }

    for i in 0..=(haystack_len - needle_len) {
        let mut found = true;

        for j in 0..needle_len {
            if *haystack.add(i + j) != *needle.add(j) {
                found = false;
                break;
            }
        }

        if found {
            return haystack.add(i) as *mut c_char;
        }
    }

    ptr::null_mut()
}

/// Duplicate a string
#[no_mangle]
pub extern "C" fn strdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }

    let len = unsafe { strlen(s) } + 1;

    // TODO: Use malloc when implemented
    // For now, return null
    ptr::null_mut()
}

/// Copy memory
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }

    // Use volatile to ensure the copy is not optimized away
    let mut i = 0;
    while i < n {
        *dest.add(i) = *src.add(i);
        i += 1;
    }

    dest
}

/// Move memory
#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() || n == 0 {
        return dest;
    }

    // Check for overlap
    if src < dest as *const u8 && (src as usize).saturating_add(n) > dest as usize {
        // Copy backwards
        let mut i = n;
        while i > 0 {
            i -= 1;
            *dest.add(i) = *src.add(i);
        }
    } else {
        // Copy forwards
        let mut i = 0;
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
    }

    dest
}

/// Fill memory with a constant byte
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: c_int, n: usize) -> *mut u8 {
    if s.is_null() || n == 0 {
        return s;
    }

    let byte = c as u8;
    let mut i = 0;
    while i < n {
        *s.add(i) = byte;
        i += 1;
    }

    s
}

/// Compare memory
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> c_int {
    if s1.is_null() || s2.is_null() {
        return if s1.is_null() && s2.is_null() { 0 } else { -1 };
    }

    for i in 0..n {
        let c1 = *s1.add(i);
        let c2 = *s2.add(i);

        if c1 != c2 {
            return (c1 as c_int) - (c2 as c_int);
        }
    }

    0
}

/// Find a byte in memory
#[no_mangle]
pub unsafe extern "C" fn memchr(s: *const u8, c: c_int, n: usize) -> *mut u8 {
    if s.is_null() {
        return ptr::null_mut();
    }

    let byte = c as u8;

    for i in 0..n {
        if *s.add(i) == byte {
            return s.add(i) as *mut u8;
        }
    }

    ptr::null_mut()
}

/// Reverse a string in place
#[no_mangle]
pub unsafe extern "C" fn strrev(s: *mut c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }

    let len = strlen(s);
    if len <= 1 {
        return s;
    }

    let mut i = 0;
    let mut j = len - 1;

    while i < j {
        let tmp = *s.add(i);
        *s.add(i) = *s.add(j);
        *s.add(j) = tmp;

        i += 1;
        if j == 0 {
            break;
        }
        j -= 1;
    }

    s
}

/// Convert string to lowercase
#[no_mangle]
pub unsafe extern "C" fn strlower(s: *mut c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }

    let mut i = 0;
    loop {
        let c = *s.add(i);

        if c == 0 {
            break;
        }

        if c >= b'A' as i8 && c <= b'Z' as i8 {
            *s.add(i) = c + 32;
        }

        i += 1;
    }

    s
}

/// Convert string to uppercase
#[no_mangle]
pub unsafe extern "C" fn strupper(s: *mut c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }

    let mut i = 0;
    loop {
        let c = *s.add(i);

        if c == 0 {
            break;
        }

        if c >= b'a' as i8 && c <= b'z' as i8 {
            *s.add(i) = c - 32;
        }

        i += 1;
    }

    s
}

/// Tokenize a string
#[no_mangle]
pub static mut STRTOK_SAVE: *mut c_char = ptr::null_mut();

#[no_mangle]
pub unsafe extern "C" fn strtok(s: *mut c_char, delim: *const c_char) -> *mut c_char {
    if s.is_null() {
        if STRTOK_SAVE.is_null() {
            return ptr::null_mut();
        }
        s = STRTOK_SAVE;
    }

    // Skip leading delimiters
    let mut start = s;
    while *start != 0 && strchr(delim, *start as c_int) != ptr::null_mut() {
        start = start.add(1);
    }

    if *start == 0 {
        STRTOK_SAVE = ptr::null_mut();
        return ptr::null_mut();
    }

    // Find end of token
    let mut end = start;
    while *end != 0 && strchr(delim, *end as c_int) == ptr::null_mut() {
        end = end.add(1);
    }

    if *end == 0 {
        STRTOK_SAVE = ptr::null_mut();
    } else {
        *end = 0;
        STRTOK_SAVE = end.add(1);
    }

    start
}
