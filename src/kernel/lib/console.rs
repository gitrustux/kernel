// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Console
//!
//! This module provides a command-line console interface for kernel debugging.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Maximum line length for console commands
pub const LINE_LEN: usize = 128;

/// Maximum line length for panic messages
pub const PANIC_LINE_LEN: usize = 32;

/// Maximum number of command arguments
pub const MAX_NUM_ARGS: usize = 16;

/// Command history length
const HISTORY_LEN: usize = 16;

/// Enable command history
const CONSOLE_ENABLE_HISTORY: bool = true;

/// Whitespace characters for command parsing
const WHITESPACE: &[char] = &[' ', '\t'];

/// Command availability flags
pub const CMD_AVAIL_ALWAYS: u32 = 0x1;
pub const CMD_AVAIL_NORMAL: u32 = 0x2;
pub const CMD_AVAIL_PANIC: u32 = 0x4;

/// Command function type
pub type CmdFunc = fn(argc: usize, argv: &[CmdArg], flags: u32) -> i32;

/// Command argument
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CmdArg {
    /// String argument
    pub str: &'static str,
    /// Unsigned integer argument
    pub u: u64,
    /// Signed integer argument
    pub i: i64,
    /// Float argument
    pub f: f64,
    /// Boolean argument
    pub b: bool,
}

/// Command descriptor
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Cmd {
    /// Command name
    pub name: &'static str,
    /// Help string
    pub help: &'static str,
    /// Command function
    pub func: Option<CmdFunc>,
    /// Availability flags
    pub flags: u32,
}

/// Console state
struct ConsoleState {
    /// Debug buffer
    debug_buffer: Option<Vec<u8>>,
    /// Echo commands
    echo: bool,
    /// Last command result
    last_result: i32,
    /// Abort script flag
    abort_script: bool,
    /// Command history
    #[cfg(CONSOLE_ENABLE_HISTORY)]
    history: Vec<String>,
    /// Next history index
    #[cfg(CONSOLE_ENABLE_HISTORY)]
    history_next: usize,
}

impl ConsoleState {
    fn new() -> Self {
        Self {
            debug_buffer: None,
            echo: true,
            last_result: 0,
            abort_script: false,
            #[cfg(CONSOLE_ENABLE_HISTORY)]
            history: Vec::with_capacity(HISTORY_LEN),
            #[cfg(CONSOLE_ENABLE_HISTORY)]
            history_next: 0,
        }
    }
}

/// Global console state
static CONSOLE_STATE: Mutex<ConsoleState> = Mutex::new(ConsoleState::new());

/// Registered commands
static COMMANDS: Mutex<BTreeMap<&'static str, Cmd>> = Mutex::new(BTreeMap::new());

/// Register a command
pub fn register_command(cmd: Cmd) {
    COMMANDS.lock().insert(cmd.name, cmd);
}

/// Unregister a command
pub fn unregister_command(name: &str) {
    COMMANDS.lock().remove(name);
}

/// Get echo state
pub fn get_echo() -> bool {
    CONSOLE_STATE.lock().echo
}

/// Set echo state
pub fn set_echo(echo: bool) {
    CONSOLE_STATE.lock().echo = echo;
}

/// Get last command result
pub fn get_last_result() -> i32 {
    CONSOLE_STATE.lock().last_result
}

/// Get abort script flag
pub fn get_abort_script() -> bool {
    CONSOLE_STATE.lock().abort_script
}

/// Set abort script flag
pub fn set_abort_script(abort: bool) {
    CONSOLE_STATE.lock().abort_script = abort;
}

/// Split a command line into arguments
pub fn split_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

/// Parse command arguments into CmdArg structures
pub fn parse_args(args: &[String]) -> Vec<CmdArg> {
    args.iter()
        .map(|s| CmdArg {
            str: Box::leak(s.clone().into_boxed_str()),
            u: s.parse().unwrap_or(0),
            i: s.parse().unwrap_or(0),
            f: s.parse().unwrap_or(0.0),
            b: s.parse().unwrap_or(false),
        })
        .collect()
}

/// Execute a command line
pub fn exec_command(line: &str) -> i32 {
    let args = split_args(line);
    if args.is_empty() {
        return 0;
    }

    let cmd_name = Box::leak(args[0].clone().into_boxed_str());
    let commands = COMMANDS.lock();

    if let Some(cmd) = commands.get(cmd_name) {
        if let Some(func) = cmd.func {
            let parsed_args = parse_args(&args[1..]);
            let result = func(args.len() - 1, &parsed_args, CMD_AVAIL_NORMAL);

            let mut state = CONSOLE_STATE.lock();
            state.last_result = result;
            result
        } else {
            println!("Command '{}' has no function", cmd_name);
            -1
        }
    } else {
        println!("Unknown command: '{}'", cmd_name);
        -1
    }
}

