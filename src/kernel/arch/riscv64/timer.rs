// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V timer functions
//!
//! Provides access to the RISC-V timer (time CSR).

use crate::rustux::types::*;

/// Get current time from RISC-V time CSR
///
/// The `time` CSR provides a monotonic counter that increments at the
/// frequency of the system's timebase (typically CPU clock frequency).
#[inline]
pub fn riscv_current_time() -> u64 {
    unsafe {
        let time: u64;
        core::arch::asm!("rdtime {0}", out(reg) time);
        time
    }
}
