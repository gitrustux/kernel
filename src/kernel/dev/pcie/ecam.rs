// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! PCIe ECAM (Enhanced Configuration Access Mechanism) Management
//!
//! This module provides functions for managing the ECAM region
//! and mapping it into the kernel address space.


use crate::kernel::dev::pcie::config::*;
use crate::kernel::dev::pcie::constants::*;
use crate::kernel::dev::pcie::device::*;
use crate::kernel::dev::pcie::PciAddrSpace;
use crate::rustux::types::*;
use alloc::vec::Vec;

/// ECAM region descriptor
#[derive(Debug, Clone)]
pub struct EcamRegion {
    /// Physical base address
    pub base: PAddr,

    /// Virtual base address (after mapping)
    pub virt_base: VAddr,

    /// Segment number
    pub segment: u16,

    /// Bus range start
    pub bus_start: u8,

    /// Bus range end
    pub bus_end: u8,

    /// Size in bytes
    pub size: usize,
}

impl EcamRegion {
    /// Create a new ECAM region descriptor
    pub const fn new(
        base: PAddr,
        segment: u16,
        bus_start: u8,
        bus_end: u8,
    ) -> Self {
        let size = ((bus_end as usize - bus_start as usize) + 1)
            * PCIE_ECAM_BYTE_PER_BUS as usize;

        Self {
            base,
            virt_base: 0,
            segment,
            bus_start,
            bus_end,
            size,
        }
    }

    /// Map the ECAM region into virtual address space
    ///
    /// # Safety
    ///
    /// The physical base address must be valid and the region must be properly mapped
    pub unsafe fn map(&mut self) -> core::result::Result<(), &'static str> {
        // Map the ECAM region using the MMU
        // Use architecture-agnostic device mapping flags
        #[cfg(target_arch = "x86_64")]
        let _flags = crate::kernel::arch::amd64::page_tables::mmu_flags::MMU_FLAGS_PERM_DEVICE
            | crate::kernel::arch::amd64::page_tables::mmu_flags::MMU_FLAGS_UNCACHED;

        #[cfg(target_arch = "aarch64")]
        let _flags = 0u64; // TODO: Use ARM64-specific device flags

        #[cfg(target_arch = "riscv64")]
        let _flags = 0u64; // TODO: Use RISC-V-specific device flags

        // TODO: Implement proper MMU mapping
        // For now, just set a dummy virtual address
        self.virt_base = 0x1000_0000_0000; // Placeholder

        if self.virt_base == 0 {
            Err("Failed to map ECAM region")
        } else {
            Ok(())
        }
    }

    /// Get the virtual base address
    pub fn virt_base(&self) -> VAddr {
        self.virt_base
    }

    /// Get the physical base address
    pub fn phys_base(&self) -> PAddr {
        self.base
    }

    /// Get the size of the region
    pub fn size(&self) -> usize {
        self.size
    }

    /// Check if a bus is within this region
    pub fn contains_bus(&self, bus: u8) -> bool {
        bus >= self.bus_start && bus <= self.bus_end
    }

    /// Get the ECAM offset for a given address
    pub fn get_offset(&self, bus: u8, device: u8, function: u8, offset: u8) -> usize {
        ((bus as usize - self.bus_start as usize) * PCIE_ECAM_BYTE_PER_BUS as usize)
            + ((device as usize) << 15)
            + ((function as usize) << 12)
            + (offset as usize)
    }
}

