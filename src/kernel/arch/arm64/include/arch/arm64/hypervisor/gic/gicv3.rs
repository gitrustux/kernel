// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! GICv3 (Generic Interrupt Controller v3) interface definitions for ARM64 hypervisor
//! 
//! This module provides constants and functions for manipulating GICv3 registers
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
/// The virtual interrupt ID (bits 0-31)
#[inline]
pub const fn ich_lr_virtual_id(id: u64) -> u32 {
    (id & 0xFFFF_FFFF) as u32
}

/// Extract the physical interrupt ID from a List Register (LR) value.
/// 
/// # Arguments
/// 
/// * `id` - The raw List Register value
/// 
/// # Returns
/// 
/// The physical interrupt ID (bits 32-41)
#[inline]
pub const fn ich_lr_physical_id(id: u64) -> u64 {
    (id & 0x3FF) << 32
}

/// Encode the priority into a List Register (LR) value.
/// 
/// # Arguments
/// 
/// * `prio` - The interrupt priority (0-255)
/// 
/// # Returns
/// 
/// The priority encoded for LR (bits 48-55)
#[inline]
pub const fn ich_lr_priority(prio: u8) -> u64 {
    (prio as u64) << 48
}

/// Indicates that the interrupt belongs to Group 1 in the List Register (LR).
pub const ICH_LR_GROUP1: u64 = 1 << 60;

/// Indicates that the interrupt is a hardware interrupt in the List Register (LR).
pub const ICH_LR_HARDWARE: u64 = 1 << 61;

/// Indicates that the interrupt is pending in the List Register (LR).
pub const ICH_LR_PENDING: u64 = 1 << 62;

/// Enable Group 1 interrupts in the Virtual Machine Control Register (VMCR).
pub const ICH_VMCR_VENG1: u32 = 1 << 1;

/// Enable FIQ in the Virtual Machine Control Register (VMCR).
pub const ICH_VMCR_VFIQEN: u32 = 1 << 3;

/// Priority mask for the Virtual Machine Control Register (VMCR).
pub const ICH_VMCR_VPMR: u32 = 0xFF << 24;

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
pub const fn ich_vtr_pres(vtr: u32) -> u32 {
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
pub const fn ich_vtr_lrs(vtr: u32) -> u32 {
    (vtr & 0x1F) + 1
}

/// Register the GICv3 hardware interface.
/// This function should be called during system initialization to set up the GICv3.
pub fn rx_gicv3_hw_interface_register() {
    // Implementation will be added later when hardware initialization is required
    unimplemented!("GICv3 hardware interface registration not yet implemented");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_id_extraction() {
        assert_eq!(ich_lr_virtual_id(0xFFFF_FFFF), 0xFFFF_FFFF);
        assert_eq!(ich_lr_virtual_id(0xFFFF_FFFF_0000_0000), 0);
    }

    #[test]
    fn test_physical_id_encoding() {
        assert_eq!(ich_lr_physical_id(0x3FF), 0x3FF_0000_0000);
        assert_eq!(ich_lr_physical_id(0x400), 0);
    }

    #[test]
    fn test_priority_encoding() {
        assert_eq!(ich_lr_priority(0xFF), 0xFF_0000_0000_0000);
        assert_eq!(ich_lr_priority(0), 0);
    }

    #[test]
    fn test_vtr_calculations() {
        assert_eq!(ich_vtr_pres(0x4000000), 2);
        assert_eq!(ich_vtr_lrs(0x1F), 32);
    }
}