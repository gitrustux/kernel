// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! GICv3 EL2 register access functions for ARM64
//! 
//! This module provides safe abstractions for accessing GICv3 system registers
//! at EL2 (Exception Level 2). These registers are used to configure and manage
//! the Generic Interrupt Controller version 3 in a virtualized environment.

#![allow(clippy::missing_safety_doc)]

use core::arch::asm;

/// Maximum index for List Registers
const MAX_LR_INDEX: u32 = 15;  // GICv3 typically supports up to 16 LRs
/// Maximum index for Active Priority Registers
const MAX_APR_INDEX: u32 = 3;  // GICv3 typically supports up to 4 APRs

/// Error type for register operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegError {
    /// Index exceeds maximum allowed value
    InvalidIndex,
}

/// Write to the GICH HCR (Hypervisor Control Register).
/// 
/// # Safety
/// 
/// This function is unsafe because it modifies system registers that affect
/// interrupt handling and virtualization behavior. Incorrect values may
/// destabilize the system.
pub unsafe fn rx_el2_gicv3_write_gich_hcr(val: u32) {
    asm!(
        "msr ICH_HCR_EL2, {0}",
        in(reg) val,
        options(nostack, preserves_flags)
    );
}

/// Read from the GICH VTR (Virtual Timer Register).
/// 
/// # Returns
/// 
/// Current value of the VTR register.
#[inline]
pub unsafe fn rx_el2_gicv3_read_gich_vtr() -> u32 {
    let val: u32;
    asm!(
        "mrs {0}, ICH_VTR_EL2",
        out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

/// Read from the GICH VMCR (Virtual Machine Control Register).
/// 
/// # Returns
/// 
/// Current value of the VMCR register.
#[inline]
pub unsafe fn rx_el2_gicv3_read_gich_vmcr() -> u32 {
    let val: u32;
    asm!(
        "mrs {0}, ICH_VMCR_EL2",
        out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

/// Write to the GICH VMCR (Virtual Machine Control Register).
/// 
/// # Safety
/// 
/// This function is unsafe because it modifies virtual machine interrupt
/// configuration. Incorrect values may cause interrupt handling issues.
pub unsafe fn rx_el2_gicv3_write_gich_vmcr(val: u32) {
    asm!(
        "msr ICH_VMCR_EL2, {0}",
        in(reg) val,
        options(nostack, preserves_flags)
    );
}

/// Read from the GICH MISR (Maintenance Interrupt Status Register).
/// 
/// # Returns
/// 
/// Current value of the MISR register.
#[inline]
pub unsafe fn rx_el2_gicv3_read_gich_misr() -> u32 {
    let val: u32;
    asm!(
        "mrs {0}, ICH_MISR_EL2",
        out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

/// Read from the GICH ELRSR (Empty List Register Status Register).
/// 
/// # Returns
/// 
/// Current value of the ELRSR register.
#[inline]
pub unsafe fn rx_el2_gicv3_read_gich_elrsr() -> u32 {
    let val: u32;
    asm!(
        "mrs {0}, ICH_ELRSR_EL2",
        out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

/// Read from the GICH APR (Active Priority Register).
/// 
/// # Arguments
/// 
/// * `idx` - Index of the APR to read (0-3)
/// 
/// # Returns
/// 
/// Result containing either the APR value or an error if the index is invalid.
#[inline]
pub unsafe fn rx_el2_gicv3_read_gich_apr(idx: u32) -> Result<u32, RegError> {
    if idx > MAX_APR_INDEX {
        return Err(RegError::InvalidIndex);
    }
    
    let val: u32;
    asm!(
        "mrs {0}, ICH_APR0_EL2",
        out(reg) val,
        options(nostack, preserves_flags)
    );
    Ok(val)
}

/// Write to the GICH APR (Active Priority Register).
/// 
/// # Arguments
/// 
/// * `val` - Value to write to the APR
/// * `idx` - Index of the APR to write (0-3)
/// 
/// # Returns
/// 
/// Error if the index is invalid.
pub unsafe fn rx_el2_gicv3_write_gich_apr(val: u32, idx: u32) -> Result<(), RegError> {
    if idx > MAX_APR_INDEX {
        return Err(RegError::InvalidIndex);
    }
    
    asm!(
        "msr ICH_APR0_EL2, {0}",
        in(reg) val,
        options(nostack, preserves_flags)
    );
    Ok(())
}

/// Read from the GICH LR (List Register).
/// 
/// # Arguments
/// 
/// * `idx` - Index of the LR to read (0-15)
/// 
/// # Returns
/// 
/// Result containing either the LR value or an error if the index is invalid.
#[inline]
pub unsafe fn rx_el2_gicv3_read_gich_lr(idx: u32) -> Result<u64, RegError> {
    if idx > MAX_LR_INDEX {
        return Err(RegError::InvalidIndex);
    }
    
    let val: u64;
    asm!(
        "mrs {0}, ICH_LR{1}_EL2",
        out(reg) val,
        const idx,
        options(nostack, preserves_flags)
    );
    Ok(val)
}

/// Write to the GICH LR (List Register).
/// 
/// # Arguments
/// 
/// * `val` - Value to write to the LR
/// * `idx` - Index of the LR to write (0-15)
/// 
/// # Returns
/// 
/// Error if the index is invalid.
pub unsafe fn rx_el2_gicv3_write_gich_lr(val: u64, idx: u32) -> Result<(), RegError> {
    if idx > MAX_LR_INDEX {
        return Err(RegError::InvalidIndex);
    }
    
    asm!(
        "msr ICH_LR{1}_EL2, {0}",
        in(reg) val,
        const idx,
        options(nostack, preserves_flags)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_index_validation() {
        unsafe {
            assert!(rx_el2_gicv3_read_gich_lr(15).is_ok());
            assert!(rx_el2_gicv3_read_gich_lr(16).is_err());
            
            assert!(rx_el2_gicv3_read_gich_apr(3).is_ok());
            assert!(rx_el2_gicv3_read_gich_apr(4).is_err());
        }
    }
}