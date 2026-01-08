// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Debug Utilities
//!
//! This module provides debugging utilities for the kernel including
//! panic handling, hex dumping, and stack canary support.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::rustux::types::*;

/// Hex dump print function type
pub type HexdumpPrintFn = extern "C" fn(&str);

/// Microseconds in a nanosecond
const USEC_PER_NSEC: u64 = 1000;

/// Current time (stub) - returns time in nanoseconds
fn current_time() -> u64 {
    // TODO: Implement proper time tracking
    0
}

/// Spin (busy-wait) for the specified number of microseconds
///
/// # Arguments
///
/// * `usecs` - Microseconds to spin
pub fn spin(usecs: u32) {
    let start = current_time();
    let nsecs = usecs as u64 * USEC_PER_NSEC;

    while current_time().saturating_sub(start) < nsecs {
        core::hint::spin_loop();
    }
}

/// Kernel panic with formatted message
///
/// # Arguments
///
/// * `caller` - Caller address
/// * `frame` - Frame address
/// * `fmt` - Format string
/// * `args` - Format arguments
///
/// # Safety
///
/// This function should never return.
pub fn panic_internal(
    caller: usize,
    frame: usize,
    fmt: core::fmt::Arguments,
) -> ! {
    platform_panic_start();

    print!("panic (caller {:#x} frame {:#x}): ", caller, frame);
    println!("{}", fmt);

    platform_halt();
}

/// Kernel panic without formatting
///
/// # Arguments
///
/// * `msg` - Panic message
/// * `len` - Length of message
///
/// # Safety
///
/// This function should never return.
pub fn panic_no_format(msg: &str) -> ! {
    platform_panic_start();
    println!("{}", msg);
    platform_halt();
}

/// Stack canary failure handler
///
/// This is called when stack smashing is detected.
///
/// # Safety
///
/// This function should never return.
#[no_mangle]
pub extern "C" fn __stack_chk_fail() -> ! {
    panic_no_format("stack canary corrupted!\n");
}

/// Choose a random stack guard value
///
/// # Returns
///
/// A random value to use as stack guard
pub fn choose_stack_guard() -> u64 {
    // TODO: Try to get entropy from hardware RNG
    // For now, use a "randomish" value
    let guard = 0xdeadbeef_00ff_00ffu64 ^ (0xDEADBEEF_u64);
    guard
}

/// Platform-specific panic start notification
fn platform_panic_start() {
    // TODO: Implement platform-specific panic handling
    // For now, just print that we're starting panic
    println!("Platform panic start");
}

/// Platform halt - stop the system
///
/// # Safety
///
/// This function should never return.
fn platform_halt() -> ! {
    // TODO: Implement platform-specific halt
    println!("System halted");
    loop {
        core::hint::spin_loop();
    }
}

/// Hex dump with custom print function
///
/// # Arguments
///
/// * `ptr` - Pointer to data to dump
/// * `len` - Length of data
/// * `disp_addr` - Display address (may differ from actual address)
/// * `pfn` - Print function to use
///
/// # Safety
///
/// The caller must ensure that `ptr` points to valid memory
/// for at least `len` bytes.
pub fn hexdump_very_ex(ptr: *const u8, len: usize, disp_addr: u64, pfn: HexdumpPrintFn) {
    let mut address = ptr as usize;
    let mut count = 0;

    while count < len {
        let remaining = len - count;
        let bytes_to_dump = remaining.min(16);

        // Print address
        if disp_addr + len as u64 > 0xFFFFFFFF {
            pfn(&format!("{:#018x}: ", disp_addr + count as u64));
        } else {
            pfn(&format!("{:#010x}: ", (disp_addr + count as u64) as u32));
        }

        // Print hex words (32-bit)
        let mut i = 0;
        let s = (bytes_to_dump + 3) & !3; // Round up to 4 bytes

        unsafe {
            while i < s {
                if i < bytes_to_dump {
                    let word = *(address.add(i) as *const u32);
                    pfn(&format!("{:08x} ", word));
                } else {
                    pfn("         ");
                }
                i += 4;
            }
        }

        pfn("|");

        // Print ASCII
        for i in 0..16 {
            if i < bytes_to_dump {
                unsafe {
                    let c = *(address.add(i)) as char;
                    if c.is_ascii_graphic() || c == ' ' {
                        pfn(&format!("{}", c));
                    } else {
                        pfn(".");
                    }
                }
            } else {
                pfn(" ");
            }
        }

        pfn("|\n");

        address += 16;
        count += 16;
    }
}

/// Hex dump (8-bit bytes) with custom print function
///
/// # Arguments
///
/// * `ptr` - Pointer to data to dump
/// * `len` - Length of data
/// * `disp_addr` - Display address (may differ from actual address)
/// * `pfn` - Print function to use
///
/// # Safety
///
/// The caller must ensure that `ptr` points to valid memory
/// for at least `len` bytes.
pub fn hexdump8_very_ex(ptr: *const u8, len: usize, disp_addr: u64, pfn: HexdumpPrintFn) {
    let mut address = ptr as usize;
    let mut count = 0;

    while count < len {
        let remaining = len - count;
        let bytes_to_dump = remaining.min(16);

        // Print address
        if disp_addr + len as u64 > 0xFFFFFFFF {
            pfn(&format!("{:#018x}: ", disp_addr + count as u64));
        } else {
            pfn(&format!("{:#010x}: ", (disp_addr + count as u64) as u32));
        }

        // Print hex bytes
        unsafe {
            for i in 0..bytes_to_dump {
                pfn(&format!("{:02x} ", *(address.add(i))));
            }

            // Pad to 16 bytes
            for i in bytes_to_dump..16 {
                pfn("   ");
            }

            pfn("|");

            // Print ASCII
            for i in 0..bytes_to_dump {
                let c = *(address.add(i)) as char;
                if c.is_ascii_graphic() || c == ' ' {
                    pfn(&format!("{}", c));
                } else {
                    pfn(".");
                }
            }
        }

        pfn("|\n");

        address += 16;
        count += 16;
    }
}

/// Default hex dump print function (prints to console)
extern "C" fn default_hexdump_print(s: &str) {
    print!("{}", s);
}

/// Hex dump to console
///
/// # Arguments
///
/// * `ptr` - Pointer to data to dump
/// * `len` - Length of data
///
/// # Safety
///
/// The caller must ensure that `ptr` points to valid memory
/// for at least `len` bytes.
pub fn hexdump(ptr: *const u8, len: usize) {
    hexdump_very_ex(ptr, len, ptr as u64, default_hexdump_print);
}

/// Hex dump (8-bit) to console
///
/// # Arguments
///
/// * `ptr` - Pointer to data to dump
/// * `len` - Length of data
///
/// # Safety
///
/// The caller must ensure that `ptr` points to valid memory
/// for at least `len` bytes.
pub fn hexdump8(ptr: *const u8, len: usize) {
    hexdump8_very_ex(ptr, len, ptr as u64, default_hexdump_print);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spin() {
        // Just test that spin returns
        spin(1);
    }

    #[test]
    fn test_stack_guard() {
        let guard = choose_stack_guard();
        assert_ne!(guard, 0);
    }

    #[test]
    fn test_constants() {
        assert_eq!(USEC_PER_NSEC, 1000);
    }

    #[test]
    fn test_hexdump() {
        let data = [0x41u8, 0x42, 0x43, 0x44, 0x45];
        hexdump8(data.as_ptr(), data.len());
    }
}
