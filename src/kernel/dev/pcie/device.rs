// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! PCIe Device Structures and Management
//!
//! This module defines structures for representing PCI devices
//! and functions for device management.

#![no_std]

use crate::kernel::dev::pcie::config::PcieAddr;
use crate::kernel::dev::pcie::constants::*;
use crate::kernel::dev::pcie::PciAddrSpace;
use crate::rustux::types::*;

/// PCI BAR (Base Address Register) descriptor
#[derive(Debug, Clone, Copy)]
pub struct PciBar {
    /// BAR index
    pub index: u8,

    /// Physical base address
    pub base: u64,

    /// Size in bytes
    pub size: u64,

    /// Address space type (MMIO or PIO)
    pub addr_space: PciAddrSpace,

    /// Is this a 64-bit BAR?
    pub is_64bit: bool,

    /// Is this prefetchable?
    pub is_prefetchable: bool,
}

impl PciBar {
    /// Create a new PCI BAR descriptor
    pub const fn new(
        index: u8,
        base: u64,
        size: u64,
        addr_space: PciAddrSpace,
        is_64bit: bool,
        is_prefetchable: bool,
    ) -> Self {
        Self {
            index,
            base,
            size,
            addr_space,
            is_64bit,
            is_prefetchable,
        }
    }

    /// Get the virtual address for this BAR
    pub fn virt_addr(&self) -> VAddr {
        unsafe { crate::kernel::arch::arch_traits::ArchMMU::phys_to_virt(self.base as PAddr) as VAddr }
    }
}

/// PCIe device structure
#[derive(Debug, Clone)]
pub struct PciDevice {
    /// PCIe address
    pub addr: PcieAddr,

    /// Vendor ID
    pub vendor_id: u16,

    /// Device ID
    pub device_id: u16,

    /// Class code (base, subclass, interface)
    pub class_code: (u8, u8, u8),

    /// Revision ID
    pub revision_id: u8,

    /// Header type
    pub header_type: u8,

    /// Is multifunction device
    pub is_multifunction: bool,

    /// BARs
    pub bars: [Option<PciBar>; PCIE_MAX_BAR_REGS as usize],

    /// Interrupt line
    pub irq_line: u8,

    /// Interrupt pin
    pub irq_pin: u8,

    /// Capabilities pointer
    pub caps_ptr: u8,
}

impl PciDevice {
    /// Create a new PCI device structure
    pub fn new(addr: PcieAddr) -> Self {
        Self {
            addr,
            vendor_id: 0,
            device_id: 0,
            class_code: (0, 0, 0),
            revision_id: 0,
            header_type: 0,
            is_multifunction: false,
            bars: [None; PCIE_MAX_BAR_REGS as usize],
            irq_line: 0,
            irq_pin: 0,
            caps_ptr: 0,
        }
    }

    /// Get the BDF (Bus:Device:Function) as a 32-bit value
    pub fn bdf(&self) -> u32 {
        ((self.addr.bus as u32) << 8)
            | ((self.addr.device as u32) << 3)
            | (self.addr.function as u32)
    }

    /// Get a human-readable name for the device class
    pub fn class_name(&self) -> &'static str {
        match self.class_code.0 {
            PCI_CLASS_CODE_UNCLASSIFIED => "Unclassified",
            PCI_CLASS_CODE_MASS_STORAGE => "Mass Storage",
            PCI_CLASS_CODE_NETWORK => "Network",
            PCI_CLASS_CODE_DISPLAY => "Display",
            PCI_CLASS_CODE_MULTIMEDIA => "Multimedia",
            PCI_CLASS_CODE_MEMORY => "Memory",
            PCI_CLASS_CODE_BRIDGE => "Bridge",
            PCI_CLASS_CODE_COMMUNICATION => "Communication",
            PCI_CLASS_CODE_PERIPHERAL => "Peripheral",
            PCI_CLASS_CODE_INPUT => "Input",
            PCI_CLASS_CODE_DOCKING => "Docking",
            PCI_CLASS_CODE_PROCESSOR => "Processor",
            PCI_CLASS_CODE_SERIAL_BUS => "Serial Bus",
            PCI_CLASS_CODE_WIRELESS => "Wireless",
            PCI_CLASS_CODE_INTELLIGENT_IO => "Intelligent I/O",
            PCI_CLASS_CODE_SATELLITE => "Satellite",
            PCI_CLASS_CODE_ENCRYPTION => "Encryption",
            PCI_CLASS_CODE_SIGNAL_PROCESSING => "Signal Processing",
            PCI_CLASS_CODE_ACCELERATOR => "Accelerator",
            PCI_CLASS_CODE_INSTRUMENTATION => "Instrumentation",
            _ => "Unknown",
        }
    }

    /// Print device information
    pub fn print_info(&self) {
        crate::println!(
            "PCI {}: {:04x}:{:04x} {} {}",
            self.bdf(),
            self.vendor_id,
            self.device_id,
            self.class_name(),
            self.revision_id
        );

        // Print BARs
        for bar in &self.bars {
            if let Some(b) = bar {
                let space = match b.addr_space {
                    PciAddrSpace::MMIO => "MMIO",
                    PciAddrSpace::PIO => "PIO",
                };
                crate::println!(
                    "  BAR{}: {:#010x} ({:#010x}) {}{}",
                    b.index,
                    b.base,
                    b.size,
                    space,
                    if b.is_prefetchable { " prefetch" } else { "" }
                );
            }
        }

        // Print interrupt info
        if self.irq_pin != 0 {
            crate::println!(
                "  IRQ: INT{}# -> IRQ {}",
                self.irq_pin,
                self.irq_line
            );
        }
    }
}

