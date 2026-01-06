// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! PCIe Configuration Space Access
//!
//! This module provides functions for reading and writing PCI configuration space
//! using ECAM (Enhanced Configuration Access Mechanism).

#![no_std]

use crate::kernel::dev::pcie::constants::*;
use crate::kernel::arch::arch_traits::ArchMMU;
use crate::rustux::types::*;

/// PCIe configuration address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcieAddr {
    /// Segment number
    pub segment: u16,

    /// Bus number
    pub bus: u8,

    /// Device number
    pub device: u8,

    /// Function number
    pub function: u8,

    /// Register offset
    pub offset: u8,
}

impl PcieAddr {
    /// Create a new PCIe address
    pub const fn new(segment: u16, bus: u8, device: u8, function: u8, offset: u8) -> Self {
        Self {
            segment,
            bus,
            device,
            function,
            offset,
        }
    }

    /// Check if the address is valid
    pub fn is_valid(&self) -> bool {
        (self.bus as u16) < PCIE_MAX_BUSES
            && self.device < PCIE_MAX_DEVICES_PER_BUS
            && self.function < PCIE_MAX_FUNCTIONS_PER_DEVICE
            && self.offset < PCIE_EXTENDED_CONFIG_SIZE as u8
    }

    /// Convert to ECAM address
    ///
    /// ECAM layout:
    /// - Bus selection: [255:20] (8 bits)
    /// - Device: [19:15] (5 bits)
    /// - Function: [14:12] (3 bits)
    /// - Register: [11:0] (12 bits)
    pub fn to_ecam_addr(&self, ecam_base: usize) -> Option<usize> {
        if !self.is_valid() {
            return None;
        }

        let addr = ecam_base
            + (self.bus as usize * PCIE_ECAM_BYTE_PER_BUS as usize)
            + ((self.device as usize) << 15)
            + ((self.function as usize) << 12)
            + (self.offset as usize);

        Some(addr)
    }
}

/// Read a 8-bit value from PCI configuration space
///
/// # Safety
///
/// The ECAM base address must be valid and the PCIe address must be within bounds
pub unsafe fn pci_conf_read8(ecam_base: usize, addr: &PcieAddr) -> u8 {
    if let Some(pa) = addr.to_ecam_addr(ecam_base) {
        #[cfg(target_arch = "x86_64")]
        use crate::kernel::arch::amd64::Amd64Arch;
        #[cfg(target_arch = "aarch64")]
        use crate::kernel::arch::arm64::AArch64Arch;
        #[cfg(target_arch = "riscv64")]
        use crate::kernel::arch::riscv64::Riscv64Arch;

        #[cfg(target_arch = "x86_64")]
        let va = <Amd64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "aarch64")]
        let va = <AArch64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "riscv64")]
        let va = <Riscv64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        *(va as *const u8)
    } else {
        0xFF
    }
}

/// Read a 16-bit value from PCI configuration space
///
/// # Safety
///
/// The ECAM base address must be valid and the PCIe address must be within bounds
pub unsafe fn pci_conf_read16(ecam_base: usize, addr: &PcieAddr) -> u16 {
    if let Some(pa) = addr.to_ecam_addr(ecam_base) {
        #[cfg(target_arch = "x86_64")]
        use crate::kernel::arch::amd64::Amd64Arch;
        #[cfg(target_arch = "aarch64")]
        use crate::kernel::arch::arm64::AArch64Arch;
        #[cfg(target_arch = "riscv64")]
        use crate::kernel::arch::riscv64::Riscv64Arch;

        #[cfg(target_arch = "x86_64")]
        let va = <Amd64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "aarch64")]
        let va = <AArch64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "riscv64")]
        let va = <Riscv64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        *(va as *const u16)
    } else {
        0xFFFF
    }
}

/// Read a 32-bit value from PCI configuration space
///
/// # Safety
///
/// The ECAM base address must be valid and the PCIe address must be within bounds
pub unsafe fn pci_conf_read32(ecam_base: usize, addr: &PcieAddr) -> u32 {
    if let Some(pa) = addr.to_ecam_addr(ecam_base) {
        #[cfg(target_arch = "x86_64")]
        use crate::kernel::arch::amd64::Amd64Arch;
        #[cfg(target_arch = "aarch64")]
        use crate::kernel::arch::arm64::AArch64Arch;
        #[cfg(target_arch = "riscv64")]
        use crate::kernel::arch::riscv64::Riscv64Arch;

        #[cfg(target_arch = "x86_64")]
        let va = <Amd64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "aarch64")]
        let va = <AArch64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "riscv64")]
        let va = <Riscv64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        *(va as *const u32)
    } else {
        0xFFFF_FFFF
    }
}

/// Write a 8-bit value to PCI configuration space
///
/// # Safety
///
/// The ECAM base address must be valid and the PCIe address must be within bounds
pub unsafe fn pci_conf_write8(ecam_base: usize, addr: &PcieAddr, value: u8) {
    if let Some(pa) = addr.to_ecam_addr(ecam_base) {
        #[cfg(target_arch = "x86_64")]
        use crate::kernel::arch::amd64::Amd64Arch;
        #[cfg(target_arch = "aarch64")]
        use crate::kernel::arch::arm64::AArch64Arch;
        #[cfg(target_arch = "riscv64")]
        use crate::kernel::arch::riscv64::Riscv64Arch;

        #[cfg(target_arch = "x86_64")]
        let va = <Amd64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "aarch64")]
        let va = <AArch64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "riscv64")]
        let va = <Riscv64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        *(va as *mut u8) = value;
    }
}

