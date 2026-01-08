// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Debug Commands
//!
//! This module provides debug commands for memory inspection and manipulation.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use spin::Mutex;

use crate::rustux::types::*;
use crate::kernel::lib::console::{register_command, Cmd, CmdArg};

/// Byte order for memory operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

/// Command state for continuing operations
struct CommandState {
    /// Last address used
    last_address: AtomicU64,
    /// Last length used
    last_length: AtomicU64,
}

/// Display memory command state
static DISPLAY_STATE: Mutex<CommandState> = Mutex::new(CommandState {
    last_address: AtomicU64::new(0),
    last_length: AtomicU64::new(0),
});

/// Read memory at address
///
/// # Arguments
///
/// * `address` - Physical address to read from
/// * `size` - Size in bytes (1, 2, or 4)
/// * `len` - Number of bytes to read
/// * `byte_order` - Byte order for multi-byte reads
pub fn debug_read_memory(address: u64, size: usize, len: usize, byte_order: ByteOrder) {
    println!("Reading {} bytes from {:#x} (size: {}, order: {:?})", len, address, size, byte_order);

    let mut current_addr = address;
    let remaining = len;

    while remaining > 0 {
        let to_read = size.min(remaining);

        // TODO: Implement actual memory read
        // For now, just print placeholder
        println!("{:#x}: [{:?}] <data>", current_addr, [0u8; to_read]);

        current_addr += to_read as u64;
        // remaining -= to_read; // This would be used in a real loop
        break; // Placeholder
    }

    // Update state
    let state = DISPLAY_STATE.lock();
    state.last_address.store(current_addr, Ordering::Release);
    state.last_length.store(len as u64, Ordering::Release);
}

/// Write memory at address
///
/// # Arguments
///
/// * `address` - Physical address to write to
/// * `size` - Size in bytes (1, 2, or 4)
/// * `value` - Value to write
/// * `byte_order` - Byte order for multi-byte writes
pub fn debug_write_memory(address: u64, size: usize, value: u64, byte_order: ByteOrder) {
    println!(
        "Writing {:#x} to {:#x} (size: {}, order: {:?})",
        value, address, size, byte_order
    );

    // TODO: Implement actual memory write
    match size {
        1 => {
            // TODO: write_byte(address, value as u8)
        }
        2 => {
            // TODO: write_halfword(address, value as u16)
        }
        4 => {
            // TODO: write_word(address, value as u32)
        }
        _ => {
            println!("Invalid size: {}", size);
        }
    }
}

/// Fill memory range with value
///
/// # Arguments
///
/// * `address` - Physical address to start at
/// * `size` - Size in bytes (1, 2, or 4)
/// * `len` - Length of range to fill
/// * `value` - Value to fill with
pub fn debug_fill_memory(address: u64, size: usize, len: usize, value: u64) {
    println!(
        "Filling {:#x} - {:#x} with {:#x} (size: {})",
        address,
        address + len as u64,
        value,
        size
    );

    // TODO: Implement actual memory fill
    let mut current_addr = address;
    let remaining = len / size;

    for _ in 0..remaining {
        // TODO: Write value to current_addr
        current_addr += size as u64;
    }
}

/// Copy memory from source to destination
///
/// # Arguments
///
/// * `dest` - Destination address
/// * `src` - Source address
/// * `len` - Length in bytes
pub fn debug_copy_memory(dest: u64, src: u64, len: usize) {
    println!("Copying {:#x} -> {:#x} ({} bytes)", src, dest, len);

    // TODO: Implement actual memory copy
    // This would read from src and write to dest
}

/// Simple memory test
///
/// # Arguments
///
/// * `address` - Start address
/// * `len` - Length to test
/// * `iterations` - Number of test iterations
pub fn debug_memtest(address: u64, len: usize, iterations: usize) {
    println!(
        "Memory test: {:#x} - {:#x} ({} iterations)",
        address,
        address + len as u64,
        iterations
    );

    // TODO: Implement memory test
    // This would write patterns, read back, and verify
}

