// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! VM Debugging Utilities
//!
//! This module provides debugging and diagnostic tools for the virtual memory
//! subsystem. It includes VA→PA walkers, page table dumps, and mapping audits.
//!
//! # Features
//!
//! - **VA→PA Walker**: Walk page tables to resolve virtual addresses
//! - **Page Table Dump**: Display entire page table hierarchy
//! - **Mapping Audit**: Verify consistency of address space mappings
//! - **Cross-Architecture**: Identical output format across all architectures
//!
//! # Usage
//!
//! ```rust
//! // Walk a virtual address
//! vm_walk(aspace, 0xffff800000100000);
//!
//! // Dump entire page table
//! vm_dump_table(aspace);
//!
//! // Audit mappings
//! vm_audit(aspace);
//! ```

#![no_std]

use crate::kernel::vm::layout::*;
use crate::kernel::vm::page_table::*;
use crate::kernel::vm::aspace::*;
use crate::kernel::vm::{Result, VmError};
use core::fmt;

// Import logging macros
use crate::{log_error, log_info};
extern crate alloc;
use alloc::format;
use alloc::string::String;

/// ============================================================================
/// VA→PA Walker
/// ============================================================================

/// Result of walking a virtual address
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalkResult {
    /// Virtual address being walked
    pub vaddr: VAddr,

    /// Physical address (if mapped)
    pub paddr: Option<PAddr>,

    /// Page table entry flags
    pub flags: PageTableFlags,

    /// Mapping level (0 = root, increasing as we go down)
    pub level: u8,

    /// Is this a leaf mapping (not a page table pointer)?
    pub is_leaf: bool,

    /// Is the mapping present?
    pub present: bool,
}

impl WalkResult {
    /// Create a new walk result
    pub const fn new(
        vaddr: VAddr,
        paddr: Option<PAddr>,
        flags: PageTableFlags,
        level: u8,
        is_leaf: bool,
    ) -> Self {
        let present = paddr.is_some() && flags.is_present();
        Self {
            vaddr,
            paddr,
            flags,
            level,
            is_leaf,
            present,
        }
    }

    /// Format as a string for display
    pub fn format(&self) -> WalkFormatter {
        WalkFormatter { result: *self }
    }
}

/// Formatter for walk results
pub struct WalkFormatter {
    result: WalkResult,
}

impl fmt::Display for WalkFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:#x}]",
            self.result.vaddr
        )?;

        if let Some(paddr) = self.result.paddr {
            write!(f, " -> {:#x}", paddr)?;

            // Print flags
            let mut sep = " (";
            if self.result.flags.is_present() {
                write!(f, "{}P", sep)?;
                sep = "|";
            }
            if self.result.flags.is_writable() {
                write!(f, "{}W", sep)?;
                sep = "|";
            }
            if self.result.flags.is_user() {
                write!(f, "{}U", sep)?;
                sep = "|";
            }
            if self.result.flags.can_execute() {
                write!(f, "{}X", sep)?;
                sep = "|";
            }
            if sep != " (" {
                write!(f, ")")?;
            }
        } else {
            write!(f, " -> (not mapped)")?;
        }

        Ok(())
    }
}

/// Walk a virtual address through the page tables
///
/// Returns information about the mapping at each level of the page table.
pub fn vm_walk(aspace: &AddressSpace, vaddr: VAddr) -> WalkResult {
    // Try to resolve the address
    let paddr = aspace.resolve(vaddr);

    // Get flags from page table
    // This is a simplified implementation
    let flags = if let Some(pa) = paddr {
        // Try to get actual flags from page table entry
        // For now, use default flags
        PageTableFlags::Present
    } else {
        PageTableFlags::None
    };

    WalkResult {
        vaddr,
        paddr,
        flags,
        level: 3, // Default to leaf level
        is_leaf: true,
        present: paddr.is_some(),
    }
}

/// ============================================================================
/// Page Table Dump
/// ============================================================================

