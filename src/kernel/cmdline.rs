// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Command Line Parsing
//!
//! This module provides command line argument parsing for the kernel.
//! It parses boot arguments and stores them in a key=value format.
//!
//! # Design
//!
//! - Double-null terminated strings
//! - Invalid characters converted to '.'
//! - Spaces become null separators
//! - Auto-adds '=' if key has no value
//!
//! # Usage
//!
//! ```rust
//! // Append command line data
//! cmdline_append("kernel.entropy=123");
//! cmdline_append("kernel.halt=true");
//!
//! // Get a value
//! let val = cmdline_get("kernel.entropy");
//!
//! // Get with default
//! let halt = cmdline_get_bool("kernel.halt", false);
//! let port = cmdline_get_uint32("kernel.serial.port", 0x3f8);
//! ```


use crate::kernel::sync::spin::SpinMutex as Mutex;

/// ============================================================================
/// Command Line Configuration
/// ============================================================================

/// Maximum command line size
const CMDLINE_MAX: usize = 4096;

/// Global command line storage
static mut CMDLINE_DATA: [u8; CMDLINE_MAX] = [0; CMDLINE_MAX];

/// Command line size
static CMDLINE_SIZE: Mutex<usize> = Mutex::new(0);

/// Command line entry count
static CMDLINE_COUNT: Mutex<usize> = Mutex::new(0);

/// ============================================================================
/// Public API
/// ============================================================================

/// Append data to the kernel command line
///
/// Processes the input string and adds it to the command line storage.
/// Invalid characters are converted to '.', spaces become separators.
///
/// # Arguments
///
/// * `data` - String to append (null-terminated)
pub fn cmdline_append(data: &str) {
    let data_bytes = data.as_bytes();

    if data_bytes.is_empty() || data_bytes[0] == 0 {
        return;
    }

    let mut size = {
        let sz = CMDLINE_SIZE.lock();
        *sz
    };

    if size >= CMDLINE_MAX {
        return;
    }

    // If we have a double-null terminator at the end, step back
    if size >= 2 {
        unsafe {
            if CMDLINE_DATA[size] == 0 && CMDLINE_DATA[size - 1] == 0 {
                size -= 1;
            }
        }
    }

    let max = CMDLINE_MAX - 2;

    // If existing arguments are missing a null separator, add one
    if size < max && size > 0 {
        unsafe {
            if CMDLINE_DATA[size] != 0 {
                size += 1;
                CMDLINE_DATA[size] = 0;
            }
        }
    }

    let mut found_equal = false;
    let mut i = size;

    while i < max {
        let c = if i < data_bytes.len() {
            data_bytes[i]
        } else {
            0
        };

        if c == 0 {
            // Finish in-progress argument
            unsafe {
                if CMDLINE_DATA[i - 1] != 0 {
                    if !found_equal {
                        CMDLINE_DATA[i] = b'=';
                        i += 1;
                    }
                    CMDLINE_DATA[i] = 0;
                    i += 1;

                    let mut count = CMDLINE_COUNT.lock();
                    *count += 1;
                }
            }
            break;
        }

        if c == b'=' {
            found_equal = true;
        }

        // Handle invalid characters
        let processed_c = if c < b' ' || c > 127 {
            match c {
                b'\n' | b'\r' | b'\t' => b' ',
                _ => b'.',
            }
        } else {
            c
        };

        if processed_c == b' ' {
            // Spaces become null separators
            if i == 0 || unsafe { CMDLINE_DATA[i - 1] } == 0 {
                // Skip leading/multiple spaces
            } else {
                unsafe {
                    if !found_equal && i < max {
                        CMDLINE_DATA[i] = b'=';
                        i += 1;
                    }
                    CMDLINE_DATA[i] = 0;
                    i += 1;
                    found_equal = false;

                    let mut count = CMDLINE_COUNT.lock();
                    *count += 1;
                }
            }
        } else {
            unsafe {
                CMDLINE_DATA[i] = processed_c;
                i += 1;
            }
        }
    }

    // Ensure double-null terminator
    unsafe {
        CMDLINE_DATA[i] = 0;
        if i + 1 < CMDLINE_MAX {
            CMDLINE_DATA[i + 1] = 0;
            i += 1;
        }
    }

    let mut sz = CMDLINE_SIZE.lock();
    *sz = i;
}

/// Get a value from the command line
///
/// # Arguments
///
/// * `key` - Key to look up (without '=')
///
/// # Returns
///
/// Value string if found, None otherwise
pub fn cmdline_get(key: &str) -> Option<&'static str> {
    if key.is_empty() {
        unsafe {
            return Some(core::str::from_utf8_unchecked(
                &CMDLINE_DATA[..*CMDLINE_SIZE.lock()],
            ));
        }
    }

    let key_bytes = key.as_bytes();

    unsafe {
        let mut ptr = 0;
        let sz = *CMDLINE_SIZE.lock();

        while ptr < sz {
            // Check if key matches at this position
            let mut matches = true;
            for (i, &kb) in key_bytes.iter().enumerate() {
                let c = CMDLINE_DATA[ptr + i];
                if c != kb && c != 0 && c != b'=' {
                    matches = false;
                    break;
                }
            }

            let next_char = CMDLINE_DATA[ptr + key_bytes.len()];
            if matches && (next_char == b'=' || next_char == 0) {
                // Found it - return the value part
                let value_start = ptr + key_bytes.len();
                if CMDLINE_DATA[value_start] == b'=' {
                    let value_ptr = value_start + 1;
                    return Some(core::str::from_utf8_unchecked(
                        &CMDLINE_DATA[value_ptr..],
                    ));
                } else {
                    return Some("");
                }
            }

            // Move to next entry
            while ptr < sz && CMDLINE_DATA[ptr] != 0 {
                ptr += 1;
            }
            if ptr < sz && CMDLINE_DATA[ptr] == 0 {
                ptr += 1;
            }
            if ptr < sz && CMDLINE_DATA[ptr] == 0 {
                // Double-null - end of list
                break;
            }
        }

        None
    }
}

