// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use core::ptr::{self, addr_of};
use core::sync::atomic::{compiler_fence, Ordering};
use crate::kernel::arch::arm64::hypervisor::gic::gicv3_regs::*;
use crate::kernel::dev::interrupt::arm_gic_hw_interface::{ArmGicHwInterfaceOps, ArmGicHwInterface};
use crate::kernel::platform::Platform;
use crate::kernel::syscalls::SyscallResult;

/// Number of APR (Active Priority Registers) in GICv3.
const NUM_APRS: usize = 4;

/// Number of List Registers (LRs) in GICv3.
const NUM_LRS: usize = 16;

/// Write to the GICH HCR register.
unsafe fn gicv3_write_gich_hcr(val: u32) {
    arm64_el2_gicv3_write_gich_hcr(val);
}

/// Read from the GICH VTR register.
unsafe fn gicv3_read_gich_vtr() -> u32 {
    arm64_el2_gicv3_read_gich_vtr()
}

/// Get the default value for the GICH VMCR register.
fn gicv3_default_gich_vmcr() -> u32 {
    // From ARM GIC v3/v4, Section 8.4.8: VFIQEn - In implementations where the
    // Non-secure copy of ICC_SRE_EL1.SRE is always 1, this bit is RES 1.
    ICH_VMCR_VPMR | ICH_VMCR_VFIQEN | ICH_VMCR_VENG1
}

/// Read from the GICH VMCR register.
unsafe fn gicv3_read_gich_vmcr() -> u32 {
    arm64_el2_gicv3_read_gich_vmcr()
}

/// Write to the GICH VMCR register.
unsafe fn gicv3_write_gich_vmcr(val: u32) {
    arm64_el2_gicv3_write_gich_vmcr(val);
}

/// Read from the GICH MISR register.
unsafe fn gicv3_read_gich_misr() -> u32 {
    arm64_el2_gicv3_read_gich_misr()
}

/// Read from the GICH ELRSR register.
unsafe fn gicv3_read_gich_elrsr() -> u64 {
    arm64_el2_gicv3_read_gich_elrsr()
}

/// Read from the GICH APR register.
unsafe fn gicv3_read_gich_apr(idx: u32) -> u32 {
    assert!(idx < NUM_APRS as u32);
    arm64_el2_gicv3_read_gich_apr(idx)
}

/// Write to the GICH APR register.
unsafe fn gicv3_write_gich_apr(idx: u32, val: u32) {
    assert!(idx < NUM_APRS as u32);
    arm64_el2_gicv3_write_gich_apr(val, idx);
}

/// Read from the GICH LR register.
unsafe fn gicv3_read_gich_lr(idx: u32) -> u64 {
    assert!(idx < NUM_LRS as u32);
    arm64_el2_gicv3_read_gich_lr(idx)
}

/// Write to the GICH LR register.
unsafe fn gicv3_write_gich_lr(idx: u32, val: u64) {
    assert!(idx < NUM_LRS as u32);
    if val & ICH_LR_HARDWARE != 0 {
        // Mark the physical interrupt as active on the physical distributor.
        let vector = ICH_LR_VIRTUAL_ID(val);
        let reg = vector / 32;
        let mask = 1u32 << (vector % 32);
        // For SGIs and PPIs, use the redistributor for the current CPU.
        if vector < 32 {
            let cpu_num = Platform::current_cpu_num();
            GICREG(0, GICR_ISACTIVER0(cpu_num)) = mask;
        } else {
            GICREG(0, GICD_ISACTIVER(reg)) = mask;
        }
    }
    arm64_el2_gicv3_write_gich_lr(val, idx);
}

/// Get the GICV base address.
unsafe fn gicv3_get_gicv(gicv_paddr: &mut u64) -> SyscallResult {
    // GICv3 does not require mapping the GICV region.
    Err(SyscallError::NotFound)
}

/// Get the LR value from a vector.
fn gicv3_get_lr_from_vector(hw: bool, prio: u8, vector: u32) -> u64 {
    let mut lr = ICH_LR_PENDING | ICH_LR_GROUP1 | ICH_LR_PRIORITY(prio as u32) | ICH_LR_VIRTUAL_ID(vector);
    if hw {
        lr |= ICH_LR_HARDWARE | ICH_LR_PHYSICAL_ID(vector);
    }
    lr
}

/// Get the vector from an LR value.
fn gicv3_get_vector_from_lr(lr: u64) -> u32 {
    lr & ICH_LR_VIRTUAL_ID(u64::MAX)
}

/// Get the number of preemption levels.
fn gicv3_get_num_pres() -> u32 {
    ICH_VTR_PRES(gicv3_read_gich_vtr())
}

/// Get the number of list registers.
fn gicv3_get_num_lrs() -> u32 {
    ICH_VTR_LRS(gicv3_read_gich_vtr())
}

/// Register the GICv3 hardware interface.
pub unsafe fn gicv3_hw_interface_register() {
    let ops = ArmGicHwInterfaceOps {
        write_gich_hcr: gicv3_write_gich_hcr,
        read_gich_vtr: gicv3_read_gich_vtr,
        default_gich_vmcr: gicv3_default_gich_vmcr,
        read_gich_vmcr: gicv3_read_gich_vmcr,
        write_gich_vmcr: gicv3_write_gich_vmcr,
        read_gich_misr: gicv3_read_gich_misr,
        read_gich_elrsr: gicv3_read_gich_elrsr,
        read_gich_apr: gicv3_read_gich_apr,
        write_gich_apr: gicv3_write_gich_apr,
        read_gich_lr: gicv3_read_gich_lr,
        write_gich_lr: gicv3_write_gich_lr,
        get_gicv: gicv3_get_gicv,
        get_lr_from_vector: gicv3_get_lr_from_vector,
        get_vector_from_lr: gicv3_get_vector_from_lr,
        get_num_pres: gicv3_get_num_pres,
        get_num_lrs: gicv3_get_num_lrs,
    };
    ArmGicHwInterface::register(&ops);
}