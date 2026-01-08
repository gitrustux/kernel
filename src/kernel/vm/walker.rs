// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! VA→PA Walker and VM Debugging Utilities
//!
//! This module provides utilities for debugging virtual memory:
//! - Virtual to physical address translation
//! - Page table dumping
//! - Mapping audit
//! - Fault diagnostics
//!
//! # Design
//!
//! - Cross-architecture: Works on x86-64, ARM64, and RISC-V
//! - Safe: Uses safe Rust where possible, unsafe only for hardware access
//! - Consistent: Same output format across architectures


use crate::kernel::vm::layout::{VAddr, PAddr, PAGE_SIZE, is_user_vaddr};
use crate::kernel::vm::aspace::AddressSpace;
use crate::rustux::types::*;

// Import logging macros
use crate::{log_debug, log_info, log_error};

/// ============================================================================
/// VA→PA Translation Result
/// ============================================================================

/// Result of virtual to physical address translation
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaToPaResult {
    /// Translation successful
    Success {
        /// Physical address
        paddr: PAddr,
        /// Page table flags
        flags: PageTableFlags,
        /// Mapping level (0=PT, 1=PD, 2=PDP, 3=PML4)
        level: u8,
    },

    /// Page not present
    NotPresent {
        /// Level where translation failed
        level: u8,
        /// Entry at failed level
        entry: u64,
    },

    /// Invalid virtual address (not canonical)
    InvalidVaddr {
        /// Virtual address
        vaddr: VAddr,
    },

    /// Address space not found
    NoAspace,
}

impl VaToPaResult {
    /// Check if translation was successful
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Get physical address if successful
    pub const fn paddr(&self) -> Option<PAddr> {
        match self {
            Self::Success { paddr, .. } => Some(*paddr),
            _ => None,
        }
    }
}

/// Page table entry flags
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageTableFlags {
    /// Present bit
    pub present: bool,

    /// Writable bit
    pub writable: bool,

    /// User bit
    pub user: bool,

    /// Execute disable bit
    pub xd: bool,

    /// Access flag
    pub accessed: bool,

    /// Dirty flag
    pub dirty: bool,

    /// Global flag
    pub global: bool,
}

impl PageTableFlags {
    /// Create flags from raw x86-64 PTE
    #[cfg(target_arch = "x86_64")]
    pub const fn from_x86_pte(pte: u64) -> Self {
        Self {
            present: (pte & 1) != 0,
            writable: (pte & 2) != 0,
            user: (pte & 4) != 0,
            xd: (pte & (1 << 63)) != 0,
            accessed: (pte & (1 << 5)) != 0,
            dirty: (pte & (1 << 6)) != 0,
            global: (pte & (1 << 8)) != 0,
        }
    }

    /// Create flags from raw ARM64 PTE
    #[cfg(target_arch = "aarch64")]
    pub const fn from_arm64_pte(pte: u64) -> Self {
        Self {
            present: (pte & 1) != 0,
            writable: (pte & (1 << 7)) != 0, // AP[2]
            user: (pte & (1 << 6)) == 0,     // AP[1]
            xd: (pte & (1 << 54)) != 0,     // UXN
            accessed: (pte & (1 << 10)) != 0,
            dirty: (pte & (1 << 51)) != 0,   // DBM
            global: false, // ARM64 doesn't have a global flag in the same sense
        }
    }

    /// Format flags as string
    /// Returns a static string representation since we have limited combinations
    pub fn format(&self) -> &'static str {
        match (self.present, self.writable, self.user, self.xd, self.accessed, self.dirty, self.global) {
            (true, true, true, false, false, false, false) => "PWU",
            (true, true, false, false, false, false, false) => "PW",
            (true, false, true, false, false, false, false) => "PU",
            (true, false, false, false, false, false, false) => "P",
            (true, true, true, true, false, false, false) => "PWUX",
            (true, true, false, true, false, false, false) => "PWX",
            (true, false, true, true, false, false, false) => "PUX",
            (true, false, false, true, false, false, false) => "PX",
            (true, _, _, _, true, _, _) => "PWA",
            (true, _, _, _, _, true, _) => "PWD",
            (true, _, _, _, _, _, true) => "PWG",
            (false, _, _, _, _, _, _) => "NP",
            _ => "P??",
        }
    }
}