/// Write a 16-bit value to PCI configuration space
///
/// # Safety
///
/// The ECAM base address must be valid and the PCIe address must be within bounds
pub unsafe fn pci_conf_write16(ecam_base: usize, addr: &PcieAddr, value: u16) {
    if let Some(pa) = addr.to_ecam_addr(ecam_base) {
        #[cfg(target_arch = "x86_64")]
        use crate::kernel::arch::amd64::Amd64Arch;
        #[cfg(target_arch = "aarch64")]
        use crate::kernel::arch::arm64::AArch64Arch;
        #[cfg(target_arch = "riscv64")]
        use crate::kernel::arch::riscv64::Riscv64Arch;

        #[cfg(target_arch = "x86_64")]
        let va = <Amd64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "aarch64")]
        let va = <AArch64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "riscv64")]
        let va = <Riscv64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        *(va as *mut u16) = value;
    }
}

/// Write a 32-bit value to PCI configuration space
///
/// # Safety
///
/// The ECAM base address must be valid and the PCIe address must be within bounds
pub unsafe fn pci_conf_write32(ecam_base: usize, addr: &PcieAddr, value: u32) {
    if let Some(pa) = addr.to_ecam_addr(ecam_base) {
        #[cfg(target_arch = "x86_64")]
        use crate::kernel::arch::amd64::Amd64Arch;
        #[cfg(target_arch = "aarch64")]
        use crate::kernel::arch::arm64::AArch64Arch;
        #[cfg(target_arch = "riscv64")]
        use crate::kernel::arch::riscv64::Riscv64Arch;

        #[cfg(target_arch = "x86_64")]
        let va = <Amd64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "aarch64")]
        let va = <AArch64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        #[cfg(target_arch = "riscv64")]
        let va = <Riscv64Arch as ArchMMU>::phys_to_virt(pa as PAddr);
        *(va as *mut u32) = value;
    }
}

/// Get the Vendor ID from configuration space
pub fn pci_get_vendor_id(ecam_base: usize, bus: u8, device: u8, function: u8) -> u16 {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_VENDOR_ID);
    unsafe { pci_conf_read16(ecam_base, &addr) }
}

/// Get the Device ID from configuration space
pub fn pci_get_device_id(ecam_base: usize, bus: u8, device: u8, function: u8) -> u16 {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_DEVICE_ID);
    unsafe { pci_conf_read16(ecam_base, &addr) }
}

/// Get the Class Code from configuration space
pub fn pci_get_class_code(ecam_base: usize, bus: u8, device: u8, function: u8) -> (u8, u8, u8) {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_CLASS_CODE_BASE);
    let val = unsafe { pci_conf_read32(ecam_base, &addr) };
    let base = ((val >> 24) & 0xFF) as u8;
    let sub = ((val >> 16) & 0xFF) as u8;
    let intr = ((val >> 8) & 0xFF) as u8;
    (base, sub, intr)
}

/// Get the Header Type from configuration space
pub fn pci_get_header_type(ecam_base: usize, bus: u8, device: u8, function: u8) -> u8 {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_HEADER_TYPE);
    let val = unsafe { pci_conf_read8(ecam_base, &addr) };
    val & PCI_HEADER_TYPE_MASK
}

/// Check if this is a multi-function device
pub fn pci_is_multi_function(ecam_base: usize, bus: u8, device: u8, function: u8) -> bool {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_HEADER_TYPE);
    let val = unsafe { pci_conf_read8(ecam_base, &addr) };
    (val & PCI_HEADER_TYPE_MULTI_FN) != 0
}

/// Get the BAR (Base Address Register) value
pub fn pci_get_bar(ecam_base: usize, bus: u8, device: u8, function: u8, bar_index: u8) -> u64 {
    if bar_index >= PCIE_MAX_BAR_REGS {
        return 0;
    }

    let offset = PCI_CONFIG_BASE_ADDRESSES + (bar_index * 4);
    let addr = PcieAddr::new(0, bus, device, function, offset);
    let mut bar = unsafe { pci_conf_read32(ecam_base, &addr) as u64 };

    // Check if this is a 64-bit BAR
    if (bar & (PCI_BAR_IO_TYPE_MMIO as u64)) == (PCI_BAR_IO_TYPE_MMIO as u64)
        && (bar & (PCI_BAR_MMIO_TYPE_MASK as u64)) == (PCI_BAR_MMIO_TYPE_64BIT as u64)
    {
        // Read upper 32 bits
        let addr_high = PcieAddr::new(0, bus, device, function, offset + 4);
        let bar_high = unsafe { pci_conf_read32(ecam_base, &addr_high) };
        bar |= (bar_high as u64) << 32;
    }

    bar
}

/// Get the Command register value
pub fn pci_get_command(ecam_base: usize, bus: u8, device: u8, function: u8) -> u16 {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_COMMAND);
    unsafe { pci_conf_read16(ecam_base, &addr) }
}

/// Set the Command register value
pub fn pci_set_command(ecam_base: usize, bus: u8, device: u8, function: u8, value: u16) {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_COMMAND);
    unsafe { pci_conf_write16(ecam_base, &addr, value); }
}

/// Enable bus mastering for a device
pub fn pci_enable_bus_master(ecam_base: usize, bus: u8, device: u8, function: u8) {
    let mut cmd = pci_get_command(ecam_base, bus, device, function);
    cmd |= PCI_COMMAND_BUS_MASTER_EN;
    cmd |= PCI_COMMAND_MEM_EN;
    pci_set_command(ecam_base, bus, device, function, cmd);
}

/// Get the capabilities pointer
pub fn pci_get_capabilities_ptr(ecam_base: usize, bus: u8, device: u8, function: u8) -> u8 {
    let addr = PcieAddr::new(0, bus, device, function, PCI_CONFIG_CAPABILITIES);
    unsafe { pci_conf_read8(ecam_base, &addr) }
}
