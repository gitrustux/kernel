// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! PCI Configuration I/O
//!
//! This module provides PCI configuration space access via I/O ports.
//! This is primarily for x86 architecture which uses port-mapped I/O
//! for PCI configuration access.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// PCI configuration address port
#[cfg(target_arch = "x86_64")]
const PCI_CONFIG_ADDR: u16 = 0xCF8;

/// PCI configuration data port
#[cfg(target_arch = "x86_64")]
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// PCI configuration enable bit
#[cfg(target_arch = "x86_64")]
const PCI_CFG_ENABLE: u32 = 1 << 31;

/// PCI I/O spin lock
#[cfg(target_arch = "x86_64")]
static PCI_PIO_LOCK: Mutex<()> = Mutex::new(());

/// Calculate PCI config address from BDF (Bus/Device/Function)
///
/// # Arguments
///
/// * `bus` - Bus number
/// * `device` - Device number
/// * `func` - Function number
/// * `offset` - Register offset
///
/// # Returns
///
/// PCI configuration address
pub fn pci_bdf_raw_addr(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

/// Read from PCI configuration space via PIO
///
/// # Arguments
///
/// * `addr` - PCI configuration address
/// * `width` - Read width in bytes (8, 16, or 32)
///
/// # Returns
///
/// Ok(value) on success, Err(status) on failure
#[cfg(target_arch = "x86_64")]
pub fn pio_cfg_read(addr: u32, width: usize) -> Result<u32, i32> {
    let _lock = PCI_PIO_LOCK.lock();

    let shift = ((addr & 0x3) as usize) * 8;

    if shift + width > 32 {
        return Err(-1); // ZX_ERR_INVALID_ARGS
    }

    // Write address to CONFIG_ADDR
    let write_addr = (addr & !0x3) | PCI_CFG_ENABLE;
    out_port_32(PCI_CONFIG_ADDR, write_addr);

    // Read from CONFIG_DATA
    let tmp_val = in_port_32(PCI_CONFIG_DATA);

    // Extract the requested bytes
    let width_mask = width_mask(width);
    let val = (tmp_val >> shift) & width_mask;

    Ok(val)
}

/// Read from PCI configuration space via PIO (BDF format)
///
/// # Arguments
///
/// * `bus` - Bus number
/// * `device` - Device number
/// * `func` - Function number
/// * `offset` - Register offset
/// * `width` - Read width in bytes (8, 16, or 32)
///
/// # Returns
///
/// Ok(value) on success, Err(status) on failure
#[cfg(target_arch = "x86_64")]
pub fn pio_cfg_read_bdf(
    bus: u8,
    device: u8,
    func: u8,
    offset: u8,
    width: usize,
) -> Result<u32, i32> {
    let addr = pci_bdf_raw_addr(bus, device, func, offset);
    pio_cfg_read(addr, width)
}

/// Write to PCI configuration space via PIO
///
/// # Arguments
///
/// * `addr` - PCI configuration address
/// * `val` - Value to write
/// * `width` - Write width in bytes (8, 16, or 32)
///
/// # Returns
///
/// Ok(()) on success, Err(status) on failure
#[cfg(target_arch = "x86_64")]
pub fn pio_cfg_write(addr: u32, mut val: u32, width: usize) -> Result<(), i32> {
    let _lock = PCI_PIO_LOCK.lock();

    let shift = ((addr & 0x3) as usize) * 8;

    if shift + width > 32 {
        return Err(-1); // ZX_ERR_INVALID_ARGS
    }

    // Write address to CONFIG_ADDR
    let write_addr = (addr & !0x3) | PCI_CFG_ENABLE;
    out_port_32(PCI_CONFIG_ADDR, write_addr);

    // Read-modify-write
    let tmp_val = in_port_32(PCI_CONFIG_DATA);
    let width_mask = width_mask(width);
    let write_mask = width_mask << shift;

    val &= width_mask;
    let mut tmp_val = tmp_val & !write_mask;
    tmp_val |= val << shift;

    out_port_32(PCI_CONFIG_DATA, tmp_val);

    Ok(())
}

/// Write to PCI configuration space via PIO (BDF format)
///
/// # Arguments
///
/// * `bus` - Bus number
/// * `device` - Device number
/// * `func` - Function number
/// * `offset` - Register offset
/// * `val` - Value to write
/// * `width` - Write width in bytes (8, 16, or 32)
///
/// # Returns
///
/// Ok(()) on success, Err(status) on failure
#[cfg(target_arch = "x86_64")]
pub fn pio_cfg_write_bdf(
    bus: u8,
    device: u8,
    func: u8,
    offset: u8,
    val: u32,
    width: usize,
) -> Result<(), i32> {
    let addr = pci_bdf_raw_addr(bus, device, func, offset);
    pio_cfg_write(addr, val, width)
}

/// Stub implementations for non-x86 architectures
#[cfg(not(target_arch = "x86_64"))]
pub fn pio_cfg_read(_addr: u32, _width: usize) -> Result<u32, i32> {
    Err(-2) // ZX_ERR_NOT_SUPPORTED
}

#[cfg(not(target_arch = "x86_64"))]
pub fn pio_cfg_read_bdf(
    _bus: u8,
    _device: u8,
    _func: u8,
    _offset: u8,
    _width: usize,
) -> Result<u32, i32> {
    Err(-2) // ZX_ERR_NOT_SUPPORTED
}

#[cfg(not(target_arch = "x86_64"))]
pub fn pio_cfg_write(_addr: u32, _val: u32, _width: usize) -> Result<(), i32> {
    Err(-2) // ZX_ERR_NOT_SUPPORTED
}

#[cfg(not(target_arch = "x86_64"))]
pub fn pio_cfg_write_bdf(
    _bus: u8,
    _device: u8,
    _func: u8,
    _offset: u8,
    _val: u32,
    _width: usize,
) -> Result<(), i32> {
    Err(-2) // ZX_ERR_NOT_SUPPORTED
}

/// Calculate width mask for PCI operations
fn width_mask(width: usize) -> u32 {
    if width == 32 {
        0xffffffff
    } else {
        (1u32 << width) - 1
    }
}

/// Port I/O: Write 32-bit value to port
#[cfg(target_arch = "x86_64")]
fn out_port_32(_port: u16, _val: u32) {
    // TODO: Implement actual port I/O
    // This requires inline assembly
    unsafe {
        core::arch::asm!(
            "out dx, eax",
            in("dx") _port,
            in("eax") _val,
            options(nomem, nostack)
        );
    }
}

/// Port I/O: Read 32-bit value from port
#[cfg(target_arch = "x86_64")]
fn in_port_32(_port: u16) -> u32 {
    // TODO: Implement actual port I/O
    // This requires inline assembly
    let val: u32;
    unsafe {
        core::arch::asm!(
            "in eax, dx",
            in("dx") _port,
            out("eax") val,
            options(nomem, nostack)
        );
    }
    val
}

/// PCI device identification
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PciBdf {
    /// Bus number
    pub bus: u8,
    /// Device number
    pub device: u8,
    /// Function number
    pub func: u8,
}

impl PciBdf {
    /// Create a new BDF
    pub fn new(bus: u8, device: u8, func: u8) -> Self {
        Self { bus, device, func }
    }

    /// Convert to raw address
    pub fn to_addr(&self, offset: u8) -> u32 {
        pci_bdf_raw_addr(self.bus, self.device, self.func, offset)
    }

    /// Read 8-bit value
    pub fn read_u8(&self, offset: u8) -> Result<u8, i32> {
        let val = pio_cfg_read_bdf(self.bus, self.device, self.func, offset, 8)?;
        Ok(val as u8)
    }

    /// Read 16-bit value
    pub fn read_u16(&self, offset: u8) -> Result<u16, i32> {
        let val = pio_cfg_read_bdf(self.bus, self.device, self.func, offset, 16)?;
        Ok(val as u16)
    }

    /// Read 32-bit value
    pub fn read_u32(&self, offset: u8) -> Result<u32, i32> {
        pio_cfg_read_bdf(self.bus, self.device, self.func, offset, 32)
    }

    /// Write 8-bit value
    pub fn write_u8(&self, offset: u8, val: u8) -> Result<(), i32> {
        pio_cfg_write_bdf(self.bus, self.device, self.func, offset, val as u32, 8)
    }

    /// Write 16-bit value
    pub fn write_u16(&self, offset: u8, val: u16) -> Result<(), i32> {
        pio_cfg_write_bdf(self.bus, self.device, self.func, offset, val as u32, 16)
    }

    /// Write 32-bit value
    pub fn write_u32(&self, offset: u8, val: u32) -> Result<(), i32> {
        pio_cfg_write_bdf(self.bus, self.device, self.func, offset, val, 32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pci_bdf_raw_addr() {
        // Bus 0, Device 0, Function 0, Offset 0
        let addr = pci_bdf_raw_addr(0, 0, 0, 0);
        assert_eq!(addr, 0x80000000);

        // Bus 1, Device 2, Function 3, Offset 0x40
        let addr = pci_bdf_raw_addr(1, 2, 3, 0x40);
        assert_eq!(addr, 0x80011840);
    }

    #[test]
    fn test_width_mask() {
        assert_eq!(width_mask(8), 0xFF);
        assert_eq!(width_mask(16), 0xFFFF);
        assert_eq!(width_mask(32), 0xFFFFFFFF);
    }

    #[test]
    fn test_pci_bdf() {
        let bdf = PciBdf::new(0, 0x1f, 0);
        assert_eq!(bdf.bus, 0);
        assert_eq!(bdf.device, 0x1f);
        assert_eq!(bdf.func, 0);
    }
}
