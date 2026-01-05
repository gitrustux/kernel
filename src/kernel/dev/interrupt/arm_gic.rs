// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM Generic Interrupt Controller (GIC)
//!
//! The GIC is the standard interrupt controller for ARMv8 systems. It handles:
//!
//! - **SGI** (Software Generated Interrupt): IDs 0-15, per-CPU interrupts
//! - **PPI** (Private Peripheral Interrupt): IDs 16-31, per-CPU peripherals
//! - **SPI** (Shared Peripheral Interrupt): IDs 32+, shared devices
//!
//! # Architecture
//!
//! The GIC consists of:
//! - **Distributor (GICD)**: Routes interrupts to CPU interfaces
//! - **CPU Interface (GICC)**: Per-CPU interface for interrupt acknowledgment
//! - **Virtual Interface (GICH/ICV)**: For hypervisor virtualization (GICv2/GICv3)
//!
//! # Versions
//!
//! - **GICv2**: Most common, excellent QEMU support
//! - **GICv3**: Newer features, ITS support
//! - **GICv4**: Virtualization enhancements
//!
//! # QEMU Support
//!
//! QEMU ARM virt uses GICv2 by default:
//! ```bash
//! qemu-system-aarch64 -M virt -cpu cortex-a57 -m 1G
//! ```

#![no_std]

pub mod gicv2;

// Re-export GICv2 for QEMU testing
pub use gicv2::*;

/// Interrupt trigger mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptTriggerMode {
    Edge = 0,
    Level = 1,
}

/// Interrupt polarity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptPolarity {
    ActiveHigh = 0,
    ActiveLow = 1,
}

/// End of interrupt action
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptEoi {
    Deactivate = 0,
    KeepActive = 1,
}

/// Base SPI interrupt ID
pub const GIC_BASE_SPI: u32 = 32;

/// Maximum number of interrupts
pub const MAX_INT: u32 = 1024;

/// SGI target filter flags
pub const ARM_GIC_SGI_FLAG_TARGET_FILTER_MASK: u32 = 0x3 << 24;
pub const ARM_GIC_SGI_FLAG_TARGET_FILTER_LIST: u32 = 0x0 << 24;
pub const ARM_GIC_SGI_FLAG_TARGET_FILTER_ALL: u32 = 0x1 << 24;
pub const ARM_GIC_SGI_FLAG_TARGET_FILTER_SELF: u32 = 0x2 << 24;
pub const ARM_GIC_SGI_FLAG_NS: u32 = 1 << 15;

/// IPI (Inter-Processor Interrupt) types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpIpi {
    Generic = 0,
    Reschedule = 1,
    Interrupt = 2,
    Halt = 3,
}

/// Interrupt handler function type
pub type InterruptHandler = fn(arg: usize) -> InterruptEoi;

/// Register an interrupt handler for a specific IRQ
///
/// # Safety
///
/// Must be called with proper synchronization
pub unsafe fn register_interrupt_handler(irq: u32, handler: InterruptHandler, arg: usize) {
    // TODO: Implement interrupt handler registration
    let _ = irq;
    let _ = handler;
    let _ = arg;
}

/// Send a Software Generated Interrupt (SGI) to target CPUs
///
/// # Arguments
///
/// * `sgi_num` - SGI number (0-15)
/// * `flags` - Target filter and NS flag
/// * `cpu_mask` - Bitmask of target CPUs (one bit per CPU, up to 8 CPUs)
///
/// # Returns
///
/// Returns 0 on success, or a negative error code on failure
pub fn send_sgi(sgi_num: u32, flags: u32, cpu_mask: u32) -> i32 {
    if sgi_num >= 16 {
        return -1;
    }

    #[cfg(target_arch = "aarch64")]
    {
        gicv2::send_sgi(sgi_num, flags, cpu_mask)
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = flags;
        let _ = cpu_mask;
        -1
    }
}

/// Mask (disable) an interrupt
pub fn mask_interrupt(irq: u32) -> i32 {
    #[cfg(target_arch = "aarch64")]
    {
        gicv2::mask_interrupt(irq)
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = irq;
        -1
    }
}

/// Unmask (enable) an interrupt
pub fn unmask_interrupt(irq: u32) -> i32 {
    #[cfg(target_arch = "aarch64")]
    {
        gicv2::unmask_interrupt(irq)
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = irq;
        -1
    }
}

/// Get the maximum interrupt ID supported
pub fn get_max_irq() -> u32 {
    #[cfg(target_arch = "aarch64")]
    {
        gicv2::get_max_irq()
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        32
    }
}

/// Initialize per-CPU interrupt controller (early, before full init)
pub fn init_percpu_early() {
    #[cfg(target_arch = "aarch64")]
    {
        gicv2::init_percpu_early();
    }
}

/// Initialize per-CPU interrupt controller (full initialization)
pub fn init_percpu() {
    #[cfg(target_arch = "aarch64")]
    {
        gicv2::init_percpu();
    }
}
