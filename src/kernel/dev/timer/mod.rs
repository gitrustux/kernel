// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Hardware Timers
//!
//! This module contains drivers for various hardware timer peripherals.
//! Timers are used for:
//! - System timekeeping
//! - Scheduler preemption
//! - High-resolution timeouts
//! - Performance monitoring
//!
//! # Supported Timers
//!
//! - **ARM Generic Timer**: Standard timer in ARMv8 systems (QEMU ARM virt)
//! - **x86 HPET**: High Precision Event Timer
//! - **x86 TSC**: Time Stamp Counter
//! - **RISC-V mtime**: Machine timer
//!
//! # QEMU Support
//!
//! ARM Generic Timer is fully supported in QEMU ARM virt:
//! ```bash
//! qemu-system-aarch64 -M virt -cpu cortex-a57 -m 1G
//! ```


#[cfg(target_arch = "aarch64")]
pub mod arm_generic;

// Re-exports
#[cfg(target_arch = "aarch64")]
pub use arm_generic::*;
