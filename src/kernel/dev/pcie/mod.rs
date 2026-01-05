// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! PCIe (PCI Express) Bus Driver
//!
//! This module provides support for PCI Express bus enumeration and management.
//! PCIe is the standard expansion bus for connecting peripherals in modern systems.
//!
//! # Features
//!
//! - ECAM (Enhanced Configuration Access Mechanism) for config space access
//! - Device enumeration across buses
//! - BAR (Base Address Register) parsing
//! - MSI/MSI-X interrupt support
//! - PCIe capability parsing
//!
//! # QEMU Support
//!
//! QEMU provides full PCIe support on all architectures:
//! ```bash
//! # ARM virt (PCIe)
//! qemu-system-aarch64 -M virt -device virtio-net-pci
//!
//! # x86_64 (PCIe)
//! qemu-system-x86_64 -M q35 -device virtio-net-pci
//!
//! # RISC-V virt (PCIe)
//! qemu-system-riscv64 -M virt -device virtio-net-pci
//! ```

#![no_std]

pub mod constants;
pub mod config;
pub mod device;
pub mod ecam;

// Re-exports
pub use constants::*;
pub use config::*;
pub use device::*;
pub use ecam::*;

/// PCIe address space type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciAddrSpace {
    MMIO = 0,
    PIO = 1,
}

/// PCIe driver state
pub struct PcieDriver {
    /// ECAM base address
    ecam_base: usize,

    /// Segment number (for multi-segment systems)
    segment: u16,

    /// Bus range start
    bus_start: u8,

    /// Bus range end
    bus_end: u8,
}

impl PcieDriver {
    /// Create a new PCIe driver
    ///
    /// # Arguments
    ///
    /// * `ecam_base` - Physical base address of ECAM region
    /// * `segment` - PCIe segment number
    /// * `bus_start` - First bus number in range
    /// * `bus_end` - Last bus number in range
    pub const fn new(ecam_base: usize, segment: u16, bus_start: u8, bus_end: u8) -> Self {
        Self {
            ecam_base,
            segment,
            bus_start,
            bus_end,
        }
    }

    /// Initialize the PCIe driver
    ///
    /// This performs device enumeration across all buses.
    pub fn init(&self) -> Result<(), &'static str> {
        // TODO: Implement bus scanning
        // TODO: Create device tree
        Ok(())
    }

    /// Get ECAM base address
    pub fn ecam_base(&self) -> usize {
        self.ecam_base
    }

    /// Get segment number
    pub fn segment(&self) -> u16 {
        self.segment
    }
}
