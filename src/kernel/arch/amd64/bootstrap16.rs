// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! 16-bit bootstrap code for SMP startup
//!
//! This module contains the 16-bit code used for booting secondary CPUs.
//!
//! # Bootstrap Process
//!
//! 1. BIOS starts APs at a specific address in low memory (typically 0x7000-0x8000)
//! 2. APs start in 16-bit real mode with CS:IP pointing to this code
//! 3. The bootstrap code must:
//!    - Load a GDT to switch to protected mode
//!    - Enable PAE (Physical Address Extension)
//!    - Load page tables (CR3)
//!    - Enable long mode (EFER.LME)
//!    - Enable paging (CR0.PG)
//!    - Load 64-bit segment registers
//!    - Jump to the 64-bit kernel entry point
//!
//! # Assembly Interface
//!
//! The actual 16-bit code must be in assembly (start16.S).
//! This Rust function is called after the switch to long mode.

use crate::rustux::types::*;

/// Bootstrap data passed from assembly to Rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BootstrapInfo {
    /// CPU number assigned to this core
    pub cpu_num: u32,
    /// APIC ID of this CPU
    pub apic_id: u32,
    /// Physical address of the kernel's PML4
    pub cr3: PAddr,
    /// Stack top address for this CPU
    pub stack_top: VAddr,
    /// Entry point for the kernel's per-CPU initialization
    pub entry_point: VAddr,
}

/// Secondary CPU bootstrap entry (called from 16-bit assembly)
///
/// This is called after the assembly code has switched to long mode.
/// At this point:
/// - We're in 64-bit mode
/// - Paging is enabled
/// - We have a valid stack
/// - All segment registers are properly set up
///
/// # Safety
///
/// Must be called with valid bootstrap info
#[no_mangle]
pub unsafe extern "C" fn bootstrap16(info: BootstrapInfo) -> ! {
    // Call the architecture-specific SMP initialization
    extern "C" {
        fn x86_secondary_cpu_entry(info: BootstrapInfo) -> !;
    }

    x86_secondary_cpu_entry(info)
}

/// Initialize the bootstrap area in low memory
///
/// This sets up the code and data that APs will execute when started.
///
/// # Arguments
///
/// * `bootstrap_code` - Physical address where bootstrap code should be placed
/// * `code_size` - Size of the bootstrap code area
///
/// # Returns
///
/// Physical address of the bootstrap entry point
pub unsafe fn init_bootstrap_area(bootstrap_code: PAddr, code_size: usize) -> PAddr {
    // In a real implementation, this would:
    // 1. Copy the 16-bit bootstrap code to the bootstrap area
    // 2. Set up the bootstrap data structure
    // 3. Configure the GDT for the bootstrap code
    // 4. Return the entry point address

    // For now, return the bootstrap code address as the entry point
    let _ = code_size;
    bootstrap_code
}

/// Start a secondary CPU
///
/// # Arguments
///
/// * `cpu_num` - CPU number to start
/// * `apic_id` - APIC ID of the target CPU
/// * `entry_point` - 64-bit kernel entry point
/// * `stack_top` - Stack pointer for the new CPU
/// * `cr3` - Page table physical address
///
/// # Returns
///
/// true if the CPU was started successfully
pub unsafe fn start_secondary_cpu(
    cpu_num: u32,
    apic_id: u32,
    entry_point: VAddr,
    stack_top: VAddr,
    cr3: PAddr,
) -> bool {
    // TODO: Implement IPI-based CPU startup
    // 1. Write the bootstrap code to low memory (typically 0x7000)
    // 2. Set up the bootstrap data with the provided parameters
    // 3. Send INIT IPI to the target CPU
    // 4. Wait 10ms
    // 5. Send STARTUP IPI to the target CPU with the bootstrap address
    // 6. Wait for the CPU to signal it's ready
    // 7. Clean up the bootstrap area

    let _ = cpu_num;
    let _ = apic_id;
    let _ = entry_point;
    let _ = stack_top;
    let _ = cr3;

    // For now, return false as this is not yet implemented
    false
}
