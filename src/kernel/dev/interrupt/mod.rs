// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Interrupt Controllers
//!
//! This module contains drivers for various interrupt controllers.
//! Different architectures use different interrupt controllers:
//!
//! - **ARM**: GIC (Generic Interrupt Controller) - GICv2, GICv3, GICv4
//! - **x86**: APIC (Advanced Programmable Interrupt Controller)
//! - **RISC-V**: PLIC (Platform-Level Interrupt Controller), CLINT
//!
//! # QEMU Support
//!
//! ARM GICv2 is fully supported in QEMU ARM virt:
//! ```bash
//! qemu-system-aarch64 -M virt -cpu cortex-a57 -m 1G
//! ```


#[cfg(target_arch = "aarch64")]
pub mod arm_gic;

#[cfg(target_arch = "aarch64")]
pub mod gicv2;

// Re-exports
#[cfg(target_arch = "aarch64")]
pub use arm_gic::*;

#[cfg(target_arch = "aarch64")]
pub use gicv2::*;
