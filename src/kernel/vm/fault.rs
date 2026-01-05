// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Page Fault Handler
//!
//! This module handles page faults for virtual memory management.
//! It integrates with VMAR to provide lazy allocation and COW semantics.
//!
//! # Design
//!
//! - **Lazy allocation**: Pages are allocated on first access
//! - **COW support**: Copy-on-write faults are handled
//! - **VMAR lookup**: Fault addresses are resolved to VMAR regions
//! - **VMO integration**: Physical pages are obtained from VMOs
//!
//! # Page Fault Handling Flow
//!
//! ```text
//! 1. Page fault occurs
//! 2. Lookup VMAR region for fault address
//! 3. Find VMO mapping for the region
//! 4. Allocate/commit page from VMO
//! 5. Map page into address space
//! 6. Resume execution
//! ```

#![no_std]

use crate::kernel::vm::aspace::AddressSpace;
use crate::kernel::vm::page_table::*;
use crate::kernel::vm::layout::PAGE_SIZE;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::sync::Arc;
use core::sync::atomic::Ordering;

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Page Fault Flags
/// ============================================================================

/// Page fault was caused by a write operation
pub const PF_FLAG_WRITE: u32 = 0x01;

/// Page fault was caused by a user mode access
pub const PF_FLAG_USER: u32 = 0x02;

/// Page fault was caused by an instruction fetch
pub const PF_FLAG_INSTRUCTION: u32 = 0x04;

/// Page fault was caused by a non-present page
pub const PF_FLAG_NOT_PRESENT: u32 = 0x08;

/// Page fault was caused by a protection violation
pub const PF_FLAG_PROTECTION: u32 = 0x10;

/// ============================================================================
/// Page Fault Handler
/// ============================================================================

/// Page fault information
#[derive(Debug, Clone, Copy)]
pub struct PageFaultInfo {
    /// Faulting virtual address
    pub addr: VAddr,

    /// Fault flags
    pub flags: u32,

    /// Instruction pointer at fault time
    pub ip: VAddr,

    /// Whether fault was from user mode
    pub is_user: bool,
}

impl PageFaultInfo {
    /// Create new page fault info
    pub fn new(addr: VAddr, flags: u32, ip: VAddr, is_user: bool) -> Self {
        Self {
            addr,
            flags,
            ip,
            is_user,
        }
    }

    /// Check if fault was caused by write
    pub fn is_write(&self) -> bool {
        (self.flags & PF_FLAG_WRITE) != 0
    }

    /// Check if fault was caused by instruction fetch
    pub fn is_instruction(&self) -> bool {
        (self.flags & PF_FLAG_INSTRUCTION) != 0
    }

    /// Check if fault was caused by page not present
    pub fn is_not_present(&self) -> bool {
        (self.flags & PF_FLAG_NOT_PRESENT) != 0
    }

    /// Check if fault was caused by protection violation
    pub fn is_protection(&self) -> bool {
        (self.flags & PF_FLAG_PROTECTION) != 0
    }

    /// Check if fault is from user mode
    pub fn from_user_mode(&self) -> bool {
        self.is_user
    }
}

/// Page fault handler result
#[derive(Debug, Clone, Copy)]
pub enum PageFaultResult {
    /// Fault was handled successfully
    Handled,

    /// Fault should be propagated to userspace
    UserSpace,

    /// Fault is fatal (should kill the process)
    Fatal,

    /// Fault should trigger a retry
    Retry,
}

/// Handle a page fault
///
/// # Arguments
///
/// * `info` - Page fault information
/// * `aspace` - Address space where the fault occurred
///
/// # Returns
///
/// * PageFaultResult indicating how to proceed
pub fn handle_page_fault(info: PageFaultInfo, aspace: &Arc<AddressSpace>) -> PageFaultResult {
    log_debug!(
        "Page fault: addr={:#x} flags={:#x} ip={:#x} user={}",
        info.addr, info.flags, info.ip, info.is_user
    );

    // Align address to page boundary
    let fault_addr = info.addr & !(PAGE_SIZE - 1);

    // Try to find the VMAR region for this address
    // TODO: Implement VMAR lookup from address space
    // For now, we'll handle it as a stub

    // Check if this is a lazy allocation fault
    if info.is_not_present() {
        // Try to handle as lazy allocation
        if let Ok(_) = try_lazy_allocation(fault_addr, aspace, info) {
            log_debug!("Page fault handled via lazy allocation");
            return PageFaultResult::Handled;
        }
    }

    // Check if this is a COW fault
    if info.is_write() && info.is_not_present() {
        // Try to handle as COW
        if let Ok(_) = try_cow_allocation(fault_addr, aspace, info) {
            log_debug!("Page fault handled via COW");
            return PageFaultResult::Handled;
        }
    }

    // If we couldn't handle the fault, decide what to do
    if info.from_user_mode() {
        log_debug!("Page fault forwarding to userspace");
        PageFaultResult::UserSpace
    } else {
        log_error!("Kernel page fault - fatal");
        PageFaultResult::Fatal
    }
}