/// Sleep for specified duration
///
/// # Arguments
///
/// * `milliseconds` - Duration to sleep in milliseconds
pub fn debug_sleep(milliseconds: u64) {
    println!("Sleeping for {} ms", milliseconds);

    // TODO: Implement actual sleep
    // This would use a timer or busy-wait
}

/// Display kernel command line
pub fn debug_display_cmdline() {
    // TODO: Get actual command line
    println!("Kernel command line: <not implemented>");
}

/// Intentionally crash the system (for testing)
pub fn debug_crash() {
    println!("Intentionally crashing system...");
    panic!("Debug crash requested");
}

/// Intentionally overrun the stack (for testing stack canaries)
pub fn debug_stack_stomp() {
    println!("Intentionally stomping stack...");

    // Allocate a large array on the stack
    let mut large_array = [0u8; 0x1000]; // 4KB
    for i in 0..large_array.len() {
        large_array[i] = i as u8;
    }

    println!("Stack stomp complete (should have triggered stack protector if enabled)");
}

/// Display memory command implementation
fn cmd_display_mem(_argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    let mut address: u64 = 0;
    let mut len: usize = 0;
    let size: usize;
    let mut byte_order = ByteOrder::LittleEndian;

    // Determine size from command name
    match argv[0].str {
        "dw" => size = 4,
        "dh" => size = 2,
        "db" => size = 1,
        _ => {
            println!("Invalid display command: {}", argv[0].str);
            return -1;
        }
    }

    // Parse arguments
    let mut arg_index = 1;
    while arg_index < argv.len() {
        match argv[arg_index].str {
            "-l" => byte_order = ByteOrder::LittleEndian,
            "-b" => byte_order = ByteOrder::BigEndian,
            arg => {
                // Try to parse as number
                if let Ok(val) = u64::from_str_radix(arg.trim_start_matches("0x"), 16) {
                    if address == 0 {
                        address = val;
                    } else {
                        len = val as usize;
                    }
                }
            }
        }
        arg_index += 1;
    }

    // Use previous state if not specified
    if address == 0 && len == 0 {
        let state = DISPLAY_STATE.lock();
        address = state.last_address.load(Ordering::Acquire);
        len = state.last_length.load(Ordering::Acquire) as usize;
    }

    if len == 0 {
        len = 0x40; // Default to 64 bytes
    }

    debug_read_memory(address, size, len, byte_order);

    0
}

/// Modify memory command implementation
fn cmd_modify_mem(_argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    let size: usize;
    let mut byte_order = ByteOrder::LittleEndian;

    // Determine size from command name
    match argv[0].str {
        "mw" => size = 4,
        "mh" => size = 2,
        "mb" => size = 1,
        _ => {
            println!("Invalid modify command: {}", argv[0].str);
            return -1;
        }
    }

    if argv.len() < 3 {
        println!("usage: {} <address> <value>", argv[0].str);
        return -1;
    }

    let address = argv[1].u;
    let value = argv[2].u;

    debug_write_memory(address, size, value, byte_order);

    0
}

/// Fill memory command implementation
fn cmd_fill_mem(_argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    let size: usize;

    // Determine size from command name
    match argv[0].str {
        "fw" => size = 4,
        "fh" => size = 2,
        "fb" => size = 1,
        _ => {
            println!("Invalid fill command: {}", argv[0].str);
            return -1;
        }
    }

    if argv.len() < 4 {
        println!("usage: {} <address> <len> <value>", argv[0].str);
        return -1;
    }

    let address = argv[1].u;
    let len = argv[2].u as usize;
    let value = argv[3].u;

    debug_fill_memory(address, size, len, value);

    0
}

/// Copy memory command implementation
fn cmd_copy_mem(_argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    if argv.len() < 4 {
        println!("usage: mc <dest> <src> <len>");
        return -1;
    }

    let dest = argv[1].u;
    let src = argv[2].u;
    let len = argv[3].u as usize;

    debug_copy_memory(dest, src, len);

    0
}

/// Memory test command implementation
fn cmd_memtest(_argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    if argv.len() < 3 {
        println!("usage: mtest <address> <len> [iterations]");
        return -1;
    }

    let address = argv[1].u;
    let len = argv[2].u as usize;
    let iterations = if argv.len() > 3 {
        argv[3].u as usize
    } else {
        1
    };

    debug_memtest(address, len, iterations);

    0
}

