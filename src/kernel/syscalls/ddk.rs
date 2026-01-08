// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Device Driver Kit (DDK) System Calls
//!
//! This module implements DDK-related system calls for device drivers.
//!
//! # Syscalls Implemented
//!
//! - `rx_vmo_create_contiguous` - Create contiguous VMO for DMA
//! - `rx_vmo_create_physical` - Create physical VMO for MMIO
//! - `rx_framebuffer_get_info` - Get framebuffer info
//! - `rx_framebuffer_set_range` - Set framebuffer range
//! - `rx_iommu_create` - Create IOMMU
//! - `rx_ioports_request` - Request I/O port access (x86)
//! - `rx_pc_firmware_tables` - Get PC firmware tables (x86)
//! - `rx_bti_create` - Create Bus Transaction Initiator
//! - `rx_bti_pin` - Pin VMO for DMA
//! - `rx_bti_release_quarantine` - Release BTI quarantine
//! - `rx_pmt_unpin` - Unpin pinned memory
//! - `rx_interrupt_create` - Create interrupt
//! - `rx_interrupt_bind` - Bind interrupt to port
//! - `rx_interrupt_bind_vcpu` - Bind interrupt to VCPU
//! - `rx_interrupt_ack` - Acknowledge interrupt
//! - `rx_interrupt_wait` - Wait for interrupt
//! - `rx_interrupt_destroy` - Destroy interrupt
//! - `rx_interrupt_trigger` - Trigger virtual interrupt
//! - `rx_smc_call` - SMC call (ARM)


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
/// DDK Constants
/// ============================================================================

/// Page size shift (for 4KB pages)
const PAGE_SIZE_SHIFT: u32 = 12;

/// Maximum IOMMU descriptor length
const IOMMU_MAX_DESC_LEN: usize = 256;

/// ============================================================================
/// BTI (Bus Transaction Initiator) Options
/// ============================================================================

/// BTI permission flags
pub mod bti_perm {
    /// Read permission
    pub const READ: u32 = 0x01;

    /// Write permission
    pub const WRITE: u32 = 0x02;

    /// Execute permission
    pub const EXECUTE: u32 = 0x04;

    /// Compress results
    pub const COMPRESS: u32 = 0x08;

    /// Contiguous memory
    pub const CONTIGUOUS: u32 = 0x10;
}

/// ============================================================================
/// Interrupt Options
/// ============================================================================

/// Interrupt flags
pub mod interrupt_flags {
    /// Virtual interrupt
    pub const VIRTUAL: u32 = 0x01;

    /// Remapped interrupt
    pub const REMAPPED: u32 = 0x02;
}

/// ============================================================================
/// IOMMU Types
/// ============================================================================

/// IOMMU types
pub mod iommu_type {
    /// Intel IOMMU (VT-d)
    pub const INTEL: u32 = 1;

    /// ARM SMMU
    pub const ARM_SMMU: u32 = 2;

    /// Broadcom SMMU
    pub const BROADCOM: u32 = 3;
}

/// ============================================================================
/// VMO Create Contiguous
/// ============================================================================