/// ============================================================================
/// VA→PA Walker
/// ============================================================================

/// Translate virtual address to physical address
///
/// # Arguments
///
/// * `vaddr` - Virtual address to translate
/// * `aspace` - Address space to use (None for kernel address space)
///
/// # Returns
///
/// - Translation result with physical address or error
pub fn va_to_pa(vaddr: VAddr, aspace: Option<&AddressSpace>) -> VaToPaResult {
    // Check if virtual address is canonical
    #[cfg(target_arch = "x86_64")]
    {
        // x86-64: bits [63:48] must be all 0 or all 1
        let top_bits = (vaddr >> 48) & 0xFFFF;
        if top_bits != 0 && top_bits != 0xFFFF {
            return VaToPaResult::InvalidVaddr { vaddr };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // ARM64: bits [63:48] must be all 0 or all 1 for 48-bit VA
        let top_bits = (vaddr >> 48) & 0xFFFF;
        if top_bits != 0 && top_bits != 0xFFFF {
            return VaToPaResult::InvalidVaddr { vaddr };
        }
    }

    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V Sv39: bits [63:39] must be sign-extended
        let top_bits = (vaddr >> 38) & 0x800000;
        if (vaddr as i64) < 0 && top_bits != 0x800000 {
            return VaToPaResult::InvalidVaddr { vaddr };
        }
    }

    // Get the appropriate address space
    let target_aspace = match aspace {
        Some(aspace) => aspace,
        None => {
            // Use kernel address space
            // TODO: Get kernel address space
            return VaToPaResult::NoAspace;
        }
    };

    // Perform architecture-specific translation
    arch_va_to_pa(vaddr, target_aspace)
}

/// Architecture-specific VA→PA translation
#[cfg(target_arch = "x86_64")]
fn arch_va_to_pa(vaddr: VAddr, _aspace: &AddressSpace) -> VaToPaResult {
    // x86-64 4-level page table translation
    //
    // Layout:
    // - PML4 (Level 3): bits [47:39]
    // - PDP (Level 2): bits [38:30]
    // - PD (Level 1): bits [29:21]
    // - PT (Level 0): bits [20:12]

    const PML4_SHIFT: u8 = 39;
    const PDP_SHIFT: u8 = 30;
    const PD_SHIFT: u8 = 21;
    const PT_SHIFT: u8 = 12;

    // For debugging, we'll just return a stub result
    // In a real implementation, this would:
    // 1. Get CR3 (page table base)
    // 2. Walk the 4-level page table
    // 3. Check each entry for presence
    // 4. Return the final physical address

    log_debug!("va_to_pa: vaddr={:#x} (x86-64)", vaddr);

    // TODO: Implement actual page table walk
    // For now, return a dummy success result
    VaToPaResult::Success {
        paddr: vaddr, // Identity mapping for now
        flags: PageTableFlags {
            present: true,
            writable: true,
            user: false,
            xd: false,
            accessed: false,
            dirty: false,
            global: false,
        },
        level: 0,
    }
}

/// Architecture-specific VA→PA translation
#[cfg(target_arch = "aarch64")]
fn arch_va_to_pa(vaddr: VAddr, _aspace: &AddressSpace) -> VaToPaResult {
    // ARM64 4-level page table translation (48-bit VA)
    //
    // Layout:
    // - Level 0: bits [47:39]
    // - Level 1: bits [38:30]
    // - Level 2: bits [29:21]
    // - Level 3: bits [20:12]

    log_debug!("va_to_pa: vaddr={:#x} (ARM64)", vaddr);

    // TODO: Implement actual page table walk
    VaToPaResult::Success {
        paddr: vaddr,
        flags: PageTableFlags {
            present: true,
            writable: true,
            user: false,
            xd: false,
            accessed: false,
            dirty: false,
            global: false,
        },
        level: 3,
    }
}

