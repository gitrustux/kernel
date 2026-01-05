// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! AMD64 (x86-64) interrupt controller support
//!
//! This module provides wrapper functions for x86 interrupt handling,
//! wrapping the APIC (Advanced Programmable Interrupt Controller) functions.

#![no_std]

use crate::kernel::arch::amd64;
use crate::kernel::arch::amd64::apic;

/// Enable an IRQ
///
/// This function unmasks the specified IRQ at the I/O APIC level,
/// allowing interrupts to be delivered.
///
/// # Arguments
///
/// * `irq` - The IRQ number to enable
///
/// # Safety
///
/// This function modifies hardware interrupt controller state.
pub unsafe fn x86_enable_irq(irq: u32) {
    // Unmask the IRQ
    apic::apic_io_mask_irq(irq);
}

/// Disable an IRQ
///
/// This function masks the specified IRQ at the I/O APIC level,
/// preventing interrupts from being delivered.
///
/// # Arguments
///
/// * `irq` - The IRQ number to disable
///
/// # Safety
///
/// This function modifies hardware interrupt controller state.
pub unsafe fn x86_disable_irq(irq: u32) {
    // Mask the IRQ
    apic::apic_io_mask_irq(irq);
}

/// Send an End-Of-Interrupt (EOI) signal
///
/// This signals to the local APIC that the interrupt handler has
/// completed processing the current interrupt.
///
/// # Arguments
///
/// * `irq` - The IRQ number (not used in basic EOI, but provided for API compatibility)
///
/// # Safety
///
/// This function must only be called from within an interrupt handler.
pub unsafe fn x86_send_eoi(_irq: u32) {
    apic::apic_issue_eoi();
}

/// Send an Inter-Processor Interrupt (IPI)
///
/// This sends an IPI to the specified target CPU.
///
/// # Arguments
///
/// * `vector` - The interrupt vector to send
/// * `target_cpu` - The target CPU APIC ID
///
/// # Returns
///
/// Returns 0 on success, or a negative error code on failure.
///
/// # Safety
///
/// This function sends an interrupt to another CPU. The caller must ensure
/// proper interrupt handling is set up on the target CPU.
pub unsafe fn x86_send_ipi(vector: u32, target_cpu: u32) -> i32 {
    // Convert vector to u8
    if vector > 255 {
        return -1; // Invalid vector
    }

    let vector = vector as u8;

    // Send the IPI using the APIC
    apic::apic_send_ipi(
        vector,
        target_cpu,
        apic::ApicInterruptDeliveryMode::Fixed,
    );

    0 // OK
}

/// Send an IPI to all CPUs
///
/// This broadcasts an IPI to all CPUs in the system.
///
/// # Arguments
///
/// * `vector` - The interrupt vector to send
///
/// # Returns
///
/// Returns 0 on success, or a negative error code on failure.
///
/// # Safety
///
/// This function sends an interrupt to all CPUs. The caller must ensure
/// proper interrupt handling is set up on the target CPUs.
pub unsafe fn x86_broadcast_ipi(vector: u32) -> i32 {
    // Convert vector to u8
    if vector > 255 {
        return -1; // Invalid vector
    }

    let vector = vector as u8;

    // Broadcast the IPI
    apic::apic_send_broadcast_ipi(vector);

    0 // OK
}

/// Send an IPI to all CPUs except the sender
///
/// This broadcasts an IPI to all CPUs except the current CPU.
///
/// # Arguments
///
/// * `vector` - The interrupt vector to send
///
/// # Returns
///
/// Returns 0 on success, or a negative error code on failure.
///
/// # Safety
///
/// This function sends an interrupt to all other CPUs. The caller must ensure
/// proper interrupt handling is set up on the target CPUs.
pub unsafe fn x86_broadcast_ipi_self(vector: u32) -> i32 {
    // Convert vector to u8
    if vector > 255 {
        return -1; // Invalid vector
    }

    let vector = vector as u8;

    // Broadcast the IPI to all except self
    apic::apic_send_broadcast_self_ipi(vector, apic::ApicInterruptDeliveryMode::Fixed);

    0 // OK
}

/// Check if interrupts are currently enabled
///
/// # Returns
///
/// `true` if interrupts are enabled, `false` if disabled.
#[inline]
pub fn x86_interrupts_enabled() -> bool {
    unsafe {
        let rflags: u64;
        core::arch::asm!("pushfq; pop {}", out(reg) rflags);
        (rflags & (1 << 9)) != 0
    }
}

/// Disable interrupts
///
/// Disables interrupts on the current CPU and returns the previous interrupt state.
///
/// # Returns
///
/// The previous interrupt flags (RFLAGS value).
///
/// # Safety
///
/// This function modifies the CPU's interrupt state. The caller must
/// eventually restore the previous state using `x86_restore_interrupts`.
#[inline]
pub unsafe fn x86_disable_interrupts() -> u64 {
    let rflags: u64;
    core::arch::asm!("pushfq; pop {}", out(reg) rflags);
    core::arch::asm!("cli");
    rflags
}

/// Restore interrupt state
///
/// Restores the interrupt state to a previously saved value.
///
/// # Arguments
///
/// * `rflags` - The saved RFLAGS value to restore
///
/// # Safety
///
/// This function modifies the CPU's interrupt state.
#[inline]
pub unsafe fn x86_restore_interrupts(rflags: u64) {
    if (rflags & (1 << 9)) != 0 {
        core::arch::asm!("sti");
    }
}

/// Enable interrupts
///
/// Enables interrupts on the current CPU.
///
/// # Safety
///
/// This function modifies the CPU's interrupt state.
#[inline]
pub unsafe fn x86_enable_interrupts() {
    core::arch::asm!("sti");
}

/// Send IPI to target CPUs
///
/// # Arguments
///
/// * `target_mask` - Bitmask of target CPUs
/// * `ipi_type` - IPI type/delivery mode
pub fn amd64_send_ipi(target_mask: u64, ipi_type: u8) {
    // TODO: Implement proper IPI sending based on target_mask and ipi_type
    // For now, this is a stub
    let _ = target_mask;
    let _ = ipi_type;
}