/// Create contiguous VMO for DMA
///
/// # Arguments
///
/// * `bti_handle` - BTI handle
/// * `size` - Size of VMO
/// * `alignment_log2` - Log2 of alignment
/// * `vmo_out` - User pointer to store VMO handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_vmo_create_contiguous_impl(
    bti_handle: u32,
    size: usize,
    mut alignment_log2: u32,
    vmo_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_vmo_create_contiguous: bti={:#x} size={} alignment={}",
        bti_handle,
        size,
        alignment_log2
    );

    if size == 0 {
        log_error!("sys_vmo_create_contiguous: invalid size");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    if alignment_log2 == 0 {
        alignment_log2 = PAGE_SIZE_SHIFT;
    }

    if alignment_log2 < PAGE_SIZE_SHIFT || alignment_log2 >= 64 {
        log_error!("sys_vmo_create_contiguous: invalid alignment");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Validate BTI handle
    // TODO: Create contiguous VMO
    // TODO: Return VMO handle

    // For now, return a stub handle
    let vmo_handle = 1000u32;
    let user_ptr = UserPtr::<u8>::new(vmo_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &vmo_handle as *const u32 as *const u8, 4) {
            log_error!("sys_vmo_create_contiguous: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_vmo_create_contiguous: success handle={:#x}", vmo_handle);
    ok_to_ret(0)
}

/// ============================================================================
/// VMO Create Physical
/// ============================================================================

/// Create physical VMO for MMIO
///
/// # Arguments
///
/// * `rsrc_handle` - Resource handle
/// * `paddr` - Physical address
/// * `size` - Size of region
/// * `vmo_out` - User pointer to store VMO handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_vmo_create_physical_impl(
    rsrc_handle: u32,
    paddr: u64,
    size: usize,
    vmo_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_vmo_create_physical: rsrc={:#x} paddr={:#x} size={}",
        rsrc_handle,
        paddr,
        size
    );

    // TODO: Validate resource handle for MMIO region
    // TODO: Create physical VMO
    // TODO: Return VMO handle

    // For now, return a stub handle
    let vmo_handle = 1001u32;
    let user_ptr = UserPtr::<u8>::new(vmo_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &vmo_handle as *const u32 as *const u8, 4) {
            log_error!("sys_vmo_create_physical: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_vmo_create_physical: success handle={:#x}", vmo_handle);
    ok_to_ret(0)
}

/// ============================================================================
/// Framebuffer Get Info
/// ============================================================================

/// Get framebuffer information (x86 only)
///
/// # Arguments
///
/// * `handle` - Root resource handle
/// * `format_out` - User pointer to store format
/// * `width_out` - User pointer to store width
/// * `height_out` - User pointer to store height
/// * `stride_out` - User pointer to store stride
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_framebuffer_get_info_impl(
    handle: u32,
    format_out: usize,
    width_out: usize,
    height_out: usize,
    stride_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_framebuffer_get_info: handle={:#x}",
        handle
    );

    // TODO: Validate resource handle
    // TODO: Implement for x86 (bootloader framebuffer)

    log_warn!("sys_framebuffer_get_info: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Framebuffer Set Range
/// ============================================================================

/// Set framebuffer range
///
/// # Arguments
///
/// * `rsrc_handle` - Root resource handle
/// * `vmo_handle` - VMO handle for framebuffer
/// * `len` - Length of framebuffer
/// * `format` - Pixel format
/// * `width` - Width in pixels
/// * `height` - Height in pixels
/// * `stride` - Stride in bytes
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_framebuffer_set_range_impl(
    rsrc_handle: u32,
    vmo_handle: u32,
    len: u32,
    format: u32,
    width: u32,
    height: u32,
    stride: u32,
) -> SyscallRet {
    log_debug!(
        "sys_framebuffer_set_range: rsrc={:#x} vmo={:#x} {}x{} stride={}",
        rsrc_handle,
        vmo_handle,
        width,
        height,
        stride
    );

    // TODO: Validate resource handle
    // TODO: Set framebuffer VMO
    // TODO: Update display info

    if vmo_handle == 0 {
        log_info!("sys_framebuffer_set_range: clearing framebuffer");
    }

    ok_to_ret(0)
}

/// ============================================================================
/// IOMMU Create
/// ============================================================================

/// Create IOMMU
///
/// # Arguments
///
/// * `resource` - Root resource handle
/// * `type` - IOMMU type
/// * `desc` - User pointer to IOMMU descriptor
/// * `desc_size` - Descriptor size
/// * `iommu_out` - User pointer to store IOMMU handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_iommu_create_impl(
    resource: u32,
    type_: u32,
    desc: usize,
    desc_size: usize,
    iommu_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_iommu_create: resource={:#x} type={} desc_size={}",
        resource,
        type_,
        desc_size
    );

    if desc_size > IOMMU_MAX_DESC_LEN {
        log_error!("sys_iommu_create: desc too large");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Validate resource handle
    // TODO: Copy descriptor from user
    // TODO: Create IOMMU
    // TODO: Return IOMMU handle

    // For now, return a stub handle
    let iommu_handle = 2000u32;
    let user_ptr = UserPtr::<u8>::new(iommu_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &iommu_handle as *const u32 as *const u8, 4) {
            log_error!("sys_iommu_create: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_iommu_create: success handle={:#x}", iommu_handle);
    ok_to_ret(0)
}

/// ============================================================================
/// I/O Ports Request (x86)
/// ============================================================================

/// Request I/O port access (x86 only)
///
/// # Arguments
///
/// * `rsrc_handle` - Resource handle
/// * `io_addr` - I/O port address
/// * `len` - Number of ports
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_ioports_request_impl(
    rsrc_handle: u32,
    io_addr: u16,
    len: u32,
) -> SyscallRet {
    log_debug!(
        "sys_ioports_request: rsrc={:#x} addr={:#x} len={}",
        rsrc_handle,
        io_addr,
        len
    );

    // TODO: Validate resource handle for I/O port range
    // TODO: Set I/O bitmap for current process

    log_warn!("sys_ioports_request: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// PC Firmware Tables (x86)
/// ============================================================================

/// Get PC firmware tables (x86 only)
///
/// # Arguments
///
/// * `rsrc_handle` - Root resource handle
/// * `acpi_rsdp_out` - User pointer to store ACPI RSDP address
/// * `smbios_out` - User pointer to store SMBIOS address
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pc_firmware_tables_impl(
    rsrc_handle: u32,
    acpi_rsdp_out: usize,
    smbios_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_pc_firmware_tables: rsrc={:#x}",
        rsrc_handle
    );

    // TODO: Validate resource handle
    // TODO: Return ACPI RSDP and SMBIOS addresses

    log_warn!("sys_pc_firmware_tables: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// BTI Create
/// ============================================================================

/// Create Bus Transaction Initiator
///
/// # Arguments
///
/// * `iommu_handle` - IOMMU handle
/// * `options` - Options (must be 0)
/// * `bti_id` - BTI ID
/// * `bti_out` - User pointer to store BTI handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_bti_create_impl(
    iommu_handle: u32,
    options: u32,
    bti_id: u64,
    bti_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_bti_create: iommu={:#x} options={:#x} bti_id={:#x}",
        iommu_handle,
        options,
        bti_id
    );

    if options != 0 {
        log_error!("sys_bti_create: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Get IOMMU from handle
    // TODO: Create BTI
    // TODO: Return BTI handle

    // For now, return a stub handle
    let bti_handle = 3000u32;
    let user_ptr = UserPtr::<u8>::new(bti_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &bti_handle as *const u32 as *const u8, 4) {
            log_error!("sys_bti_create: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_bti_create: success handle={:#x}", bti_handle);
    ok_to_ret(0)
}

/// ============================================================================
/// BTI Pin
/// ============================================================================

/// Pin VMO for DMA
///
/// # Arguments
///
/// * `bti_handle` - BTI handle
/// * `options` - Options (permissions)
/// * `vmo_handle` - VMO handle
/// * `offset` - Offset in VMO
/// * `size` - Size to pin
/// * `addrs_out` - User pointer to store physical addresses
/// * `addrs_count` - Number of addresses
/// * `pmt_out` - User pointer to store PMT handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_bti_pin_impl(
    bti_handle: u32,
    options: u32,
    vmo_handle: u32,
    offset: u64,
    size: u64,
    addrs_out: usize,
    addrs_count: usize,
    pmt_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_bti_pin: bti={:#x} options={:#x} vmo={:#x} offset={:#x} size={:#x}",
        bti_handle,
        options,
        vmo_handle,
        offset,
        size
    );

    // TODO: Get BTI from handle
    // TODO: Validate offset and size are page-aligned
    // TODO: Get VMO from handle and validate rights
    // TODO: Parse options and validate permissions
    // TODO: Pin VMO
    // TODO: Encode addresses
    // TODO: Return addresses and PMT handle

    log_warn!("sys_bti_pin: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// BTI Release Quarantine
/// ============================================================================

/// Release BTI quarantine
///
/// # Arguments
///
/// * `bti_handle` - BTI handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_bti_release_quarantine_impl(bti_handle: u32) -> SyscallRet {
    log_debug!("sys_bti_release_quarantine: bti={:#x}", bti_handle);

    // TODO: Get BTI from handle
    // TODO: Release quarantine

    log_warn!("sys_bti_release_quarantine: not implemented (stub)");
    ok_to_ret(0)
}

/// ============================================================================
/// PMT Unpin
/// ============================================================================

/// Unpin pinned memory
///
/// # Arguments
///
/// * `pmt_handle` - PMT handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_pmt_unpin_impl(pmt_handle: u32) -> SyscallRet {
    log_debug!("sys_pmt_unpin: pmt={:#x}", pmt_handle);

    // TODO: Get PMT from handle
    // TODO: Mark as unpinned

    log_warn!("sys_pmt_unpin: not implemented (stub)");
    ok_to_ret(0)
}

/// ============================================================================
/// Interrupt Create
/// ============================================================================

/// Create interrupt
///
/// # Arguments
///
/// * `src_obj` - Resource handle (for physical interrupts)
/// * `src_num` - Interrupt number
/// * `options` - Options
/// * `handle_out` - User pointer to store interrupt handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_interrupt_create_impl(
    src_obj: u32,
    src_num: u32,
    options: u32,
    handle_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_interrupt_create: src={:#x} num={} options={:#x}",
        src_obj,
        src_num,
        options
    );

    // Resource not required for virtual interrupts
    if options & interrupt_flags::VIRTUAL == 0 {
        // TODO: Validate resource handle for IRQ
        log_debug!("sys_interrupt_create: physical interrupt");
    } else {
        log_debug!("sys_interrupt_create: virtual interrupt");
    }

    // TODO: Create interrupt
    // TODO: Return interrupt handle

    // For now, return a stub handle
    let irq_handle = 4000u32;
    let user_ptr = UserPtr::<u8>::new(handle_out);
    unsafe {
        if let Err(err) = copy_to_user(user_ptr, &irq_handle as *const u32 as *const u8, 4) {
            log_error!("sys_interrupt_create: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_interrupt_create: success handle={:#x}", irq_handle);
    ok_to_ret(0)
}

/// ============================================================================
/// Interrupt Bind
/// ============================================================================

/// Bind interrupt to port
///
/// # Arguments
///
/// * `handle` - Interrupt handle
/// * `port_handle` - Port handle
/// * `key` - Key for port packets
/// * `options` - Options (must be 0)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_interrupt_bind_impl(
    handle: u32,
    port_handle: u32,
    key: u64,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_interrupt_bind: handle={:#x} port={:#x} key={:#x}",
        handle,
        port_handle,
        key
    );

    if options != 0 {
        log_error!("sys_interrupt_bind: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Get interrupt from handle
    // TODO: Get port from handle
    // TODO: Bind interrupt to port

    log_warn!("sys_interrupt_bind: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Interrupt Bind VCPU
/// ============================================================================

/// Bind interrupt to VCPU
///
/// # Arguments
///
/// * `handle` - Interrupt handle
/// * `vcpu_handle` - VCPU handle
/// * `options` - Options (must be 0)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_interrupt_bind_vcpu_impl(
    handle: u32,
    vcpu_handle: u32,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_interrupt_bind_vcpu: handle={:#x} vcpu={:#x}",
        handle,
        vcpu_handle
    );

    // TODO: Get interrupt from handle
    // TODO: Get VCPU from handle
    // TODO: Bind interrupt to VCPU

    log_warn!("sys_interrupt_bind_vcpu: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Interrupt Ack
/// ============================================================================

/// Acknowledge interrupt
///
/// # Arguments
///
/// * `handle` - Interrupt handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_interrupt_ack_impl(handle: u32) -> SyscallRet {
    log_debug!("sys_interrupt_ack: handle={:#x}", handle);

    // TODO: Get interrupt from handle
    // TODO: Acknowledge interrupt

    log_warn!("sys_interrupt_ack: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Interrupt Wait
/// ============================================================================

/// Wait for interrupt
///
/// # Arguments
///
/// * `handle` - Interrupt handle
/// * `timestamp_out` - User pointer to store timestamp
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_interrupt_wait_impl(handle: u32, timestamp_out: usize) -> SyscallRet {
    log_debug!("sys_interrupt_wait: handle={:#x}", handle);

    // TODO: Get interrupt from handle
    // TODO: Wait for interrupt
    // TODO: Return timestamp

    log_warn!("sys_interrupt_wait: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Interrupt Destroy
/// ============================================================================

/// Destroy interrupt
///
/// # Arguments
///
/// * `handle` - Interrupt handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_interrupt_destroy_impl(handle: u32) -> SyscallRet {
    log_debug!("sys_interrupt_destroy: handle={:#x}", handle);

    // TODO: Get interrupt from handle
    // TODO: Destroy interrupt

    log_warn!("sys_interrupt_destroy: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// Interrupt Trigger
/// ============================================================================

/// Trigger virtual interrupt
///
/// # Arguments
///
/// * `handle` - Interrupt handle
/// * `options` - Options (must be 0)
/// * `timestamp` - Timestamp
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_interrupt_trigger_impl(
    handle: u32,
    options: u32,
    timestamp: u64,
) -> SyscallRet {
    log_debug!(
        "sys_interrupt_trigger: handle={:#x} timestamp={}",
        handle,
        timestamp
    );

    if options != 0 {
        log_error!("sys_interrupt_trigger: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Get interrupt from handle
    // TODO: Trigger interrupt

    log_warn!("sys_interrupt_trigger: not implemented (stub)");
    err_to_ret(RX_ERR_NOT_SUPPORTED)
}

/// ============================================================================
/// SMC Call
/// ============================================================================

/// SMC call (ARM only)
///
/// # Arguments
///
/// * `handle` - Resource handle
/// * `parameters` - User pointer to SMC parameters
/// * `result_out` - User pointer to store SMC result
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_smc_call_impl(
    handle: u32,
    parameters: usize,
    result_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_smc_call: handle={:#x} parameters={:#x}",
        handle,
        parameters
    );

    #[cfg(target_arch = "x86_64")]
    {
        // SMC not supported on x86
        log_error!("sys_smc_call: SMC not supported on x86");
        return err_to_ret(RX_ERR_NOT_SUPPORTED);
    }

    #[cfg(target_arch = "aarch64")]
    {
        // TODO: Copy parameters from user
        // TODO: Validate resource handle for SMC
        // TODO: Call arch_smc_call
        // TODO: Copy result to user

        log_warn!("sys_smc_call: not implemented (stub)");
        err_to_ret(RX_ERR_NOT_SUPPORTED)
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        log_error!("sys_smc_call: unsupported architecture");
        err_to_ret(RX_ERR_NOT_SUPPORTED)
    }
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get DDK statistics
pub fn get_stats() -> DdkStats {
    DdkStats {
        total_vmo_contiguous: 0,  // TODO: Track
        total_vmo_physical: 0,    // TODO: Track
        total_interrupts: 0,      // TODO: Track
        total_bti_ops: 0,         // TODO: Track
    }
}

/// DDK statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DdkStats {
    /// Total VMO create contiguous operations
    pub total_vmo_contiguous: u64,

    /// Total VMO create physical operations
    pub total_vmo_physical: u64,

    /// Total interrupt operations
    pub total_interrupts: u64,

    /// Total BTI operations
    pub total_bti_ops: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the DDK subsystem
pub fn init() {
    log_info!("DDK subsystem initialized");
    log_info!("  IOMMU max descriptor: {}", IOMMU_MAX_DESC_LEN);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bti_perm_consts() {
        assert_eq!(bti_perm::READ, 0x01);
        assert_eq!(bti_perm::WRITE, 0x02);
        assert_eq!(bti_perm::EXECUTE, 0x04);
        assert_eq!(bti_perm::COMPRESS, 0x08);
        assert_eq!(bti_perm::CONTIGUOUS, 0x10);
    }

    #[test]
    fn test_interrupt_flags_consts() {
        assert_eq!(interrupt_flags::VIRTUAL, 0x01);
        assert_eq!(interrupt_flags::REMAPPED, 0x02);
    }

    #[test]
    fn test_iommu_type_consts() {
        assert_eq!(iommu_type::INTEL, 1);
        assert_eq!(iommu_type::ARM_SMMU, 2);
        assert_eq!(iommu_type::BROADCOM, 3);
    }

    #[test]
    fn test_vmo_create_contiguous_invalid_size() {
        let result = sys_vmo_create_contiguous_impl(0, 0, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_vmo_create_contiguous_invalid_alignment() {
        let result = sys_vmo_create_contiguous_impl(0, 4096, 64, 0);
        assert!(result < 0);
    }
}
