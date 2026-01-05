// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! CPU topology detection and management for x86 processors
//!
//! This module provides functionality for determining the topology of x86
//! processors including package, node, core, and SMT information.

use crate::arch::amd64::feature;

/// Represents the topology information for an x86 CPU
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct X86CpuTopology {
    /// Physical package/socket identifier
    pub package_id: u32,
    /// NUMA node identifier
    pub node_id: u32,
    /// Core identifier within the package
    pub core_id: u32,
    /// Simultaneous Multi-Threading (SMT) identifier within the core
    pub smt_id: u32,
}

impl X86CpuTopology {
    /// Creates a new CPU topology structure with all fields set to 0
    #[inline]
    pub const fn new() -> Self {
        Self {
            package_id: 0,
            node_id: 0,
            core_id: 0,
            smt_id: 0,
        }
    }
}

/// Initialize the CPU topology detection subsystem
///
/// This function should be called early in the boot process before
/// other cores are brought online.
pub fn x86_cpu_topology_init() {
    unsafe { sys_x86_cpu_topology_init() }
}

/// Decode the topology information from an APIC ID
///
/// # Arguments
///
/// * `apic_id` - The APIC ID to decode
///
/// # Returns
///
/// The decoded CPU topology information
pub fn x86_cpu_topology_decode(apic_id: u32) -> X86CpuTopology {
    let mut topo = X86CpuTopology::new();
    unsafe { sys_x86_cpu_topology_decode(apic_id, &mut topo) };
    topo
}

// FFI declarations for the system implementations
extern "C" {
    fn sys_x86_cpu_topology_init();
    fn sys_x86_cpu_topology_decode(apic_id: u32, topo: *mut X86CpuTopology);
}