/// Initialize the console subsystem
pub fn console_init() {
    #[cfg(CONSOLE_ENABLE_HISTORY)]
    {
        let mut state = CONSOLE_STATE.lock();
        // Initialize history with empty strings
        state.history = vec![String::new(); HISTORY_LEN];
        state.history_next = 0;
    }

    // Register built-in commands
    register_builtin_commands();
}

/// Register built-in commands
fn register_builtin_commands() {
    // Help command
    register_command(Cmd {
        name: "help",
        help: "Show this list",
        func: Some(cmd_help),
        flags: CMD_AVAIL_ALWAYS,
    });

    // Echo command
    register_command(Cmd {
        name: "echo",
        help: "Toggle command echo or echo text",
        func: Some(cmd_echo),
        flags: CMD_AVAIL_ALWAYS,
    });

    #[cfg(CONSOLE_ENABLE_HISTORY)]
    {
        // History command
        register_command(Cmd {
            name: "history",
            help: "Show command history",
            func: Some(cmd_history),
            flags: CMD_AVAIL_ALWAYS,
        });
    }
}

/// Help command implementation
fn cmd_help(_argc: usize, _argv: &[CmdArg], _flags: u32) -> i32 {
    println!("Available commands:");

    let commands = COMMANDS.lock();
    for (name, cmd) in commands.iter() {
        if cmd.help.is_empty() {
            println!("  {}", name);
        } else {
            println!("  {:16} - {}", name, cmd.help);
        }
    }

    0
}

/// Echo command implementation
fn cmd_echo(argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    if argc == 0 {
        // Toggle echo
        let echo = get_echo();
        set_echo(!echo);
        println!("Echo {}", if !echo { "on" } else { "off" });
    } else {
        // Print the arguments
        for i in 0..argc {
            print!("{}", argv[i].str);
            if i < argc - 1 {
                print!(" ");
            }
        }
        println!();
    }

    0
}

#[cfg(CONSOLE_ENABLE_HISTORY)]
/// History command implementation
fn cmd_history(_argc: usize, _argv: &[CmdArg], _flags: u32) -> i32 {
    dump_history();
    0
}

#[cfg(CONSOLE_ENABLE_HISTORY)]
/// Add a line to command history
pub fn add_history(line: &str) {
    // Reject empty lines
    if line.is_empty() {
        return;
    }

    let mut state = CONSOLE_STATE.lock();

    // Check if the same as last entry
    let last = (state.history_next + HISTORY_LEN - 1) % HISTORY_LEN;
    if state.history[last] == line {
        return;
    }

    // Add the new line
    state.history[state.history_next] = line.to_string();
    state.history_next = (state.history_next + 1) % HISTORY_LEN;
}

#[cfg(CONSOLE_ENABLE_HISTORY)]
/// Get previous history entry
pub fn prev_history(cursor: &mut usize) -> Option<String> {
    let state = CONSOLE_STATE.lock();

    if *cursor == state.history_next {
        return None;
    }

    *cursor = (*cursor + HISTORY_LEN - 1) % HISTORY_LEN;
    Some(state.history[*cursor].clone())
}

#[cfg(CONSOLE_ENABLE_HISTORY)]
/// Get next history entry
pub fn next_history(cursor: &mut usize) -> Option<String> {
    let state = CONSOLE_STATE.lock();

    let next = (*cursor + 1) % HISTORY_LEN;
    if next == state.history_next {
        return None;
    }

    *cursor = next;
    Some(state.history[*cursor].clone())
}

#[cfg(CONSOLE_ENABLE_HISTORY)]
/// Dump command history
fn dump_history() {
    println!("Command history:");

    let state = CONSOLE_STATE.lock();
    let mut ptr = (state.history_next + HISTORY_LEN - 1) % HISTORY_LEN;

    for _ in 0..HISTORY_LEN {
        if !state.history[ptr].is_empty() {
            println!("\t{}", state.history[ptr]);
        }
        ptr = (ptr + HISTORY_LEN - 1) % HISTORY_LEN;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_args() {
        let args = split_args("hello world");
        assert_eq!(args, vec!["hello", "world"]);

        let args = split_args("  foo  bar  baz  ");
        assert_eq!(args, vec!["foo", "bar", "baz"]);

        let args = split_args("single");
        assert_eq!(args, vec!["single"]);

        let args = split_args("");
        assert!(args.is_empty());
    }

    #[test]
    fn test_parse_args() {
        let args = vec!["123".to_string(), "-456".to_string()];
        let parsed = parse_args(&args);

        assert_eq!(parsed[0].u, 123);
        assert_eq!(parsed[0].i, 123);
        assert_eq!(parsed[1].i, -456);
    }

    #[test]
    fn test_cmd_echo() {
        let argv = [CmdArg {
            str: "test",
            u: 0,
            i: 0,
            f: 0.0,
            b: false,
        }];
        let result = cmd_echo(1, &argv, 0);
        assert_eq!(result, 0);
    }
}
