// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! GIC (Generic Interrupt Controller) interface definitions for ARM64 hypervisor
//! 
//! This module provides constants and functions for manipulating GIC registers
//! in a virtualized environment, specifically for handling List Registers (LR),
//! Virtual Machine Control Register (VMCR), and Virtual Timer Register (VTR).

#![allow(non_upper_case_globals)]
#![allow(clippy::cast_possible_truncation)]

/// Extract the virtual interrupt ID from a List Register (LR) value.
/// 
/// # Arguments
/// 
/// * `id` - The raw List Register value
/// 
/// # Returns
/// 
/// The virtual interrupt ID (bits 0-9)
#[inline]
pub const fn gich_lr_virtual_id(id: u64) -> u32 {
    (id & 0x3ff) as u32
}

/// Extract the physical interrupt ID from a List Register (LR) value.
/// 
/// # Arguments
/// 
/// * `id` - The raw List Register value
/// 
/// # Returns
/// 
/// The physical interrupt ID (bits 10-19)
#[inline]
pub const fn gich_lr_physical_id(id: u64) -> u32 {
    ((id & 0x3ff) << 10) as u32
}

/// Encode the priority into a List Register (LR) value.
/// 
/// # Arguments
/// 
/// * `prio` - The interrupt priority (0-31)
/// 
/// # Returns
/// 
/// The priority encoded for LR (bits 23-27)
#[inline]
pub const fn gich_lr_priority(prio: u8) -> u64 {
    debug_assert!(prio <= 31, "priority must be 5 bits or less");
    ((prio & 0x1f) as u64) << 23
}

/// Indicates that the interrupt is pending in the List Register (LR).
pub const GICH_LR_PENDING: u64 = 1 << 28;

/// Indicates that the interrupt belongs to Group 1 in the List Register (LR).
pub const GICH_LR_GROUP1: u64 = 1 << 30;

/// Indicates that the interrupt is a hardware interrupt in the List Register (LR).
pub const GICH_LR_HARDWARE: u64 = 1 << 31;

/// Enable Group 0 interrupts in the Virtual Machine Control Register (VMCR).
pub const GICH_VMCR_VENG0: u32 = 1 << 0;

/// Priority mask for the Virtual Machine Control Register (VMCR).
pub const GICH_VMCR_VPMR: u32 = 0x1f << 27;

/// Extract the number of preemption levels from the Virtual Timer Register (VTR).
/// 
/// # Arguments
/// 
/// * `vtr` - The Virtual Timer Register value
/// 
/// # Returns
/// 
/// The number of preemption levels supported
#[inline]
pub const fn gich_vtr_pres(vtr: u32) -> u32 {
    ((vtr & (0x7 << 26)) >> 26) + 1
}

/// Extract the number of List Registers (LRs) from the Virtual Timer Register (VTR).
/// 
/// # Arguments
/// 
/// * `vtr` - The Virtual Timer Register value
/// 
/// # Returns
/// 
/// The number of List Registers available
#[inline]
pub const fn gich_vtr_lrs(vtr: u32) -> u32 {
    (vtr & 0x3f) + 1
}

/// Register the GICv2 hardware interface.
/// This function should be called during system initialization to set up the GICv2.
pub fn rx_gicv2_hw_interface_register() {
    // Implementation will be added later when hardware initialization is required
    unimplemented!("GICv2 hardware interface registration not yet implemented");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_id_extraction() {
        assert_eq!(gich_lr_virtual_id(0x3ff), 0x3ff);
        assert_eq!(gich_lr_virtual_id(0x400), 0);
    }

    #[test]
    fn test_physical_id_encoding() {
        assert_eq!(gich_lr_physical_id(0x3ff), 0xffc00);
        assert_eq!(gich_lr_physical_id(0x400), 0);
    }

    #[test]
    fn test_priority_encoding() {
        assert_eq!(gich_lr_priority(0x1f), 0x1f << 23);
        assert_eq!(gich_lr_priority(0x20), 0);
    }

    #[test]
    fn test_vtr_calculations() {
        assert_eq!(gich_vtr_pres(0x4000000), 2);
        assert_eq!(gich_vtr_lrs(0x3f), 64);
    }
}