/// PCI capability entry
#[derive(Debug, Clone, Copy)]
pub struct PciCapability {
    /// Capability ID
    pub id: u8,

    /// Pointer to next capability
    pub next: u8,

    /// Offset in config space
    pub offset: u8,
}

impl PciCapability {
    /// Create a new capability entry
    pub const fn new(id: u8, next: u8, offset: u8) -> Self {
        Self { id, next, offset }
    }
}

/// PCI capability IDs
pub mod pci_cap_id {
    pub const PCI_CAP_ID_PM: u8 = 0x01;
    pub const PCI_CAP_ID_AGP: u8 = 0x02;
    pub const PCI_CAP_ID_VPD: u8 = 0x03;
    pub const PCI_CAP_ID_SLOTID: u8 = 0x04;
    pub const PCI_CAP_ID_MSI: u8 = 0x05;
    pub const PCI_CAP_ID_EXP: u8 = 0x10;
    pub const PCI_CAP_ID_MSIX: u8 = 0x11;
    pub const PCI_CAP_ID_SATA: u8 = 0x12;
    pub const PCI_CAP_ID_AF: u8 = 0x13;
}

/// PCIe extended capability entry
#[derive(Debug, Clone, Copy)]
pub struct PciExtCapability {
    /// Capability ID
    pub id: u16,

    /// Capability version
    pub version: u8,

    /// Pointer to next capability
    pub next: u16,

    /// Offset in extended config space
    pub offset: u16,
}

impl PciExtCapability {
    /// Create a new extended capability entry
    pub const fn new(id: u16, version: u8, next: u16, offset: u16) -> Self {
        Self {
            id,
            version,
            next,
            offset,
        }
    }
}

/// PCI device iterator
pub struct PciDeviceIterator {
    ecam_base: usize,
    bus: u8,
    device: u8,
    function: u8,
    bus_end: u8,
}

impl PciDeviceIterator {
    /// Create a new PCI device iterator
    pub const fn new(ecam_base: usize, bus_start: u8, bus_end: u8) -> Self {
        Self {
            ecam_base,
            bus: bus_start,
            device: 0,
            function: 0,
            bus_end,
        }
    }
}

impl Iterator for PciDeviceIterator {
    type Item = (PcieAddr, u16, u16);

    fn next(&mut self) -> Option<Self::Item> {
        while self.bus <= self.bus_end {
            while self.device < PCIE_MAX_DEVICES_PER_BUS {
                while self.function < PCIE_MAX_FUNCTIONS_PER_DEVICE {
                    // Check if device exists
                    let vendor_id =
                        crate::kernel::dev::pcie::config::pci_get_vendor_id(
                            self.ecam_base,
                            self.bus,
                            self.device,
                            self.function,
                        );

                    // Increment function for next iteration
                    self.function += 1;

                    // Check if this is a valid device
                    if vendor_id != PCIE_INVALID_VENDOR_ID && vendor_id != 0 {
                        let addr = PcieAddr::new(0, self.bus, self.device, self.function - 1, 0);
                        let device_id = crate::kernel::dev::pcie::config::pci_get_device_id(
                            self.ecam_base,
                            self.bus,
                            self.device,
                            self.function - 1,
                        );

                        // Check if we need to scan all functions
                        if self.function == 1
                            && !crate::kernel::dev::pcie::config::pci_is_multi_function(
                                self.ecam_base,
                                self.bus,
                                self.device,
                                0,
                            )
                        {
                            // Skip to next device
                            self.function = PCIE_MAX_FUNCTIONS_PER_DEVICE;
                        }

                        return Some((addr, vendor_id, device_id));
                    }
                }

                // Move to next device
                self.device += 1;
                self.function = 0;
            }

            // Move to next bus
            self.bus += 1;
            self.device = 0;
        }

        None
    }
}