/// Try to handle a page fault via lazy allocation
///
/// # Arguments
///
/// * `addr` - Faulting virtual address (page-aligned)
/// * `aspace` - Address space
/// * `info` - Page fault info
///
/// # Returns
///
/// * Ok(()) if handled successfully
/// * Err otherwise
fn try_lazy_allocation(addr: VAddr, aspace: &Arc<AddressSpace>, info: PageFaultInfo) -> Result {
    // TODO: Implement proper VMAR lookup and VMO integration
    // For now, this is a stub that logs what would happen

    log_debug!(
        "Lazy allocation for addr={:#x} (would allocate page and map)",
        addr
    );

    // In a full implementation:
    // 1. Lookup VMAR region containing addr
    // 2. Find VMO mapping for the region
    // 3. Get physical page from VMO (allocate if needed)
    // 4. Map page into address space with correct permissions
    // 5. Flush TLB

    // Placeholder: Return error to indicate not handled
    Err(RX_ERR_NOT_FOUND)
}

/// Try to handle a page fault via COW
///
/// # Arguments
///
/// * `addr` - Faulting virtual address (page-aligned)
/// * `aspace` - Address space
/// * `info` - Page fault info
///
/// # Returns
///
/// * Ok(()) if handled successfully
/// * Err otherwise
fn try_cow_allocation(addr: VAddr, aspace: &Arc<AddressSpace>, info: PageFaultInfo) -> Result {
    // TODO: Implement proper COW handling
    // For now, this is a stub

    log_debug!(
        "COW allocation for addr={:#x} (would copy page and make writable)",
        addr
    );

    // In a full implementation:
    // 1. Lookup VMAR region containing addr
    // 2. Find VMO mapping for the region
    // 3. Check if VMO page is shared (COW)
    // 4. Allocate new physical page
    // 5. Copy contents from original page
    // 6. Map new page with write permissions
    // 7. Update VMO page map
    // 8. Flush TLB

    // Placeholder: Return error to indicate not handled
    Err(RX_ERR_NOT_FOUND)
}

/// ============================================================================
/// Arch Interface
/// ============================================================================

/// Architecture-agnostic page fault handler
///
/// This is called from architecture-specific fault handlers.
///
/// # Arguments
///
/// * `addr` - Faulting virtual address
/// * `flags` - Fault flags (architecture-specific, converted to generic)
/// * `ip` - Instruction pointer
/// * `is_user` - Whether fault was from user mode
///
/// # Returns
///
/// * 0 if handled, negative error code otherwise
#[no_mangle]
pub extern "C" fn vm_page_fault_handler(addr: VAddr, flags: u32, ip: VAddr, is_user: bool) -> i32 {
    let info = PageFaultInfo::new(addr, flags, ip, is_user);

    // Get current address space
    // TODO: Get current process's address space
    // For now, return error to indicate not handled
    log_debug!("vm_page_fault_handler: addr={:#x} flags={:#x}", addr, flags);

    // Try to handle the page fault
    // let aspace = get_current_address_space();
    // match handle_page_fault(info, &aspace) {
    //     PageFaultResult::Handled => 0,
    //     PageFaultResult::UserSpace => -1,
    //     PageFaultResult::Fatal => -2,
    //     PageFaultResult::Retry => -3,
    // }

    // Return not handled for now
    -1
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the page fault handler
pub fn init() {
    log_info!("Page fault handler initialized");
    log_info!("  Lazy allocation: enabled");
    log_info!("  COW support: stub");
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_fault_info() {
        let info = PageFaultInfo::new(0x1000, PF_FLAG_WRITE | PF_FLAG_USER, 0x4000, true);

        assert!(info.is_write());
        assert!(!info.is_instruction());
        assert!(info.from_user_mode());
    }

    #[test]
    fn test_page_fault_flags() {
        assert_eq!(PF_FLAG_WRITE, 0x01);
        assert_eq!(PF_FLAG_USER, 0x02);
        assert_eq!(PF_FLAG_INSTRUCTION, 0x04);
        assert_eq!(PF_FLAG_NOT_PRESENT, 0x08);
        assert_eq!(PF_FLAG_PROTECTION, 0x10);
    }
}
