// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! MMU memory type management
//!
//! This module provides functions for initializing and configuring 
//! memory types used by the x86 MMU, including PAT (Page Attribute Table) configuration.

use crate::kernel::cpu::CpuMask;

/// Initialize the memory type system for the x86 MMU
///
/// This function sets up the Page Attribute Table (PAT) and other memory type
/// configuration for the x86 architecture.
///
/// # Safety
///
/// This function is unsafe because it modifies CPU configuration registers directly.
pub unsafe fn x86_mmu_mem_type_init() {
    sys_x86_mmu_mem_type_init();
}

/// Synchronize the PAT configuration across multiple CPUs
///
/// # Arguments
///
/// * `targets` - CPU mask indicating which CPUs to update
///
/// # Safety
///
/// This function is unsafe because it modifies CPU configuration registers directly
/// and performs IPI (Inter-Processor Interrupt) operations.
pub unsafe fn x86_pat_sync(targets: CpuMask) {
    sys_x86_pat_sync(targets);
}

// Foreign function declarations
extern "C" {
    fn sys_x86_mmu_mem_type_init();
    fn sys_x86_pat_sync(targets: CpuMask);
}