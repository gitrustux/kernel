// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Out of Memory (OOM) Handler
//!
//! This module provides an out-of-memory detection and response system.
//! It monitors free memory and triggers callbacks when memory gets low.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::rustux::types::*;
use crate::kernel::vm::pmm;
use crate::kernel::lib::console::{register_command, Cmd, CmdArg, MAX_NUM_ARGS};

/// OOM low memory callback type
pub type OomLowmemCallback = fn(shortfall_bytes: usize);

/// OOM state
struct OomState {
    /// Low memory callback
    lowmem_callback: Option<OomLowmemCallback>,
    /// Whether OOM thread is running
    running: bool,
    /// Sleep duration between checks (nanoseconds)
    sleep_duration_ns: u64,
    /// Redline bytes (start killing below this)
    redline_bytes: u64,
    /// Print free memory status
    printing: bool,
    /// Simulate low memory condition
    simulate_lowmem: bool,
}

/// Global OOM state
static OOM_STATE: Mutex<OomState> = Mutex::new(OomState {
    lowmem_callback: None,
    running: false,
    sleep_duration_ns: 1_000_000_000, // 1 second
    redline_bytes: 16 * 1024 * 1024, // 16 MB
    printing: false,
    simulate_lowmem: false,
});

/// Initialize the OOM subsystem
///
/// # Arguments
///
/// * `enable` - Whether to start the OOM thread
/// * `sleep_duration_ns` - Sleep duration between checks
/// * `redline_bytes` - Minimum free bytes before triggering OOM
/// * `lowmem_callback` - Callback function when memory is low
pub fn oom_init(
    enable: bool,
    sleep_duration_ns: u64,
    redline_bytes: u64,
    lowmem_callback: OomLowmemCallback,
) {
    assert!(sleep_duration_ns > 0);
    assert!(redline_bytes > 0);

    let mut state = OOM_STATE.lock();

    // Only initialize once
    assert!(state.lowmem_callback.is_none());

    state.lowmem_callback = Some(lowmem_callback);
    state.sleep_duration_ns = sleep_duration_ns;
    state.redline_bytes = redline_bytes;
    state.printing = false;
    state.simulate_lowmem = false;

    if enable {
        // TODO: Start OOM thread
        println!("OOM: started thread");
    } else {
        println!("OOM: thread disabled");
    }
}

/// OOM monitoring loop (called by thread)
pub fn oom_loop() -> ! {
    let total_bytes = pmm::pmm_count_total_bytes();
    let mut last_free_bytes = total_bytes;

    loop {
        let free_bytes = pmm::pmm_count_free_pages() * 4096;

        let mut lowmem = false;
        let mut printing = false;
        let mut shortfall_bytes = 0;
        let callback = {
            let mut state = OOM_STATE.lock();

            if !state.running {
                // Should exit
                break;
            }

            if state.simulate_lowmem {
                println!("OOM: simulating low-memory situation");
            }

            lowmem = free_bytes < state.redline_bytes || state.simulate_lowmem;

            if lowmem {
                shortfall_bytes = if state.simulate_lowmem {
                    512 * 1024 // Simulate 512KB shortfall
                } else {
                    state.redline_bytes - free_bytes
                };
            }

            state.simulate_lowmem = false;
            printing = lowmem || (state.printing && free_bytes != last_free_bytes);

            state.lowmem_callback
        };

        // Print memory status if needed
        if printing {
            let free_delta_bytes = (free_bytes as i64) - (last_free_bytes as i64);
            let delta_sign = if free_delta_bytes < 0 { '-' } else { '+' };
            let delta_abs = free_delta_bytes.unsigned_abs();

            println!(
                "OOM: {} free ({}{:+}) / {} total",
                format_bytes(free_bytes),
                delta_sign,
                format_bytes(delta_abs),
                format_bytes(total_bytes)
            );
        }

        last_free_bytes = free_bytes;

        // Call low memory callback if needed
        if lowmem {
            if let Some(cb) = callback {
                cb(shortfall_bytes);
            }
        }

        // Sleep until next check
        let sleep_ns = OOM_STATE.lock().sleep_duration_ns;
        // TODO: thread_sleep_relative(sleep_ns);
    }

    panic!("OOM loop exited unexpectedly");
}

