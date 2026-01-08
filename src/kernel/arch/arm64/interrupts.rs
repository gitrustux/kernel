// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 interrupt controller support (GIC)
//!
//! This module provides functions for interacting with the ARM Generic
//! Interrupt Controller (GIC), including masking/unmasking IRQs,
//! sending end-of-interrupt signals, and sending inter-processor interrupts.

#![no_std]

use crate::arch::arm64;

// GICv3/GICv2 register offsets (simplified - would typically be memory-mapped)
const GICD_BASE: usize = 0x08000000;  // Distributor base (platform-specific)
const GICR_BASE: usize = 0x080A0000;  // Redistributor base (platform-specific)

// External assembly functions for GIC access
extern "C" {
    /// Read from a GIC register
    fn gic_read(reg: u32) -> u32;

    /// Write to a GIC register
    fn gic_write(reg: u32, value: u32);

    /// Send SGI (Software Generated Interrupt)
    fn gic_send_sgi(sgi_num: u32, target_filter: u32, target_list: u32);
}

/// Target filter for SGI
const GIC_SGI_TARGET_FILTER_LIST: u32 = 0;     // Target specified CPUs in target_list
const GIC_SGI_TARGET_FILTER_ALL: u32 = 1;      // Target all CPUs except sender
const GIC_SGI_TARGET_FILTER_SELF: u32 = 2;     // Target only the calling CPU

