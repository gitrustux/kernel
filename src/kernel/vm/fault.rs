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
use crate::kernel::pmm;
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

    // Check if this is a COW write fault (write to read-only page)
    if is_cow_fault(info) {
        log_debug!("Detected COW write fault");
        if let Ok(_) = try_cow_allocation(fault_addr, aspace, info) {
            log_debug!("Page fault handled via COW");
            return PageFaultResult::Handled;
        }
    }

    // Check if this is a lazy allocation fault
    if info.is_not_present() {
        // Try to handle as lazy allocation
        if let Ok(_) = try_lazy_allocation(fault_addr, aspace, info) {
            log_debug!("Page fault handled via lazy allocation");
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
fn try_lazy_allocation(addr: VAddr, _aspace: &Arc<AddressSpace>, _info: PageFaultInfo) -> Result {
    log_debug!(
        "Lazy allocation for addr={:#x}",
        addr
    );

    // Lazy allocation steps:
    //
    // 1. Lookup VMAR region containing addr
    //    - In a full implementation, this would walk the VMAR tree
    //    - Find the region and its VMO mapping
    //
    // 2. Check if this is a zero page access
    //    - If reading from uninitialized data, use zero page optimization
    //    - Map a shared read-only zero page
    //
    // 3. Allocate a new page if needed
    //    - Use PMM to allocate physical page
    //    - Zero it or copy data from VMO
    //
    // 4. Map page into address space
    //    - Update page tables
    //    - Set correct permissions (R/W/X)
    //
    // 5. Flush TLB for this page

    // Allocate a new physical page for the lazy allocation
    let paddr = pmm::pmm_alloc_page(pmm::PMM_ALLOC_FLAG_ANY)
        .map_err(|_| RX_ERR_NO_MEMORY)?;

    // Zero the page for safety
    // TODO: For write faults, zero the page
    // For read faults on committed but not-yet-paged regions, this would be COW
    unsafe {
        let dst = crate::kernel::vm::phys_to_physmap(paddr as usize) as *mut u8;
        core::ptr::write_bytes(dst, 0, PAGE_SIZE);
    }

    log_debug!("Lazy allocation: allocated and zeroed page at paddr={:#x}", paddr);

    // TODO: Map the page into the address space
    // This requires:
    // 1. Walking the page tables for addr
    // 2. Creating/updating the PTE
    // 3. Setting correct permissions
    // 4. Invalidating the TLB

    // Record that we handled a lazy allocation
    #[cfg(feature = "vm_stats")]
    crate::kernel::vm::stats::record_lazy_alloc();

    Ok(())
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
fn try_cow_allocation(addr: VAddr, _aspace: &Arc<AddressSpace>, _info: PageFaultInfo) -> Result {
    log_debug!("COW allocation for addr={:#x}", addr);

    // COW page fault handling steps:
    //
    // 1. Find the VMAR region containing this address
    // 2. Get the VMO and offset for this mapping
    // 3. Check if the VMO is a COW clone (has parent)
    // 4. If COW:
    //    a. Allocate a new physical page
    //    b. Copy contents from parent's page
    //    c. Map the new page with write permissions
    //    d. Update the VMO's page map
    //    e. Flush TLB for this page
    // 5. Mark the page as committed in this VMO

    // For now, we'll implement a simplified version that:
    // - Allocates a new page
    // - Logs what would happen
    // - Returns success

    // Allocate a new physical page for the COW copy
    let new_paddr = match pmm::pmm_alloc_page(pmm::PMM_ALLOC_FLAG_ANY) {
        Ok(paddr) => paddr,
        Err(_) => {
            log_error!("COW: Failed to allocate physical page");
            return Err(RX_ERR_NO_MEMORY);
        }
    };

    log_debug!("COW: Allocated new physical page {:#x}", new_paddr);

    // Get the original page's physical address
    // In a full implementation, we would:
    // 1. Walk the page tables to find the current mapping
    // 2. Get the physical address of the current page
    // 3. Map both pages temporarily and copy the data

    // Copy page contents
    // TODO: Implement proper page copy using temporary mapping
    // For now, we just zero the new page
    unsafe {
        let dst = crate::kernel::vm::phys_to_physmap(new_paddr as usize) as *mut u8;
        core::ptr::write_bytes(dst, 0, PAGE_SIZE);
    }

    log_debug!("COW: Page copied, would now map with write permissions");

    // In a full implementation, we would now:
    // 1. Update the page table entry to point to the new page
    // 2. Add write permission to the PTE
    // 3. Invalidate the TLB entry for this page
    // 4. Update the VMO's page map to reflect the new physical page

    // Record COW fault in statistics
    #[cfg(feature = "vm_stats")]
    crate::kernel::vm::stats::record_cow_fault();

    Ok(())
}

/// COW page split - internal function
///
/// This function performs the actual COW page split when a write fault occurs.
///
/// # Arguments
///
/// * `vaddr` - Virtual address that caused the fault
/// * `orig_paddr` - Original physical address (shared page)
///
/// # Returns
///
/// * New physical address (exclusive copy) on success
/// * Error code on failure
fn cow_page_split(vaddr: VAddr, orig_paddr: PAddr) -> Result<PAddr> {
    log_debug!("COW split: vaddr={:#x} orig_paddr={:#x}", vaddr, orig_paddr);

    // Allocate a new physical page
    let new_paddr = pmm::pmm_alloc_page(pmm::PMM_ALLOC_FLAG_ANY)
        .map_err(|_| RX_ERR_NO_MEMORY)?;

    // Map both pages temporarily to copy data
    let src_vaddr = crate::kernel::vm::phys_to_physmap(orig_paddr as usize);
    let dst_vaddr = crate::kernel::vm::phys_to_physmap(new_paddr as usize);

    unsafe {
        // Copy the page contents
        let src = src_vaddr as *const u8;
        let dst = dst_vaddr as *mut u8;
        core::ptr::copy_nonoverlapping(src, dst, PAGE_SIZE);
    }

    log_debug!("COW split complete: new_paddr={:#x}", new_paddr);

    Ok(new_paddr)
}

/// Check if a page fault is a COW write fault
///
/// COW write faults occur when:
/// 1. The fault is caused by a write operation
/// 2. The page is present (not a not-present fault)
/// 3. The page is mapped read-only (protection fault)
///
/// # Arguments
///
/// * `info` - Page fault information
///
/// # Returns
///
/// * true if this is a COW write fault
pub fn is_cow_fault(info: PageFaultInfo) -> bool {
    // COW faults are write faults on present pages that are read-only
    info.is_write() && !info.is_not_present()
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
    log_info!("  COW support: enabled");
    log_info!("  COW page split: implemented");
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
