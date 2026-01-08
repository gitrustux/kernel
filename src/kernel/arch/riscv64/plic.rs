// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V Platform-Level Interrupt Controller (PLIC) support
//!
//! This module provides functions for interacting with the RISC-V PLIC,
//! which handles external interrupts for the system.


use crate::arch::riscv64;

// PLIC register offsets (platform-specific, would typically come from device tree)
const PLIC_PRIORITY_BASE: usize = 0x0000;
const PLIC_PENDING_BASE: usize = 0x1000;
const PLIC_ENABLE_BASE: usize = 0x2000;
const PLIC_ENABLE_BASE_PER_HART: usize = 0x80;
const PLIC_CONTEXT_BASE: usize = 0x200000;
const PLIC_CONTEXT_BASE_PER_HART: usize = 0x1000;
const PLIC_THRESHOLD: usize = 0x00;
const PLIC_COMPLETE: usize = 0x04;
const PLIC_CLAIM: usize = 0x00;

/// PLIC base address (platform-specific, must be initialized)
static mut PLIC_BASE: usize = 0;

/// Maximum number of interrupt sources (platform-specific)
const PLIC_MAX_SOURCES: u32 = 512;

/// Initialize the PLIC
///
/// This function sets up the PLIC base address and performs basic initialization.
///
/// # Arguments
///
/// * `base_addr` - The physical base address of the PLIC registers
///
/// # Safety
///
/// This function modifies global state and should only be called once during boot.
pub unsafe fn plic_init(base_addr: usize) {
    PLIC_BASE = base_addr;
}

