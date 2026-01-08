// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! UART Drivers
//!
//! This module contains drivers for various UART (Universal Asynchronous
//! Receiver-Transmitter) peripherals. UARTs are used for serial communication
//! and are commonly used for kernel debug output and console input.
//!
//! # Supported UARTs
//!
//! - **PL011**: ARM PrimeCell PL011 UART (QEMU ARM virt)
//!
//! # Usage
//!
//! The UART subsystem provides both polling and interrupt-driven modes:
//! - **Panic/Early mode**: Polling, used before interrupts are enabled
//! - **Normal mode**: Interrupt-driven, uses RX buffer and TX events


#[cfg(target_arch = "aarch64")]
pub mod pl011;

// Re-exports
#[cfg(target_arch = "aarch64")]
pub use pl011::*;