/// Get a boolean value from the command line
///
/// Returns false if the value is "0", "false", or "off".
/// Returns true for any other value (or if key not found).
///
/// # Arguments
///
/// * `key` - Key to look up
/// * `default` - Default value if key not found
pub fn cmdline_get_bool(key: &str, default: bool) -> bool {
    match cmdline_get(key) {
        None => default,
        Some(v) => {
            matches!(v, "0" | "false" | "off") == false
        }
    }
}

/// Get a uint32 value from the command line
///
/// # Arguments
///
/// * `key` - Key to look up
/// * `default` - Default value if key not found or invalid
pub fn cmdline_get_uint32(key: &str, default: u32) -> u32 {
    match cmdline_get(key) {
        None => default,
        Some("") => default,
        Some(v) => {
            // Parse as hex or decimal
            let mut result: u32 = 0;
            let mut base = 10;

            let bytes = v.as_bytes();
            let mut i = 0;

            // Check for hex prefix
            if bytes.len() >= 2 && bytes[0] == b'0' && (bytes[1] == b'x' || bytes[1] == b'X') {
                base = 16;
                i = 2;
            }

            while i < bytes.len() {
                let c = bytes[i];
                let digit = if c >= b'0' && c <= b'9' {
                    (c - b'0') as u32
                } else if c >= b'a' && c <= b'f' {
                    (c - b'a' + 10) as u32
                } else if c >= b'A' && c <= b'F' {
                    (c - b'A' + 10) as u32
                } else {
                    return default; // Invalid character
                };

                if digit >= base as u32 {
                    return default;
                }

                result = result.wrapping_mul(base);
                result = result.wrapping_add(digit);
                i += 1;
            }

            result
        }
    }
}

/// Get a uint64 value from the command line
///
/// # Arguments
///
/// * `key` - Key to look up
/// * `default` - Default value if key not found or invalid
pub fn cmdline_get_uint64(key: &str, default: u64) -> u64 {
    match cmdline_get(key) {
        None => default,
        Some("") => default,
        Some(v) => {
            // Parse as hex or decimal
            let mut result: u64 = 0;
            let mut base = 10;

            let bytes = v.as_bytes();
            let mut i = 0;

            // Check for hex prefix
            if bytes.len() >= 2 && bytes[0] == b'0' && (bytes[1] == b'x' || bytes[1] == b'X') {
                base = 16;
                i = 2;
            }

            while i < bytes.len() {
                let c = bytes[i];
                let digit = if c >= b'0' && c <= b'9' {
                    (c - b'0') as u64
                } else if c >= b'a' && c <= b'f' {
                    (c - b'a' + 10) as u64
                } else if c >= b'A' && c <= b'F' {
                    (c - b'A' + 10) as u64
                } else {
                    return default; // Invalid character
                };

                if digit >= base as u64 {
                    return default;
                }

                result = result.wrapping_mul(base as u64);
                result = result.wrapping_add(digit);
                i += 1;
            }

            result
        }
    }
}

/// Get the raw command line data
///
/// # Returns
///
/// Slice containing the command line data
pub fn cmdline_data() -> &'static [u8] {
    unsafe {
        &CMDLINE_DATA[..*CMDLINE_SIZE.lock()]
    }
}

/// Get the number of command line entries
pub fn cmdline_count() -> usize {
    *CMDLINE_COUNT.lock()
}

/// Initialize the cmdline subsystem
pub fn init() {
    // Cmdline is initialized through cmdline_append() calls
    // from early boot parameters
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmdline_empty() {
        assert!(cmdline_get("test").is_none());
        assert_eq!(cmdline_count(), 0);
    }

    #[test]
    fn test_cmdline_append_simple() {
        cmdline_append("test=value");
        assert_eq!(cmdline_get("test"), Some("value"));
    }

    #[test]
    fn test_cmdline_get_bool() {
        cmdline_append("flag1=true");
        cmdline_append("flag2=false");
        cmdline_append("flag3=1");

        assert!(cmdline_get_bool("flag1", false));
        assert!(!cmdline_get_bool("flag2", true));
        assert!(cmdline_get_bool("flag3", false));

        // Test default for missing key
        assert!(cmdline_get_bool("missing", true));
    }

    #[test]
    fn test_cmdline_get_uint32() {
        cmdline_append("num1=123");
        cmdline_append("num2=0x1ff");
        cmdline_append("num3=0XABC");

        assert_eq!(cmdline_get_uint32("num1", 0), 123);
        assert_eq!(cmdline_get_uint32("num2", 0), 0x1ff);
        assert_eq!(cmdline_get_uint32("num3", 0), 0xABC);

        // Test default for missing key
        assert_eq!(cmdline_get_uint32("missing", 42), 42);
    }

    #[test]
    fn test_cmdline_get_uint64() {
        cmdline_append("num1=123456789");
        cmdline_append("num2=0x1ffffffff");

        assert_eq!(cmdline_get_uint64("num1", 0), 123456789);
        assert_eq!(cmdline_get_uint64("num2", 0), 0x1ffffffff);
    }

    #[test]
    fn test_cmdline_spaces() {
        cmdline_append("key1=value1 key2=value2");

        assert_eq!(cmdline_get("key1"), Some("value1"));
        assert_eq!(cmdline_get("key2"), Some("value2"));
    }
}
