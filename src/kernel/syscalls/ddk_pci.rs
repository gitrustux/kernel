// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! PCI DDK System Calls
//!
//! This module implements PCI-related DDK system calls.
//!
//! # Syscalls Implemented
//!
//! - `rx_pci_add_subtract_io_range` - Add/subtract IO range
//! - `rx_pci_init` - Initialize PCI subsystem
//! - `rx_pci_get_nth_device` - Get Nth PCI device
//! - `rx_pci_config_read` - Read PCI config space
//! - `rx_pci_config_write` - Write PCI config space
//! - `rx_pci_cfg_pio_rw` - PCI config PIO read/write (x86)
//! - `rx_pci_enable_bus_master` - Enable/disable bus master
//! - `rx_pci_reset_device` - Reset PCI device
//! - `rx_pci_get_bar` - Get PCI BAR
//! - `rx_pci_map_interrupt` - Map PCI interrupt
//! - `rx_pci_query_irq_mode` - Query IRQ mode
//! - `rx_pci_set_irq_mode` - Set IRQ mode

#![no_std]

use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info, log_warn};

/// ============================================================================
/// PCI Constants
/// ============================================================================

/// Maximum PCI init argument size
const PCI_INIT_ARG_MAX_SIZE: u32 = 4096;

/// PCI BDF address format: bus (8 bits) | device (5 bits) | function (3 bits)
pub const PCI_MAX_BUSES: u32 = 256;
pub const PCI_MAX_DEVICES: u32 = 32;
pub const PCI_MAX_FUNCTIONS: u32 = 8;

/// PCI config space types
pub mod pci_cfg_space_type {
    /// MMIO config space
    pub const MMIO: u32 = 0;

    /// PIO config space
    pub const PIO: u32 = 1;

    /// DesignWare root bridge (MMIO)
    pub const DW_ROOT: u32 = 2;

    /// DesignWare downstream (MMIO)
    pub const DW_DS: u32 = 3;
}

/// No IRQ mapping marker
pub const PCI_NO_IRQ_MAPPING: u32 = 0xFFFFFFFF;

/// Standard PCI config header size
pub const PCI_STANDARD_CONFIG_HDR_SIZE: u16 = 64;

/// PCIe extended config space size
pub const PCIE_EXTENDED_CONFIG_SIZE: u16 = 4096;

/// PCIe base config space size
pub const PCIE_BASE_CONFIG_SIZE: u16 = 256;

/// Maximum BAR registers
pub const PCIE_MAX_BAR_REGS: u32 = 6;

/// ============================================================================
/// PCI IRQ Modes
/// ============================================================================

/// PCI IRQ modes
pub mod pci_irq_mode {
    /// Legacy IRQ mode
    pub const LEGACY: u32 = 0;

    /// MSI (Message Signaled Interrupts)
    pub const MSI: u32 = 1;

    /// MSI-X (extended MSI)
    pub const MSI_X: u32 = 2;
}

/// ============================================================================
/// PCI BAR Types
/// ============================================================================

/// PCI BAR types
pub mod pci_bar_type {
    /// MMIO BAR
    pub const MMIO: u32 = 0;

    /// PIO BAR
    pub const PIO: u32 = 1;
}

/// ============================================================================
/// PCI Device Info
/// ============================================================================

/// PCI device information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PciDeviceInfo {
    /// Vendor ID
    pub vendor_id: u16,

    /// Device ID
    pub device_id: u16,

    /// Base Class
    pub base_class: u8,

    /// Sub Class
    pub sub_class: u8,

    /// Program Interface
    pub prog_if: u8,

    /// Revision ID
    pub revision_id: u8,

    /// Bus ID
    pub bus_id: u8,

    /// Device ID
    pub dev_id: u8,

    /// Function ID
    pub func_id: u8,

    /// IRQs
    pub irqs: [u32; 6],
}

/// ============================================================================
/// PCI BAR
/// ============================================================================

/// PCI BAR information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PciBar {
    /// Size of BAR
    pub size: u64,

    /// Type of BAR (MMIO or PIO)
    pub bar_type: u32,

    /// Reserved
    pub reserved: u32,

    /// Address (for PIO BARs)
    pub addr: u32,
}

