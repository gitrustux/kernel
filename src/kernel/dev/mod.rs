// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Device Drivers
//!
//! This module contains device drivers for various hardware peripherals.
//! Drivers are organized by category and can be selectively included based
//! on the target platform.

#![no_std]

// UART drivers
pub mod uart;

// Interrupt controllers
pub mod interrupt;

// Hardware timers
pub mod timer;

// PCIe bus driver
pub mod pcie;

// Re-exports
pub use uart::*;
pub use interrupt::*;
pub use timer::*;
pub use pcie::*;