/// Mask or unmask a specific interrupt
///
/// This function controls whether a specific interrupt is enabled or disabled
/// at the interrupt controller level.
///
/// # Arguments
///
/// * `irq` - The interrupt number to mask/unmask
/// * `enable` - `true` to enable (unmask) the interrupt, `false` to disable (mask) it
///
/// # Safety
///
/// This function modifies hardware interrupt controller state. The caller must
/// ensure proper synchronization with interrupt handlers.
pub unsafe fn mask_unmask_irq(irq: u32, enable: bool) {
    // In a real implementation, this would access the GIC distributor registers
    // GICD_ISENABLERn / GICD_ICENABLERn
    //
    // For now, this is a stub that will be implemented when the GIC driver is complete

    let enable_num = irq / 32;
    let bit = 1 << (irq % 32);

    if enable {
        // GICD_ISENABLERn - Set Enable
        // gic_write(GICD_BASE + 0x100 + (enable_num * 4), bit);
        // For now, this is a stub
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    } else {
        // GICD_ICENABLERn - Clear Enable
        // gic_write(GICD_BASE + 0x180 + (enable_num * 4), bit);
        // For now, this is a stub
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

/// Send End Of Interrupt (EOI) signal
///
/// This signals to the interrupt controller that the interrupt handler
/// has completed processing the interrupt.
///
/// # Arguments
///
/// * `irq` - The interrupt number to complete
///
/// # Safety
///
/// This function modifies hardware interrupt controller state. It must only
/// be called from within the interrupt handler for the specified IRQ.
pub unsafe fn send_eoi(irq: u32) {
    // In a real implementation, this would write to the GIC CPU interface
    // GICC_EOIR or GICR_EOIR0 for GICv3
    //
    // For GICv3:
    // gic_write(GICR_BASE + GICR_EOIR0, irq);
    //
    // For now, this is a stub
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Send a Software Generated Interrupt (SGI) to target CPUs
///
/// SGIs are inter-processor interrupts (IPIs) used for CPU-to-CPU communication.
///
/// # Arguments
///
/// * `sgi_num` - The SGI number (0-15 for SGIs)
/// * `target_mask` - Bitmask of target CPUs (one bit per CPU)
///
/// # Returns
///
/// Returns 0 on success, or a negative error code on failure.
///
/// # Safety
///
/// This function sends an interrupt to other CPUs. The caller must ensure
/// proper interrupt handling is set up on the target CPUs.
pub unsafe fn send_sgi(sgi_num: u32, target_mask: u64) -> i32 {
    // SGI numbers are 0-15
    if sgi_num > 15 {
        return -1; // Invalid SGI number
    }

    // In a real implementation, this would write to the GIC distributor
    // GICD_SGIR register for GICv2 or use ICC_SGI1R_EL1 for GICv3
    //
    // For GICv3 (using system register):
    // let sgi1r_el1: u64 = (target_mask << 16) | ((sgi_num & 0xF) as u64);
    // core::arch::asm!("msr icc_sgi1r_el1, {}", in(reg) sgi1r_el1);
    //
    // For now, this is a stub
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    0 // OK
}

/// Send an SGI to a specific target CPU
///
/// # Arguments
///
/// * `sgi_num` - The SGI number (0-15 for SGIs)
/// * `target_cpu` - The target CPU number
///
/// # Returns
///
/// Returns 0 on success, or a negative error code on failure.
///
/// # Safety
///
/// This function sends an interrupt to another CPU. The caller must ensure
/// proper interrupt handling is set up on the target CPU.
pub unsafe fn send_sgi_to_cpu(sgi_num: u32, target_cpu: u32) -> i32 {
    // SGI numbers are 0-15
    if sgi_num > 15 {
        return -1; // Invalid SGI number
    }

    // In a real implementation for GICv3:
    // ICC_SGI1R_EL1 format:
    //   [63:56] - Aff3 (cluster)
    //   [55:48] - Aff2
    //   [47:40] - Aff1
    //   [39:32] - Target Aff0 (CPU within cluster)
    //   [27:24] - TargetList (for filtered group)
    //   [23:0]  - SGI number
    //
    // For now, this is a stub
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    0 // OK
}

/// Send an SGI to all CPUs except the sender
///
/// # Arguments
///
/// * `sgi_num` - The SGI number (0-15 for SGIs)
///
/// # Returns
///
/// Returns 0 on success, or a negative error code on failure.
///
/// # Safety
///
/// This function sends an interrupt to all other CPUs. The caller must ensure
/// proper interrupt handling is set up on the target CPUs.
pub unsafe fn broadcast_sgi(sgi_num: u32) -> i32 {
    // SGI numbers are 0-15
    if sgi_num > 15 {
        return -1; // Invalid SGI number
    }

    // In a real implementation for GICv3:
    // let sgi1r_el1: u64 = (1u64 << 40) | ((sgi_num & 0xF) as u64);
    // core::arch::asm!("msr icc_sgi1r_el1, {}", in(reg) sgi1r_el1);
    //
    // For now, this is a stub
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    0 // OK
}

/// Initialize the GIC for the current CPU
///
/// This must be called for each CPU during boot to enable interrupt handling.
///
/// # Safety
///
/// This function modifies hardware interrupt controller state.
pub unsafe fn init_cpu_interface() {
    // In a real implementation, this would:
    // 1. Set the priority mask for the CPU interface
    // 2. Enable group 0 and group 1 interrupts
    // 3. Configure binary point registers
    //
    // For now, this is a stub
}

/// Get the current interrupt acknowledge
///
/// This reads the interrupt acknowledge register to get the interrupt
/// number and group of the highest priority pending interrupt.
///
/// # Returns
///
/// The interrupt ID and group information.
///
/// # Safety
///
/// This function must only be called from an interrupt handler context.
pub unsafe fn get_interrupt_acknowledge() -> u32 {
    // In a real implementation, this would read GICC_IAR or GICR_IAR0
    // let iar: u32;
    // core::arch::asm!("mrs {0}, icc_iar1_el1", out(reg) iar);
    // iar

    0 // Stub - return spurious interrupt (ID 1023)
}

// ============================================================================
// Interrupt State Management
// ============================================================================

/// Check if IRQ interrupts are disabled
#[inline]
pub fn arch_ints_disabled() -> bool {
    // Read the DAIF (Disable Interrupts) register
    // Bit 7 (I) controls IRQ masking
    let daif: u64;
    unsafe {
        core::arch::asm!("mrs {}, daif", out(reg) daif);
    }
    (daif & (1 << 7)) != 0
}

/// Disable IRQ interrupts
#[inline]
pub fn arch_disable_ints() {
    unsafe {
        core::arch::asm!("msr daifset, #2"); // Set IRQ bit (bit 1 in DAIFSET)
    }
}

/// Enable IRQ interrupts
#[inline]
pub fn arch_enable_ints() {
    unsafe {
        core::arch::asm!("msr daifclr, #2"); // Clear IRQ bit (bit 1 in DAIFCLR)
    }
}

/// Save current interrupt state and return it
///
/// # Returns
///
/// The current interrupt state as a bitmask
#[inline]
pub fn arch_save_ints() -> u64 {
    let daif: u64;
    unsafe {
        core::arch::asm!("mrs {}, daif", out(reg) daif);
    }
    daif
}

/// Restore interrupt state from a previously saved value
///
/// # Arguments
///
/// * `state` - The interrupt state to restore
#[inline]
pub fn arch_restore_ints(state: u64) {
    unsafe {
        core::arch::asm!("msr daif, {}", in(reg) state);
    }
}

/// Check if FIQ interrupts are disabled
#[inline]
pub fn arch_fiqs_disabled() -> bool {
    // Read the DAIF (Disable Interrupts) register
    // Bit 6 (F) controls FIQ masking
    let daif: u64;
    unsafe {
        core::arch::asm!("mrs {}, daif", out(reg) daif);
    }
    (daif & (1 << 6)) != 0
}

/// Disable FIQ interrupts
#[inline]
pub fn arch_disable_fiqs() {
    unsafe {
        core::arch::asm!("msr daifset, #1"); // Set FIQ bit (bit 0 in DAIFSET)
    }
}

/// Enable FIQ interrupts
#[inline]
pub fn arch_enable_fiqs() {
    unsafe {
        core::arch::asm!("msr daifclr, #1"); // Clear FIQ bit (bit 0 in DAIFCLR)
    }
}

// ============================================================================
// Interrupt Handler Support (LK Compatibility)
// ============================================================================

/// Saved state during interrupt handler (LK compatibility)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct int_handler_saved_state_t {
    /// Saved interrupt state flags
    pub flags: u64,
    /// Saved DAIF register value
    pub daif: u64,
}

impl Default for int_handler_saved_state_t {
    fn default() -> Self {
        Self {
            flags: 0,
            daif: 0,
        }
    }
}

/// Start interrupt handler (save state)
pub fn int_handler_start() -> int_handler_saved_state_t {
    unsafe {
        let daif: u64;
        core::arch::asm!("mrs {}, daif", out(reg) daif);
        int_handler_saved_state_t {
            flags: 0,
            daif,
        }
    }
}

/// Finish interrupt handler (restore state)
pub fn int_handler_finish(_state: int_handler_saved_state_t) {
    // TODO: Restore interrupt state if needed
}

// ============================================================================
// Inter-Processor Interrupts (IPI)
// ============================================================================

/// Send IPI to target CPUs
pub fn arm64_send_ipi(_target: u32, _ipi_type: u32) {
    // TODO: Implement IPI sending via GIC
    // This would use gic_send_sgi or similar mechanism
}