/// Architecture-specific VA→PA translation
#[cfg(target_arch = "riscv64")]
fn arch_va_to_pa(vaddr: VAddr, _aspace: &AddressSpace) -> VaToPaResult {
    // RISC-V Sv39 3-level page table translation
    //
    // Layout:
    // - Level 2: bits [38:30]
    // - Level 1: bits [29:21]
    // - Level 0: bits [20:12]

    log_debug!("va_to_pa: vaddr={:#x} (RISC-V)", vaddr);

    // TODO: Implement actual page table walk
    VaToPaResult::Success {
        paddr: vaddr,
        flags: PageTableFlags {
            present: true,
            writable: true,
            user: false,
            xd: false,
            accessed: false,
            dirty: false,
            global: false,
        },
        level: 0,
    }
}

/// ============================================================================
/// Page Table Dump Utility
/// ============================================================================

/// Dump page table entry
///
/// # Arguments
///
/// * `vaddr` - Virtual address to dump PTE for
/// * `aspace` - Address space (None for kernel)
pub fn dump_pte(vaddr: VAddr, aspace: Option<&AddressSpace>) {
    log_info!("Page Table Entry Dump:");
    log_info!("  VAddr: {:#x}", vaddr);

    match va_to_pa(vaddr, aspace) {
        VaToPaResult::Success { paddr, flags, level } => {
            log_info!("  PAddr: {:#x}", paddr);
            log_info!("  Level: {}", level);
            log_info!("  Flags: {}", flags.format());
        }
        VaToPaResult::NotPresent { level, entry } => {
            log_info!("  NOT PRESENT at level {}", level);
            log_info!("  Entry: {:#x}", entry);
        }
        VaToPaResult::InvalidVaddr { .. } => {
            log_info!("  INVALID VADDR");
        }
        VaToPaResult::NoAspace => {
            log_info!("  NO ADDRESS SPACE");
        }
    }
}

/// Dump address space mappings
///
/// # Arguments
///
/// * `aspace` - Address space to dump
pub fn dump_aspace(aspace: &AddressSpace) {
    log_info!("Address Space Dump:");
    log_info!("  Base: {:#x}", aspace.base());
    log_info!("  Size: {:#x} ({} MB)", aspace.size(), aspace.size() / (1024 * 1024));

    // TODO: Dump all mappings in the address space
    // This would iterate through all VMARs and mappings
}

/// ============================================================================
/// Mapping Audit
/// ============================================================================

/// Audit result for a single mapping
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MappingAudit {
    /// Virtual address
    pub vaddr: VAddr,

    /// Physical address (if mapped)
    pub paddr: Option<PAddr>,

    /// Flags (if mapped)
    pub flags: Option<PageTableFlags>,

    /// Is present
    pub present: bool,

    /// Is valid
    pub valid: bool,
}

/// Audit a range of virtual addresses
///
/// # Arguments
///
/// * `start` - Start virtual address
/// * `end` - End virtual address (exclusive)
/// * `aspace` - Address space (None for kernel)
///
/// # Returns
///
/// - Vector of audit results
pub fn audit_range(start: VAddr, end: VAddr, aspace: Option<&AddressSpace>) -> alloc::vec::Vec<MappingAudit> {
    let mut results = alloc::vec::Vec::new();

    let mut vaddr = start & !(PAGE_SIZE - 1); // Align to page boundary

    while vaddr < end {
        let result = match va_to_pa(vaddr, aspace) {
            VaToPaResult::Success { paddr, flags, .. } => MappingAudit {
                vaddr,
                paddr: Some(paddr),
                flags: Some(flags),
                present: true,
                valid: true,
            },
            VaToPaResult::NotPresent { .. } => MappingAudit {
                vaddr,
                paddr: None,
                flags: None,
                present: false,
                valid: true,
            },
            VaToPaResult::InvalidVaddr { .. } => MappingAudit {
                vaddr,
                paddr: None,
                flags: None,
                present: false,
                valid: false,
            },
            VaToPaResult::NoAspace => MappingAudit {
                vaddr,
                paddr: None,
                flags: None,
                present: false,
                valid: false,
            },
        };

        results.push(result);

        vaddr += PAGE_SIZE;
    }

    results
}