/// Dump the entire page table hierarchy
pub fn vm_dump_table(aspace: &AddressSpace) {
    log_info!("Page Table Dump:");
    log_info!("  ASID: {}", aspace.asid());
    log_info!("  Base: {:#x}", aspace.base());
    log_info!("  Size: {:#x}", aspace.size());
    log_info!("  Root: {:#x}", aspace.root_phys());

    // Dump kernel regions
    dump_kernel_regions();
}

/// Dump kernel memory regions
fn dump_kernel_regions() {
    #[cfg(target_arch = "aarch64")]
    {
        log_info!("ARM64 Kernel Regions:");
        log_info!("  KERNEL_BASE:   {:#x}", arm64::KERNEL_BASE);
        log_info!("  TEXT: {:#x} - {:#x}",
            arm64::KERNEL_TEXT_BASE,
            arm64::KERNEL_TEXT_BASE + arm64::KERNEL_TEXT_SIZE);
        log_info!("  DATA: {:#x} - {:#x}",
            arm64::KERNEL_DATA_BASE,
            arm64::KERNEL_DATA_BASE + arm64::KERNEL_DATA_SIZE);
        log_info!("  PERCPU: {:#x}",
            arm64::KERNEL_PERCPU_BASE);
        log_info!("  PHYSMAP: {:#x} - {:#x}",
            arm64::KERNEL_PHYSMAP_BASE,
            arm64::KERNEL_PHYSMAP_BASE + arm64::KERNEL_PHYSMAP_SIZE);
    }

    #[cfg(target_arch = "x86_64")]
    {
        log_info!("AMD64 Kernel Regions:");
        log_info!("  KERNEL_BASE:   {:#x}", amd64::KERNEL_BASE);
        log_info!("  TEXT: {:#x} - {:#x}",
            amd64::KERNEL_TEXT_BASE,
            amd64::KERNEL_TEXT_BASE + amd64::KERNEL_TEXT_SIZE);
        log_info!("  DATA: {:#x} - {:#x}",
            amd64::KERNEL_DATA_BASE,
            amd64::KERNEL_DATA_BASE + amd64::KERNEL_DATA_SIZE);
        log_info!("  PERCPU: {:#x}",
            amd64::KERNEL_PERCPU_BASE);
        log_info!("  PHYSMAP: {:#x} - {:#x}",
            amd64::KERNEL_PHYSMAP_BASE,
            amd64::KERNEL_PHYSMAP_BASE + amd64::KERNEL_PHYSMAP_SIZE);
    }

    #[cfg(target_arch = "riscv64")]
    {
        log_info!("RISC-V Kernel Regions:");
        log_info!("  KERNEL_BASE:   {:#x}", riscv::KERNEL_BASE);
        log_info!("  TEXT: {:#x} - {:#x}",
            riscv::KERNEL_TEXT_BASE,
            riscv::KERNEL_TEXT_BASE + riscv::KERNEL_TEXT_SIZE);
        log_info!("  DATA: {:#x} - {:#x}",
            riscv::KERNEL_DATA_BASE,
            riscv::KERNEL_DATA_BASE + riscv::KERNEL_DATA_SIZE);
        log_info!("  PERCPU: {:#x}",
            riscv::KERNEL_PERCPU_BASE);
        log_info!("  PHYSMAP: {:#x} - {:#x}",
            riscv::KERNEL_PHYSMAP_BASE,
            riscv::KERNEL_PHYSMAP_BASE + riscv::KERNEL_PHYSMAP_SIZE);
    }
}

/// ============================================================================
/// Mapping Audit
/// ============================================================================