/// ============================================================================
/// PCI Init Argument
/// ============================================================================

/// PCI address window
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PciAddrWindow {
    /// Base address
    pub base: u64,

    /// Size
    pub size: u64,

    /// Bus start
    pub bus_start: u8,

    /// Bus end
    pub bus_end: u8,

    /// Config space type
    pub cfg_space_type: u32,

    /// Has ECAM
    pub has_ecam: bool,
}

/// PCI IRQ info
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PciIrqInfo {
    /// Global IRQ
    pub global_irq: u32,

    /// Level triggered
    pub level_triggered: bool,

    /// Active high
    pub active_high: bool,
}

/// PCI init argument (variable length)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PciInitArg {
    /// Address window count
    pub addr_window_count: u32,

    /// Number of IRQs
    pub num_irqs: u32,

    /// Reserved
    pub reserved: [u64; 2],

    // Address windows (variable length array) - commented out
    // pub addr_windows: [PciAddrWindow; addr_window_count]
    // pub irqs: [PciIrqInfo; num_irqs]
    // pub dev_pin_to_global_irq: [u32; 32][8][4]
}

/// ============================================================================
/// Syscall: PCI Add/Subtract IO Range
/// ============================================================================

/// Add or subtract IO range from PCI
///
/// # Arguments
///
/// * `handle` - Root resource handle
/// * `mmio` - true for MMIO, false for PIO
/// * `base` - Base address
/// * `len` - Length of range
/// * `add` - true to add, false to subtract
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_add_subtract_io_range_impl(
    handle: u32,
    mmio: bool,
    base: u64,
    len: u64,
    add: bool,
) -> SyscallRet {
    log_debug!(
        "sys_pci_add_subtract_io_range: handle={:#x} mmio={} base={:#x} len={:#x} add={}",
        handle,
        mmio,
        base,
        len,
        add
    );

    // TODO: Validate resource handle
    // TODO: Get PCIe bus driver
    // TODO: Add/subtract bus region

    log_warn!("sys_pci_add_subtract_io_range: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Init
/// ============================================================================

/// Initialize PCI subsystem
///
/// # Arguments
///
/// * `handle` - Root resource handle
/// * `init_buf` - User pointer to init buffer
/// * `len` - Length of init buffer
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_init_impl(
    handle: u32,
    init_buf: usize,
    len: u32,
) -> SyscallRet {
    log_debug!(
        "sys_pci_init: handle={:#x} buf={:#x} len={}",
        handle,
        init_buf,
        len
    );

    if len < core::mem::size_of::<PciInitArg>() as u32 || len > PCI_INIT_ARG_MAX_SIZE {
        log_error!("sys_pci_init: invalid len");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Validate resource handle
    // TODO: Copy init buffer from user
    // TODO: Parse address windows
    // TODO: Configure interrupts
    // TODO: Create ECAM regions
    // TODO: Add root complex
    // TODO: Start bus driver

    log_info!("sys_pci_init: PCI init stub");
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: PCI Get Nth Device
/// ============================================================================

/// Get Nth PCI device
///
/// # Arguments
///
/// * `handle` - Root resource handle
/// * `index` - Device index
/// * `info_out` - User pointer to store device info
/// * `handle_out` - User pointer to store device handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_get_nth_device_impl(
    handle: u32,
    index: u32,
    info_out: usize,
    handle_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_pci_get_nth_device: handle={:#x} index={}",
        handle,
        index
    );

    // TODO: Validate resource handle
    // TODO: Get PCI device by index
    // TODO: Return device info and handle

    log_warn!("sys_pci_get_nth_device: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Config Read
/// ============================================================================

/// Read from PCI config space
///
/// # Arguments
///
/// * `handle` - PCI device handle
/// * `offset` - Register offset
/// * `width` - Width (1, 2, or 4 bytes)
/// * `val_out` - User pointer to store value
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_config_read_impl(
    handle: u32,
    offset: u16,
    width: usize,
    val_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_pci_config_read: handle={:#x} offset={:#x} width={}",
        handle,
        offset,
        width
    );

    if width != 1 && width != 2 && width != 4 {
        log_error!("sys_pci_config_read: invalid width");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Get PCI device from handle
    // TODO: Read config space
    // TODO: Return value

    log_warn!("sys_pci_config_read: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Config Write
/// ============================================================================

/// Write to PCI config space
///
/// # Arguments
///
/// * `handle` - PCI device handle
/// * `offset` - Register offset
/// * `width` - Width (1, 2, or 4 bytes)
/// * `val` - Value to write
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_config_write_impl(
    handle: u32,
    offset: u16,
    width: usize,
    val: u32,
) -> SyscallRet {
    log_debug!(
        "sys_pci_config_write: handle={:#x} offset={:#x} width={} val={:#x}",
        handle,
        offset,
        width,
        val
    );

    if width != 1 && width != 2 && width != 4 {
        log_error!("sys_pci_config_write: invalid width");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Get PCI device from handle
    // TODO: Validate offset (not in standard header)
    // TODO: Write config space

    log_warn!("sys_pci_config_write: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Config PIO RW (x86)
/// ============================================================================

/// PCI config PIO read/write (x86 only)
///
/// # Arguments
///
/// * `handle` - Root resource handle
/// * `bus` - Bus number
/// * `dev` - Device number
/// * `func` - Function number
/// * `offset` - Register offset
/// * `val` - User pointer to value (in/out)
/// * `width` - Width (1, 2, or 4 bytes)
/// * `write` - true for write, false for read
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_cfg_pio_rw_impl(
    handle: u32,
    bus: u8,
    dev: u8,
    func: u8,
    offset: u8,
    val: usize,
    width: usize,
    write: bool,
) -> SyscallRet {
    log_debug!(
        "sys_pci_cfg_pio_rw: handle={:#x} {}:{}.{} offset={:#x} width={} write={}",
        handle,
        bus,
        dev,
        func,
        offset,
        width,
        write
    );

    #[cfg(target_arch = "x86_64")]
    {
        // TODO: Validate resource handle
        // TODO: Perform PIO read/write

        log_warn!("sys_pci_cfg_pio_rw: not implemented (stub)");
        err_to_ret(RX_ERR_NOT_SUPPORTED)
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        log_error!("sys_pci_cfg_pio_rw: not supported on non-x86");
        err_to_ret(RX_ERR_NOT_SUPPORTED)
    }
}

/// ============================================================================
/// Syscall: PCI Enable Bus Master
/// ============================================================================

/// Enable or disable PCI bus master
///
/// # Arguments
///
/// * `dev_handle` - PCI device handle
/// * `enable` - true to enable, false to disable
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_enable_bus_master_impl(
    dev_handle: u32,
    enable: bool,
) -> SyscallRet {
    log_debug!(
        "sys_pci_enable_bus_master: handle={:#x} enable={}",
        dev_handle,
        enable
    );

    // TODO: Get PCI device from handle
    // TODO: Enable/disable bus master

    log_warn!("sys_pci_enable_bus_master: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Reset Device
/// ============================================================================

/// Reset PCI device
///
/// # Arguments
///
/// * `dev_handle` - PCI device handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_reset_device_impl(dev_handle: u32) -> SyscallRet {
    log_debug!("sys_pci_reset_device: handle={:#x}", dev_handle);

    // TODO: Get PCI device from handle
    // TODO: Reset device

    log_warn!("sys_pci_reset_device: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Get BAR
/// ============================================================================

/// Get PCI BAR
///
/// # Arguments
///
/// * `dev_handle` - PCI device handle
/// * `bar_num` - BAR number
/// * `bar_out` - User pointer to store BAR info
/// * `handle_out` - User pointer to store VMO handle (for MMIO BARs)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_get_bar_impl(
    dev_handle: u32,
    bar_num: u32,
    bar_out: usize,
    handle_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_pci_get_bar: handle={:#x} bar={}",
        dev_handle,
        bar_num
    );

    if bar_num >= PCIE_MAX_BAR_REGS {
        log_error!("sys_pci_get_bar: invalid bar_num");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Get PCI device from handle
    // TODO: Get BAR info
    // TODO: Create VMO for MMIO BARs
    // TODO: Return BAR info and handle

    log_warn!("sys_pci_get_bar: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Map Interrupt
/// ============================================================================

/// Map PCI interrupt
///
/// # Arguments
///
/// * `dev_handle` - PCI device handle
/// * `which_irq` - IRQ number (-1 for legacy)
/// * `handle_out` - User pointer to store interrupt handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_map_interrupt_impl(
    dev_handle: u32,
    which_irq: i32,
    handle_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_pci_map_interrupt: handle={:#x} irq={}",
        dev_handle,
        which_irq
    );

    // TODO: Get PCI device from handle
    // TODO: Map interrupt
    // TODO: Return interrupt handle

    log_warn!("sys_pci_map_interrupt: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Query IRQ Mode
/// ============================================================================

/// Query PCI IRQ mode capabilities
///
/// # Arguments
///
/// * `dev_handle` - PCI device handle
/// * `mode` - IRQ mode to query
/// * `max_irqs_out` - User pointer to store max IRQs
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_query_irq_mode_impl(
    dev_handle: u32,
    mode: u32,
    max_irqs_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_pci_query_irq_mode: handle={:#x} mode={}",
        dev_handle,
        mode
    );

    // TODO: Get PCI device from handle
    // TODO: Query IRQ mode capabilities
    // TODO: Return max IRQs

    log_warn!("sys_pci_query_irq_mode: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Syscall: PCI Set IRQ Mode
/// ============================================================================

/// Set PCI IRQ mode
///
/// # Arguments
///
/// * `dev_handle` - PCI device handle
/// * `mode` - IRQ mode to set
/// * `requested_irq_count` - Number of IRQs to request
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pci_set_irq_mode_impl(
    dev_handle: u32,
    mode: u32,
    requested_irq_count: u32,
) -> SyscallRet {
    log_debug!(
        "sys_pci_set_irq_mode: handle={:#x} mode={} count={}",
        dev_handle,
        mode,
        requested_irq_count
    );

    // TODO: Get PCI device from handle
    // TODO: Set IRQ mode

    log_warn!("sys_pci_set_irq_mode: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get PCI DDK statistics
pub fn get_stats() -> PciStats {
    PciStats {
        total_pci_init: 0,      // TODO: Track
        total_config_ops: 0,    // TODO: Track
        total_irq_ops: 0,       // TODO: Track
        total_devices: 0,       // TODO: Track
    }
}

/// PCI DDK statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PciStats {
    /// Total PCI init operations
    pub total_pci_init: u64,

    /// Total config read/write operations
    pub total_config_ops: u64,

    /// Total IRQ operations
    pub total_irq_ops: u64,

    /// Total PCI devices
    pub total_devices: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the PCI DDK subsystem
pub fn init() {
    log_info!("PCI DDK subsystem initialized");
    log_info!("  Max init arg size: {}", PCI_INIT_ARG_MAX_SIZE);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pci_cfg_space_type_consts() {
        assert_eq!(pci_cfg_space_type::MMIO, 0);
        assert_eq!(pci_cfg_space_type::PIO, 1);
        assert_eq!(pci_cfg_space_type::DW_ROOT, 2);
        assert_eq!(pci_cfg_space_type::DW_DS, 3);
    }

    #[test]
    fn test_pci_irq_mode_consts() {
        assert_eq!(pci_irq_mode::LEGACY, 0);
        assert_eq!(pci_irq_mode::MSI, 1);
        assert_eq!(pci_irq_mode::MSI_X, 2);
    }

    #[test]
    fn test_pci_bar_type_consts() {
        assert_eq!(pci_bar_type::MMIO, 0);
        assert_eq!(pci_bar_type::PIO, 1);
    }

    #[test]
    fn test_pci_constants() {
        assert_eq!(PCI_MAX_BUSES, 256);
        assert_eq!(PCI_MAX_DEVICES, 32);
        assert_eq!(PCI_MAX_FUNCTIONS, 8);
        assert_eq!(PCI_NO_IRQ_MAPPING, 0xFFFFFFFF);
    }

    #[test]
    fn test_pci_device_info_size() {
        assert_eq!(core::mem::size_of::<PciDeviceInfo>(), 20);
    }

    #[test]
    fn test_pci_bar_size() {
        assert_eq!(core::mem::size_of::<PciBar>(), 24);
    }
}
