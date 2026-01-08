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


// UART drivers
pub mod uart;

// Interrupt controllers
pub mod interrupt;

// Hardware timers
pub mod timer;

// PCIe bus driver
pub mod pcie;

// Hardware Random Number Generator
pub mod hw_rng;

// Intel-specific RNG (RDSEED/RDRAND)
pub mod intel_rng;

// Userspace display (framebuffer)
pub mod udisplay;

// ARM PSCI (Power State Coordination Interface)
pub mod psci;

// Re-exports
pub use uart::*;
pub use interrupt::*;
pub use timer::*;
pub use pcie::*;