/// Sleep command implementation
fn cmd_sleep(_argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    if argv.len() < 2 {
        println!("usage: sleep <duration>");
        return -1;
    }

    let duration = argv[1].u;

    debug_sleep(duration);

    0
}

/// Crash command implementation
fn cmd_crash(_argc: usize, _argv: &[CmdArg], _flags: u32) -> i32 {
    debug_crash();
    0
}

/// Stack stomp command implementation
fn cmd_stackstomp(_argc: usize, _argv: &[CmdArg], _flags: u32) -> i32 {
    debug_stack_stomp();
    0
}

/// Command line display implementation
fn cmd_cmdline(_argc: usize, _argv: &[CmdArg], _flags: u32) -> i32 {
    debug_display_cmdline();
    0
}

/// Register all debug commands
pub fn debugcommands_register() {
    // Display memory commands
    register_command(Cmd {
        name: "dw",
        help: "display memory in words",
        func: Some(cmd_display_mem),
        flags: 0,
    });
    register_command(Cmd {
        name: "dh",
        help: "display memory in halfwords",
        func: Some(cmd_display_mem),
        flags: 0,
    });
    register_command(Cmd {
        name: "db",
        help: "display memory in bytes",
        func: Some(cmd_display_mem),
        flags: 0,
    });

    // Modify memory commands
    register_command(Cmd {
        name: "mw",
        help: "modify word of memory",
        func: Some(cmd_modify_mem),
        flags: 0,
    });
    register_command(Cmd {
        name: "mh",
        help: "modify halfword of memory",
        func: Some(cmd_modify_mem),
        flags: 0,
    });
    register_command(Cmd {
        name: "mb",
        help: "modify byte of memory",
        func: Some(cmd_modify_mem),
        flags: 0,
    });

    // Fill memory commands
    register_command(Cmd {
        name: "fw",
        help: "fill range of memory by word",
        func: Some(cmd_fill_mem),
        flags: 0,
    });
    register_command(Cmd {
        name: "fh",
        help: "fill range of memory by halfword",
        func: Some(cmd_fill_mem),
        flags: 0,
    });
    register_command(Cmd {
        name: "fb",
        help: "fill range of memory by byte",
        func: Some(cmd_fill_mem),
        flags: 0,
    });

    // Copy memory command
    register_command(Cmd {
        name: "mc",
        help: "copy a range of memory",
        func: Some(cmd_copy_mem),
        flags: 0,
    });

    // Memory test command
    register_command(Cmd {
        name: "mtest",
        help: "simple memory test",
        func: Some(cmd_memtest),
        flags: 0,
    });

    // Sleep commands
    register_command(Cmd {
        name: "sleep",
        help: "sleep number of seconds",
        func: Some(cmd_sleep),
        flags: 0,
    });
    register_command(Cmd {
        name: "sleepm",
        help: "sleep number of milliseconds",
        func: Some(cmd_sleep),
        flags: 0,
    });

    // Debug commands
    register_command(Cmd {
        name: "crash",
        help: "intentionally crash",
        func: Some(cmd_crash),
        flags: 0,
    });
    register_command(Cmd {
        name: "stackstomp",
        help: "intentionally overrun the stack",
        func: Some(cmd_stackstomp),
        flags: 0,
    });

    // Command line display
    register_command(Cmd {
        name: "cmdline",
        help: "display kernel commandline",
        func: Some(cmd_cmdline),
        flags: 0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_order() {
        assert_eq!(ByteOrder::LittleEndian, ByteOrder::LittleEndian);
        assert_eq!(ByteOrder::BigEndian, ByteOrder::BigEndian);
    }

    #[test]
    fn test_state() {
        let state = CommandState {
            last_address: AtomicU64::new(0),
            last_length: AtomicU64::new(0),
        };

        assert_eq!(state.last_address.load(Ordering::Acquire), 0);
        assert_eq!(state.last_length.load(Ordering::Acquire), 0);
    }
}