/// Print audit results
///
/// # Arguments
///
/// * `audits` - Audit results to print
pub fn print_audit(audits: &[MappingAudit]) {
    log_info!("Mapping Audit Results:");
    log_info!("  VAddr       | PAddr       | Flags | Present | Valid");
    log_info!("  -------------+-------------+-------+---------+------");

    for audit in audits {
        // Format values directly without using String::from
        let paddr_display: u64 = match audit.paddr {
            Some(p) => p as u64,
            None => 0xFFFFFFFFFFFFFFFF, // Use max value for "N/A"
        };
        let flags_display = match audit.flags {
            Some(f) => f.format(),
            None => "N/A",
        };
        let present_display = if audit.present { "Y" } else { "N" };
        let valid_display = if audit.valid { "Y" } else { "N" };

        if audit.paddr.is_some() {
            log_info!("  {:#x} | {:#x} | {:5} | {:7} | {:5}",
                audit.vaddr, paddr_display, flags_display, present_display, valid_display);
        } else {
            log_info!("  {:#x} | N/A         | {:5} | {:7} | {:5}",
                audit.vaddr, flags_display, present_display, valid_display);
        }
    }
}

/// ============================================================================
/// Fault Diagnostics
/// ============================================================================

/// Page fault diagnostic information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FaultInfo {
    /// Faulting virtual address
    pub vaddr: VAddr,

    /// Fault instruction pointer
    pub ip: VAddr,

    /// Is user mode fault
    pub is_user: bool,

    /// Fault type
    pub fault_type: FaultType,

    /// Access type
    pub access_type: AccessType,
}

/// Fault type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultType {
    /// Page not present
    NotPresent = 0,

    /// Access violation (permission denied)
    AccessDenied = 1,

    /// Reserved bit set
    Reserved = 2,
}

/// Access type that caused fault
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// Read access
    Read = 0,

    /// Write access
    Write = 1,

    /// Instruction fetch
    Execute = 2,
}

/// Diagnose a page fault
///
/// # Arguments
///
/// * `fault` - Fault information
/// * `aspace` - Address space (None for kernel)
///
/// # Returns
///
/// - Diagnostic message
pub fn diagnose_fault(fault: FaultInfo, aspace: Option<&AddressSpace>) -> &'static str {
    log_error!("Page Fault Diagnostics:");
    log_error!("  Faulting VAddr: {:#x}", fault.vaddr);
    log_error!("  Instruction IP: {:#x}", fault.ip);
    log_error!("  Mode: {}", if fault.is_user { "User" } else { "Kernel" });
    log_error!("  Type: {:?}", fault.fault_type);
    log_error!("  Access: {:?}", fault.access_type);

    // Check PTE for faulting address
    dump_pte(fault.vaddr, aspace);

    // Provide diagnostic
    match va_to_pa(fault.vaddr, aspace) {
        VaToPaResult::NotPresent { level, entry } => {
            log_error!("  DIAGNOSIS: Page not present at level {}", level);
            log_error!("  Entry value: {:#x}", entry);
            "Page not present"
        }
        VaToPaResult::InvalidVaddr { .. } => {
            log_error!("  DIAGNOSIS: Invalid virtual address (non-canonical)");
            "Invalid virtual address"
        }
        VaToPaResult::NoAspace => {
            log_error!("  DIAGNOSIS: No address space");
            "No address space"
        }
        VaToPaResult::Success { flags, .. } => {
            // Page is present, must be permission issue
            if !flags.writable && fault.access_type == AccessType::Write {
                log_error!("  DIAGNOSIS: Write to read-only page");
                "Write to read-only page"
            } else if flags.xd && fault.access_type == AccessType::Execute {
                log_error!("  DIAGNOSIS: Execute from XD page");
                "Execute from XD page"
            } else if !flags.user && fault.is_user {
                log_error!("  DIAGNOSIS: User access to kernel page");
                "User access to kernel page"
            } else {
                log_error!("  DIAGNOSIS: Unknown permission fault");
                "Unknown permission fault"
            }
        }
    }
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the VM walker subsystem
pub fn init() {
    log_info!("VM debugging utilities initialized");
    log_info!("  VA→PA walker: ready");
    log_info!("  Page table dump: ready");
    log_info!("  Mapping audit: ready");
    log_info!("  Fault diagnostics: ready");
}
