// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use core::ptr::{self, addr_of};
use core::sync::atomic::{compiler_fence, Ordering};
use crate::kernel::arch::arm64::hypervisor::gic::gicv2_regs::*;
use crate::kernel::dev::interrupt::arm_gic_hw_interface::{ArmGicHwInterfaceOps, ArmGicHwInterface};
use crate::kernel::platform::Platform;
use crate::kernel::syscalls::SyscallResult;

/// Number of List Registers (LRs) in GICv2.
const NUM_LRS: usize = 64;

/// Representation of GICH registers.
/// For details, refer to ARM Generic Interrupt Controller Architecture Specification Version 2.
#[repr(C)]
#[repr(packed)]
struct Gich {
    hcr: u32,
    vtr: u32,
    vmcr: u32,
    _reserved0: u32,
    misr: u32,
    _reserved1: [u32; 3],
    eisr0: u32,
    eisr1: u32,
    _reserved2: [u32; 2],
    elrsr0: u32,
    elrsr1: u32,
    _reserved3: [u32; 46],
    apr: u32,
    _reserved4: [u32; 3],
    lr: [u32; NUM_LRS],
}

// Ensure the offsets of GICH registers match the specification.
const_assert!(offset_of!(Gich, hcr) == 0x00);
const_assert!(offset_of!(Gich, vtr) == 0x04);
const_assert!(offset_of!(Gich, vmcr) == 0x08);
const_assert!(offset_of!(Gich, misr) == 0x10);
const_assert!(offset_of!(Gich, eisr0) == 0x20);
const_assert!(offset_of!(Gich, eisr1) == 0x24);
const_assert!(offset_of!(Gich, elrsr0) == 0x30);
const_assert!(offset_of!(Gich, elrsr1) == 0x34);
const_assert!(offset_of!(Gich, apr) == 0xF0);
const_assert!(offset_of!(Gich, lr) == 0x100);

/// Global GICH register pointer.
static mut GICH: *mut Gich = ptr::null_mut();

/// Write to the GICH HCR register.
unsafe fn gicv2_write_gich_hcr(val: u32) {
    (*GICH).hcr = val;
}

/// Read from the GICH VTR register.
unsafe fn gicv2_read_gich_vtr() -> u32 {
    (*GICH).vtr
}

/// Get the default value for the GICH VMCR register.
fn gicv2_default_gich_vmcr() -> u32 {
    GICH_VMCR_VPMR | GICH_VMCR_VENG0
}

/// Read from the GICH VMCR register.
unsafe fn gicv2_read_gich_vmcr() -> u32 {
    (*GICH).vmcr
}

/// Write to the GICH VMCR register.
unsafe fn gicv2_write_gich_vmcr(val: u32) {
    (*GICH).vmcr = val;
}

/// Read from the GICH MISR register.
unsafe fn gicv2_read_gich_misr() -> u32 {
    (*GICH).misr
}

/// Read from the GICH ELRSR register.
unsafe fn gicv2_read_gich_elrsr() -> u64 {
    ((*GICH).elrsr0 as u64) | ((*GICH).elrsr1 as u64) << 32
}

/// Read from the GICH APR register.
unsafe fn gicv2_read_gich_apr(idx: u32) -> u32 {
    assert!(idx == 0);
    (*GICH).apr
}

/// Write to the GICH APR register.
unsafe fn gicv2_write_gich_apr(idx: u32, val: u32) {
    assert!(idx == 0);
    (*GICH).apr = val;
}

/// Read from the GICH LR register.
unsafe fn gicv2_read_gich_lr(idx: u32) -> u64 {
    assert!(idx < NUM_LRS as u32);
    (*GICH).lr[idx as usize] as u64
}

/// Write to the GICH LR register.
unsafe fn gicv2_write_gich_lr(idx: u32, val: u64) {
    assert!(idx < NUM_LRS as u32);
    if val & GICH_LR_HARDWARE != 0 {
        // Mark the physical interrupt as active on the physical distributor.
        let vector = GICH_LR_VIRTUAL_ID(val);
        let reg = vector / 32;
        let mask = 1u32 << (vector % 32);
        GICREG(0, GICD_ISACTIVER(reg)) = mask;
    }
    (*GICH).lr[idx as usize] = val as u32;
}

/// Get the GICV base address.
unsafe fn gicv2_get_gicv(gicv_paddr: &mut u64) -> SyscallResult {
    if GICV_OFFSET == 0 {
        return Err(SyscallError::NotSupported);
    }
    *gicv_paddr = vaddr_to_paddr(GICV_ADDRESS as *const ());
    Ok(())
}

/// Get the LR value from a vector.
fn gicv2_get_lr_from_vector(hw: bool, prio: u8, vector: u32) -> u64 {
    let mut lr = GICH_LR_PENDING | GICH_LR_PRIORITY(prio as u32) | GICH_LR_VIRTUAL_ID(vector);
    if hw {
        lr |= GICH_LR_HARDWARE | GICH_LR_PHYSICAL_ID(vector);
    }
    lr
}

/// Get the vector from an LR value.
fn gicv2_get_vector_from_lr(lr: u64) -> u32 {
    lr & GICH_LR_VIRTUAL_ID(u64::MAX)
}

/// Get the number of preemption levels.
fn gicv2_get_num_pres() -> u32 {
    GICH_VTR_PRES(gicv2_read_gich_vtr())
}

/// Get the number of list registers.
fn gicv2_get_num_lrs() -> u32 {
    GICH_VTR_LRS(gicv2_read_gich_vtr())
}

/// Register the GICv2 hardware interface.
pub unsafe fn gicv2_hw_interface_register() {
    GICH = GICH_ADDRESS as *mut Gich;
    let ops = ArmGicHwInterfaceOps {
        write_gich_hcr: gicv2_write_gich_hcr,
        read_gich_vtr: gicv2_read_gich_vtr,
        default_gich_vmcr: gicv2_default_gich_vmcr,
        read_gich_vmcr: gicv2_read_gich_vmcr,
        write_gich_vmcr: gicv2_write_gich_vmcr,
        read_gich_misr: gicv2_read_gich_misr,
        read_gich_elrsr: gicv2_read_gich_elrsr,
        read_gich_apr: gicv2_read_gich_apr,
        write_gich_apr: gicv2_write_gich_apr,
        read_gich_lr: gicv2_read_gich_lr,
        write_gich_lr: gicv2_write_gich_lr,
        get_gicv: gicv2_get_gicv,
        get_lr_from_vector: gicv2_get_lr_from_vector,
        get_vector_from_lr: gicv2_get_vector_from_lr,
        get_num_pres: gicv2_get_num_pres,
        get_num_lrs: gicv2_get_num_lrs,
    };
    ArmGicHwInterface::register(&ops);
}