/// Start the OOM thread
pub fn oom_start() {
    let mut state = OOM_STATE.lock();

    if state.running {
        println!("OOM thread already running");
        return;
    }

    // TODO: Create and start OOM thread
    state.running = true;
    println!("OOM: started thread");
}

/// Stop the OOM thread
pub fn oom_stop() {
    let mut state = OOM_STATE.lock();

    if !state.running {
        println!("OOM thread already stopped");
        return;
    }

    println!("Stopping OOM thread...");
    state.running = false;

    // TODO: Join the thread
    println!("OOM thread stopped.");
}

/// Enable or disable continuous printing of free memory
pub fn oom_set_printing(printing: bool) {
    OOM_STATE.lock().printing = printing;
}

/// Toggle printing of free memory
pub fn oom_toggle_printing() -> bool {
    let mut state = OOM_STATE.lock();
    state.printing = !state.printing;
    state.printing
}

/// Simulate a low memory condition
pub fn oom_simulate_lowmem() {
    OOM_STATE.lock().simulate_lowmem = true;
}

/// Get OOM info as a string
pub fn oom_info() -> String {
    let state = OOM_STATE.lock();

    format!(
        "OOM info:\n\
         running: {}\n\
         printing: {}\n\
         simulating lowmem: {}\n\
         sleep duration: {}ms\n\
         redline: {} ({} bytes)",
        state.running,
        state.printing,
        state.simulate_lowmem,
        state.sleep_duration_ns / 1_000_000,
        format_bytes(state.redline_bytes as usize),
        state.redline_bytes
    )
}

/// Format bytes as human-readable size
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{}GB", bytes / GB)
    } else if bytes >= MB {
        format!("{}MB", bytes / MB)
    } else if bytes >= KB {
        format!("{}KB", bytes / KB)
    } else {
        format!("{}B", bytes)
    }
}

/// OOM command implementation
fn cmd_oom(_argc: usize, argv: &[CmdArg], _flags: u32) -> i32 {
    if argv.len() < 2 {
        println!("Not enough arguments:");
        println!("  oom start  : ensure that the OOM thread is running");
        println!("  oom stop   : ensure that the OOM thread is not running");
        println!("  oom info   : dump OOM params/state");
        println!("  oom print  : continually print free memory (toggle)");
        println!("  oom lowmem : act as if the redline was just hit (once)");
        return -1;
    }

    match argv[1].str {
        "start" => {
            oom_start();
        }
        "stop" => {
            oom_stop();
        }
        "info" => {
            println!("{}", oom_info());
        }
        "print" => {
            let printing = oom_toggle_printing();
            println!("OOM print is now {}", if printing { "on" } else { "off" });
        }
        "lowmem" => {
            oom_simulate_lowmem();
        }
        _ => {
            println!("Unrecognized subcommand '{}'", argv[1].str);
            return -1;
        }
    }

    0
}

/// Register OOM commands
pub fn oom_register_commands() {
    register_command(Cmd {
        name: "oom",
        help: "out-of-memory watcher/killer",
        func: Some(cmd_oom),
        flags: 0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(2048), "2KB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2MB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3GB");
    }

    #[test]
    fn test_oom_init() {
        // Test that initialization works
        let callback_called = Arc::new(AtomicBool::new(false));

        let callback = |_shortfall: usize| {
            callback_called.store(true, Ordering::Release);
        };

        oom_init(false, 1_000_000_000, 16 * 1024 * 1024, callback);

        // Verify state was set correctly
        let state = OOM_STATE.lock();
        assert_eq!(state.sleep_duration_ns, 1_000_000_000);
        assert_eq!(state.redline_bytes, 16 * 1024 * 1024);
        assert!(!state.running);
    }
}