/// Enable an interrupt for a specific hart (CPU)
///
/// This function enables a specific interrupt source in the PLIC's enable register
/// for the specified hart.
///
/// # Arguments
///
/// * `hart` - The hart (CPU) number (0-indexed)
/// * `irq` - The interrupt number to enable (must be >= 1, 0 is reserved)
///
/// # Safety
///
/// This function modifies hardware PLIC registers. The caller must ensure
/// that `hart` is valid for this system.
pub unsafe fn plic_enable_irq(hart: u32, irq: u32) {
    if PLIC_BASE == 0 {
        return; // PLIC not initialized
    }

    if irq == 0 || irq > PLIC_MAX_SOURCES {
        return; // Invalid IRQ
    }

    // Calculate the enable register address for this hart
    let hart_enable_base = PLIC_BASE + PLIC_ENABLE_BASE +
        (hart as usize * PLIC_ENABLE_BASE_PER_HART);

    // Each enable register controls 32 interrupts (bits 0-31, but bit 0 is reserved)
    // IRQ numbers are 1-based, so we shift by 1
    let word_num = ((irq - 1) / 32) as usize;
    let bit_num = ((irq - 1) % 32) as usize;

    let enable_reg = hart_enable_base + (word_num * 4);

    // Set the bit to enable the interrupt
    let ptr = enable_reg as *mut u32;
    let current_value = ptr.read_volatile();
    ptr.write_volatile(current_value | (1 << bit_num));

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Disable an interrupt for a specific hart (CPU)
///
/// This function disables a specific interrupt source in the PLIC's enable register
/// for the specified hart.
///
/// # Arguments
///
/// * `hart` - The hart (CPU) number (0-indexed)
/// * `irq` - The interrupt number to disable (must be >= 1, 0 is reserved)
///
/// # Safety
///
/// This function modifies hardware PLIC registers. The caller must ensure
/// that `hart` is valid for this system.
pub unsafe fn plic_disable_irq(hart: u32, irq: u32) {
    if PLIC_BASE == 0 {
        return; // PLIC not initialized
    }

    if irq == 0 || irq > PLIC_MAX_SOURCES {
        return; // Invalid IRQ
    }

    // Calculate the enable register address for this hart
    let hart_enable_base = PLIC_BASE + PLIC_ENABLE_BASE +
        (hart as usize * PLIC_ENABLE_BASE_PER_HART);

    // Each enable register controls 32 interrupts
    let word_num = ((irq - 1) / 32) as usize;
    let bit_num = ((irq - 1) % 32) as usize;

    let enable_reg = hart_enable_base + (word_num * 4);

    // Clear the bit to disable the interrupt
    let ptr = enable_reg as *mut u32;
    let current_value = ptr.read_volatile();
    ptr.write_volatile(current_value & !(1 << bit_num));

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Set the interrupt priority
///
/// This function sets the priority level for a specific interrupt source.
///
/// # Arguments
///
/// * `irq` - The interrupt number (must be >= 1, 0 is reserved)
/// * `priority` - The priority level (typically 0-7, higher = more priority)
///
/// # Safety
///
/// This function modifies hardware PLIC registers.
pub unsafe fn plic_set_priority(irq: u32, priority: u32) {
    if PLIC_BASE == 0 {
        return; // PLIC not initialized
    }

    if irq == 0 || irq > PLIC_MAX_SOURCES {
        return; // Invalid IRQ
    }

    if priority > 7 {
        return; // Invalid priority
    }

    // Each priority register is 4 bytes
    let priority_reg = PLIC_BASE + PLIC_PRIORITY_BASE + ((irq as usize) * 4);

    let ptr = priority_reg as *mut u32;
    ptr.write_volatile(priority);

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Claim an interrupt for a specific hart
///
/// This function claims the highest-priority pending interrupt for the specified hart.
/// This should be called from the interrupt handler to get the IRQ number.
///
/// # Arguments
///
/// * `hart` - The hart (CPU) number (0-indexed)
///
/// # Returns
///
/// The IRQ number that was claimed, or 0 if no interrupt is pending.
///
/// # Safety
///
/// This function modifies hardware PLIC state. It must only be called
/// from within an interrupt handler.
pub unsafe fn plic_claim(hart: u32) -> u32 {
    if PLIC_BASE == 0 {
        return 0; // PLIC not initialized
    }

    // Calculate the claim/complete register address for this hart
    let hart_context = PLIC_BASE + PLIC_CONTEXT_BASE +
        (hart as usize * PLIC_CONTEXT_BASE_PER_HART);

    let claim_reg = hart_context + PLIC_CLAIM;

    let ptr = claim_reg as *const u32;
    let irq = ptr.read_volatile();

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    irq
}

/// Complete handling of an interrupt for a specific hart
///
/// This function signals to the PLIC that the interrupt handler has finished
/// processing the specified interrupt.
///
/// # Arguments
///
/// * `hart` - The hart (CPU) number (0-indexed)
/// * `irq` - The interrupt number to complete
///
/// # Safety
///
/// This function modifies hardware PLIC state. It must only be called
/// after the interrupt has been properly handled and only with an IRQ
/// that was previously claimed.
pub unsafe fn plic_complete(hart: u32, irq: u32) {
    if PLIC_BASE == 0 {
        return; // PLIC not initialized
    }

    if irq == 0 {
        return; // Cannot complete IRQ 0 (no interrupt)
    }

    // Calculate the claim/complete register address for this hart
    let hart_context = PLIC_BASE + PLIC_CONTEXT_BASE +
        (hart as usize * PLIC_CONTEXT_BASE_PER_HART);

    let complete_reg = hart_context + PLIC_COMPLETE;

    let ptr = complete_reg as *mut u32;
    ptr.write_volatile(irq);

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Set the interrupt threshold for a specific hart
///
/// This function sets the priority threshold for interrupts that will be
/// delivered to the specified hart. Only interrupts with priority greater
/// than or equal to the threshold will be delivered.
///
/// # Arguments
///
/// * `hart` - The hart (CPU) number (0-indexed)
/// * `threshold` - The priority threshold (typically 0-7)
///
/// # Safety
///
/// This function modifies hardware PLIC registers.
pub unsafe fn plic_set_threshold(hart: u32, threshold: u32) {
    if PLIC_BASE == 0 {
        return; // PLIC not initialized
    }

    if threshold > 7 {
        return; // Invalid threshold
    }

    // Calculate the threshold register address for this hart
    let hart_context = PLIC_BASE + PLIC_CONTEXT_BASE +
        (hart as usize * PLIC_CONTEXT_BASE_PER_HART);

    let threshold_reg = hart_context + PLIC_THRESHOLD;

    let ptr = threshold_reg as *mut u32;
    ptr.write_volatile(threshold);

    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}