/// Scan a single PCI function and populate device info
///
/// # Safety
///
/// ecam_base must be a valid mapped ECAM region
pub unsafe fn pci_scan_function(
    ecam_base: usize,
    bus: u8,
    device: u8,
    function: u8,
) -> Option<PciDevice> {
    // Check if device exists
    let vendor_id = pci_get_vendor_id(ecam_base, bus, device, function);
    if vendor_id == PCIE_INVALID_VENDOR_ID || vendor_id == 0 {
        return None;
    }

    let addr = PcieAddr::new(0, bus, device, function, 0);
    let mut pci_dev = PciDevice::new(addr);

    // Read device identification
    pci_dev.vendor_id = vendor_id;
    pci_dev.device_id = pci_get_device_id(ecam_base, bus, device, function);
    pci_dev.class_code = pci_get_class_code(ecam_base, bus, device, function);

    // Read revision and header type
    let addr_rev = PcieAddr::new(0, bus, device, function, PCI_CONFIG_REVISION_ID);
    let rev_and_ht = pci_conf_read8(ecam_base, &addr_rev);
    pci_dev.revision_id = rev_and_ht;
    pci_dev.header_type = pci_get_header_type(ecam_base, bus, device, function);
    pci_dev.is_multifunction = pci_is_multi_function(ecam_base, bus, device, function);

    // Read interrupt info
    let addr_irq = PcieAddr::new(0, bus, device, function, PCI_CONFIG_INTERRUPT_LINE);
    let irq_line = pci_conf_read8(ecam_base, &addr_irq);
    pci_dev.irq_line = irq_line;
    pci_dev.irq_pin = pci_conf_read8(ecam_base, &PcieAddr::new(0, bus, device, function, PCI_CONFIG_INTERRUPT_PIN));

    // Read capabilities pointer
    pci_dev.caps_ptr = pci_get_capabilities_ptr(ecam_base, bus, device, function);

    // Parse BARs based on header type
    let num_bars = match pci_dev.header_type {
        PCI_HEADER_TYPE_PCI_BRIDGE => PCIE_BAR_REGS_PER_BRIDGE as usize,
        PCI_HEADER_TYPE_STANDARD => PCIE_BAR_REGS_PER_DEVICE as usize,
        _ => 0,
    };

    for i in 0..num_bars {
        let bar_value = pci_get_bar(ecam_base, bus, device, function, i as u8);

        if bar_value == 0 {
            continue;
        }

        // Determine BAR type
        let addr_space = if (bar_value & (PCI_BAR_IO_TYPE_MASK as u64)) == (PCI_BAR_IO_TYPE_PIO as u64) {
            PciAddrSpace::PIO
        } else {
            PciAddrSpace::MMIO
        };

        let is_64bit = addr_space == PciAddrSpace::MMIO
            && (bar_value & (PCI_BAR_MMIO_TYPE_MASK as u64)) == (PCI_BAR_MMIO_TYPE_64BIT as u64);

        let is_prefetchable =
            addr_space == PciAddrSpace::MMIO && (bar_value & (PCI_BAR_MMIO_PREFETCH_MASK as u64)) != 0;

        // Get base address (mask out type bits)
        let base = match addr_space {
            PciAddrSpace::PIO => bar_value & (PCI_BAR_PIO_ADDR_MASK as u64),
            PciAddrSpace::MMIO => bar_value & (PCI_BAR_MMIO_ADDR_MASK as u64),
        };

        // Determine size by writing all 1s and reading back
        let size = pci_get_bar_size(ecam_base, bus, device, function, i as u8);

        pci_dev.bars[i] = Some(PciBar::new(
            i as u8,
            base,
            size,
            addr_space,
            is_64bit,
            is_prefetchable,
        ));

        // Skip next BAR if this is 64-bit
        if is_64bit {
            pci_dev.bars[i + 1] = None;
        }
    }

    Some(pci_dev)
}

/// Get the size of a BAR by probing
///
/// # Safety
///
/// ecam_base must be a valid mapped ECAM region
unsafe fn pci_get_bar_size(
    ecam_base: usize,
    bus: u8,
    device: u8,
    function: u8,
    bar_index: u8,
) -> u64 {
    let offset = PCI_CONFIG_BASE_ADDRESSES + (bar_index * 4);
    let addr = PcieAddr::new(0, bus, device, function, offset);

    // Save original value
    let original = pci_conf_read32(ecam_base, &addr);

    // Write all 1s
    pci_conf_write32(ecam_base, &addr, 0xFFFF_FFFF);

    // Read back to get size info
    let size_mask = pci_conf_read32(ecam_base, &addr);

    // Restore original value
    pci_conf_write32(ecam_base, &addr, original);

    // Extract size from mask
    let size = if (size_mask & PCI_BAR_IO_TYPE_MASK) == PCI_BAR_IO_TYPE_PIO {
        // PIO
        (!(size_mask & PCI_BAR_PIO_ADDR_MASK)) & 0xFFFF_FFFF
    } else {
        // MMIO
        (!(size_mask & PCI_BAR_MMIO_ADDR_MASK)) & 0xFFFF_FFFF
    };

    // Get lowest set bit (actual size)
    if size == 0 {
        0
    } else {
        1u64 << (size.trailing_zeros() as u64)
    }
}

/// Scan all PCI functions on a device
///
/// # Safety
///
/// ecam_base must be a valid mapped ECAM region
pub unsafe fn pci_scan_device(
    ecam_base: usize,
    bus: u8,
    device: u8,
) -> Vec<PciDevice> {
    let mut devices = Vec::new();

    // Always scan function 0
    if let Some(dev) = pci_scan_function(ecam_base, bus, device, 0) {
        let is_multifunction = dev.is_multifunction;

        devices.push(dev);

        // Scan other functions if multifunction
        if is_multifunction {
            for func in 1..PCIE_MAX_FUNCTIONS_PER_DEVICE {
                if let Some(dev) = pci_scan_function(ecam_base, bus, device, func) {
                    devices.push(dev);
                }
            }
        }
    }

    devices
}

/// Scan all PCI devices on a bus
///
/// # Safety
///
/// ecam_base must be a valid mapped ECAM region
pub unsafe fn pci_scan_bus(
    ecam_base: usize,
    bus: u8,
) -> Vec<PciDevice> {
    let mut devices = Vec::new();

    for device in 0..PCIE_MAX_DEVICES_PER_BUS {
        for func in 0..PCIE_MAX_FUNCTIONS_PER_DEVICE {
            if let Some(dev) = pci_scan_function(ecam_base, bus, device, func) {
                // If not multifunction, skip to next device
                let is_multifunction = dev.is_multifunction;
                devices.push(dev);

                if func == 0 && !is_multifunction {
                    break;
                }
            } else if func == 0 {
                // No device at this location
                break;
            }
        }
    }

    devices
}