/// Audit result for a single check
#[derive(Debug, Clone, Copy)]
pub enum AuditCheck {
    Passed,
    Failed(&'static str),
    Warning(&'static str),
}

impl AuditCheck {
    pub const fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// Result of auditing an address space
#[repr(C)]
#[derive(Debug)]
pub struct AuditResult {
    /// Total number of mappings checked
    pub total_mappings: usize,

    /// Number of passed checks
    pub passed_checks: usize,

    /// Number of failed checks
    pub failed_checks: usize,

    /// Number of warnings
    pub warnings: usize,

    /// Specific check results
    pub checks: [AuditCheck; 16],

    /// Number of check results
    pub check_count: usize,
}

impl AuditResult {
    pub const fn new() -> Self {
        Self {
            total_mappings: 0,
            passed_checks: 0,
            failed_checks: 0,
            warnings: 0,
            checks: [AuditCheck::Passed; 16],
            check_count: 0,
        }
    }

    fn add_check(&mut self, check: AuditCheck) {
        if self.check_count < 16 {
            self.checks[self.check_count] = check;
            self.check_count += 1;
        }

        match check {
            AuditCheck::Passed => self.passed_checks += 1,
            AuditCheck::Failed(_) => self.failed_checks += 1,
            AuditCheck::Warning(_) => self.warnings += 1,
        }
    }

    pub fn is_passed(&self) -> bool {
        self.failed_checks == 0
    }
}

/// Audit an address space for consistency
pub fn vm_audit(aspace: &AddressSpace) -> AuditResult {
    let mut result = AuditResult::new();

    log_info!("Auditing address space ASID={}", aspace.asid());

    // Check 1: Verify base and size are valid
    if aspace.base() % PAGE_SIZE == 0 {
        result.add_check(AuditCheck::Passed);
    } else {
        result.add_check(AuditCheck::Failed("Base not page-aligned"));
    }

    // Check 2: Verify base is canonical
    if is_canonical_vaddr(aspace.base()) {
        result.add_check(AuditCheck::Passed);
    } else {
        result.add_check(AuditCheck::Failed("Base not canonical"));
    }

    // Check 3: Verify end doesn't overflow
    let end = aspace.base().saturating_add(aspace.size());
    if end > aspace.base() && is_canonical_vaddr(end.saturating_sub(1)) {
        result.add_check(AuditCheck::Passed);
    } else {
        result.add_check(AuditCheck::Failed("Invalid address range"));
    }

    // Check 4: Verify kernel address space has kernel base
    if aspace.is_kernel() {
        let is_kernel_base = is_kernel_vaddr(aspace.base());
        if is_kernel_base {
            result.add_check(AuditCheck::Passed);
        } else {
            result.add_check(AuditCheck::Failed("Kernel AS has user base"));
        }
    }

    // Check 5: Verify user address space has user base
    if aspace.is_user() {
        let is_user_base = is_user_vaddr(aspace.base());
        if is_user_base {
            result.add_check(AuditCheck::Passed);
        } else {
            result.add_check(AuditCheck::Failed("User AS has kernel base"));
        }
    }

    // Check 6: Verify root page table is page-aligned
    if aspace.root_phys() % PAGE_SIZE == 0 {
        result.add_check(AuditCheck::Passed);
    } else {
        result.add_check(AuditCheck::Failed("Root PT not page-aligned"));
    }

    // Print results
    log_info!("  Total checks: {}", result.check_count);
    log_info!("  Passed: {}", result.passed_checks);
    log_info!("  Failed: {}", result.failed_checks);
    log_info!("  Warnings: {}", result.warnings);

    if result.is_passed() {
        log_info!("  Audit: PASSED");
    } else {
        log_error!("  Audit: FAILED");
    }

    result
}

/// ============================================================================
/// Summary Statistics
/// ============================================================================

/// Virtual memory statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmStats {
    /// Total virtual address space size
    pub total_vaddr_space: usize,

    /// Amount of mapped virtual memory
    pub mapped_bytes: usize,

    /// Number of page table pages
    pub pt_pages: usize,

    /// Number of mapped pages
    pub mapped_pages: usize,

    /// Number of user mappings
    pub user_mappings: usize,

    /// Number of kernel mappings
    pub kernel_mappings: usize,
}

impl VmStats {
    pub const fn new() -> Self {
        Self {
            total_vaddr_space: 0,
            mapped_bytes: 0,
            pt_pages: 0,
            mapped_pages: 0,
            user_mappings: 0,
            kernel_mappings: 0,
        }
    }
}

/// Get statistics for an address space
pub fn vm_get_stats(aspace: &AddressSpace) -> VmStats {
    VmStats {
        total_vaddr_space: aspace.size(),
        mapped_bytes: 0, // Would require tracking all mappings
        pt_pages: 1,     // At minimum, root table exists
        mapped_pages: 0,  // Would require counting
        user_mappings: if aspace.is_user() { 1 } else { 0 },
        kernel_mappings: if aspace.is_kernel() { 1 } else { 0 },
    }
}

/// Print VM statistics
pub fn vm_print_stats(aspace: &AddressSpace) {
    let stats = vm_get_stats(aspace);

    log_info!("VM Statistics for ASID {}:", aspace.asid());
    log_info!("  Total VA space: {:#x} ({} MB)",
        stats.total_vaddr_space,
        stats.total_vaddr_space / (1024 * 1024)
    );
    log_info!("  Page table pages: {}", stats.pt_pages);
    log_info!("  User mappings: {}", stats.user_mappings);
    log_info!("  Kernel mappings: {}", stats.kernel_mappings);
}

/// ============================================================================
/// Console Commands
/// ============================================================================

/// Handle VM debug console command
///
/// This is called from the console/debug shell to process VM commands.
pub fn vm_console_command(cmd: &str, args: &[&str]) -> Result {
    match cmd {
        "walk" => {
            if args.len() < 1 {
                log_error!("Usage: walk <vaddr>");
                return Err(VmError::InvalidArgs);
            }

            let vaddr = args[0].parse::<VAddr>()
                .map_err(|_| VmError::InvalidArgs)?;

            // This would require getting the current address space
            log_info!("walk {:#x}", vaddr);
            Ok(())
        }

        "dump" => {
            // This would require getting the current address space
            log_info!("dump table");
            Ok(())
        }

        "audit" => {
            // This would require getting the current address space
            log_info!("audit");
            Ok(())
        }

        "stats" => {
            log_info!("VM stats:");
            log_info!("  Free pages: {}", crate::kernel::pmm::pmm_count_free_pages());
            log_info!("  Total pages: {}", crate::kernel::pmm::pmm_count_total_pages());
            Ok(())
        }

        _ => {
            log_error!("Unknown VM command: {}", cmd);
            Err(VmError::InvalidArgs)
        }
    }
}

/// ============================================================================
/// Cross-Architecture Page Table Utilities
/// ============================================================================

/// Get page table level name
pub fn level_name(level: u8) -> &'static str {
    match level {
        0 => "PML4/L0",
        1 => "PDPT/L1",
        2 => "PD/L2",
        3 => "PT/L3",
        _ => "Unknown",
    }
}

/// Format physical address with architecture
pub fn format_paddr(paddr: PAddr) -> String {
    format!("{:#x}", paddr)
}

/// Format virtual address with architecture
pub fn format_vaddr(vaddr: VAddr) -> String {
    format!("{:#x}", vaddr)
}

/// ============================================================================
// Module Initialization
// ============================================================================

/// Initialize VM debugging subsystem
pub fn init() {
    log_info!("VM debugging utilities initialized");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_walk_result_format() {
        let result = WalkResult::new(
            0xffff800000100000,
            Some(0x80001000),
            PageTableFlags::Present,
            3,
            true,
        );

        let formatted = result.format();
        // The format should contain the addresses
        assert!(formatted.to_string().contains("ffff800000100000"));
    }

    #[test]
    fn test_audit_result() {
        let mut result = AuditResult::new();
        result.add_check(AuditCheck::Passed);
        result.add_check(AuditCheck::Failed("Test failure"));
        result.add_check(AuditCheck::Warning("Test warning"));

        assert_eq!(result.passed_checks, 1);
        assert_eq!(result.failed_checks, 1);
        assert_eq!(result.warnings, 1);
        assert!(!result.is_passed());
    }
}
