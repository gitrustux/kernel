// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Hardware Random Number Generator (Debug Interface)
//!
//! Provides debug console commands for the hardware RNG subsystem.
//! This is a Rust replacement for the legacy C++ debug implementation.
//!
//! # Commands
//!
//! - `rng32` - Generate and display a random 32-bit integer
//! - `rng <N> [wait]` - Generate and display N random bytes
//!
//! # Usage
//!
//! ```text
//! // Generate a random u32
//! rng32
//!
//! // Generate 32 bytes (non-blocking)
//! rng 32
//!
//! // Generate 64 bytes (wait for entropy)
//! rng 64 true
//! ```

use crate::kernel::dev::intel_rng;
use crate::kernel::debug::hexdump_ex;
use crate::{log_info, log_warn};

/// Generate and print a random 32-bit unsigned integer
///
/// Console command: `rng32`
pub fn cmd_rng32() {
    match intel_rng::hw_rng_get_u32() {
        val => {
            log_info!("Random val = {} (0x{:08x})", val, val);
        }
    }
}

/// Generate and print N random bytes
///
/// Console command: `rng <N> [wait]`
///
/// # Arguments
///
/// * `count` - Number of bytes to generate
/// * `wait` - If true, wait indefinitely for bytes to be generated;
///            otherwise terminate if HW generator runs out of entropy
pub fn cmd_rng(count: usize, wait: bool) {
    if count == 0 {
        log_warn!("Invalid argument count");
        log_info!("Usage: rng <N> [wait]");
        log_info!("  N    : Number of bytes to generate");
        log_info!("  wait : true  -> wait indefinitely for bytes to be generated");
        log_info!("       : false -> terminate if HW generator runs out of entropy (default)");
        return;
    }

    log_info!("Generating {} random bytes", count);

    let mut offset = 0;
    let mut buf = [0u8; 16];

    while offset < count {
        let todo = core::cmp::min(buf.len(), count - offset);
        let done = intel_rng::hw_rng_get_entropy(&mut buf[..todo], wait);

        debug_assert!(done <= todo, "HW RNG returned more bytes than requested");

        if done > 0 {
            hexdump_ex(buf.as_ptr(), done, offset);
            offset += done;
        }

        if done < todo {
            log_warn!("Entropy exhausted after {} byte{}", offset, if offset == 1 { "" } else { "s" });
            break;
        }
    }
}

/// Initialize the HW RNG debug interface
///
/// Called during kernel initialization to register console commands.
pub fn init() {
    // Console commands are registered through the debug subsystem
    log_info!("HW RNG debug interface initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_rng32() {
        cmd_rng32();
    }

    #[test]
    fn test_cmd_rng_small() {
        cmd_rng(16, false);
    }

    #[test]
    fn test_cmd_rng_zero() {
        cmd_rng(0, false);
    }
}
