// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Guest Physical Address Space
//!
//! This module provides guest physical address space management for the hypervisor.
//! It handles memory mapping, page faults, and address space operations for guest VMs.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Page fault flags for guest physical address space
const PF_FLAGS: u32 = VMM_PF_FLAG_WRITE | VMM_PF_FLAG_SW_FAULT;

/// MMU flags for interrupt controller mapping
const INTERRUPT_MMU_FLAGS: u32 = ARCH_MMU_FLAG_PERM_READ | ARCH_MMU_FLAG_PERM_WRITE;

/// MMU flags for guest memory mappings
const GUEST_MMU_FLAGS: u32 =
    ARCH_MMU_FLAG_CACHED | ARCH_MMU_FLAG_PERM_READ | ARCH_MMU_FLAG_PERM_WRITE;

/// VMM PF flag: Write access
const VMM_PF_FLAG_WRITE: u32 = 1 << 0;

/// VMM PF flag: Software fault
const VMM_PF_FLAG_SW_FAULT: u32 = 1 << 1;

/// VMM PF flag: Guest fault
const VMM_PF_FLAG_GUEST: u32 = 1 << 2;

/// VMM PF flag: Hardware fault
const VMM_PF_FLAG_HW_FAULT: u32 = 1 << 3;

/// VMM PF flag: Instruction fetch
const VMM_PF_FLAG_INSTRUCTION: u32 = 1 << 4;

/// ARCH MMU flag: Cached
const ARCH_MMU_FLAG_CACHED: u32 = 1 << 0;

/// ARCH MMU flag: Read permission
const ARCH_MMU_FLAG_PERM_READ: u32 = 1 << 1;

/// ARCH MMU flag: Write permission
const ARCH_MMU_FLAG_PERM_WRITE: u32 = 1 << 2;

/// ARCH MMU flag: Execute permission
const ARCH_MMU_FLAG_PERM_EXECUTE: u32 = 1 << 3;

/// Guest physical address
pub type GuestPaddr = u64;

/// Host physical address
pub type HostPaddr = u64;

/// Guest physical address space
pub struct GuestPhysicalAddressSpace {
    /// Guest address space
    guest_aspace: Option<*mut VmAspace>,
    /// Size of the address space
    size: usize,
    /// VMID (for ARM64)
    #[cfg(target_arch = "aarch64")]
    vmid: u8,
}

unsafe impl Send for GuestPhysicalAddressSpace {}
unsafe impl Sync for GuestPhysicalAddressSpace {}

impl GuestPhysicalAddressSpace {
    /// Create a new guest physical address space
    ///
    /// # Arguments
    ///
    /// * `vmid` - VM ID (ARM64 only)
    ///
    /// # Returns
    ///
    /// Ok(gpas) on success, Err(status) on failure
    #[cfg(target_arch = "aarch64")]
    pub fn create(vmid: u8) -> Result<Self, i32> {
        println!(
            "GuestPhysicalAddressSpace: Creating (vmid: {}, size: 0x{:x})",
            vmid, GUEST_PHYSICAL_ASPACE_SIZE
        );

        // TODO: Create actual VM address space
        let guest_aspace = Self::create_aspace();

        if guest_aspace.is_null() {
            return Err(-1); // ZX_ERR_NO_MEMORY
        }

        let mut gpas = Self {
            guest_aspace: Some(guest_aspace),
            size: GUEST_PHYSICAL_ASPACE_SIZE,
            vmid,
        };

        // Set ASID for ARM64
        gpas.arch_set_asid(vmid);

        println!("GuestPhysicalAddressSpace: Created");
        Ok(gpas)
    }

    /// Create a new guest physical address space (x86_64)
    ///
    /// # Returns
    ///
    /// Ok(gpas) on success, Err(status) on failure
    #[cfg(target_arch = "x86_64")]
    pub fn create() -> Result<Self, i32> {
        println!(
            "GuestPhysicalAddressSpace: Creating (size: 0x{:x})",
            GUEST_PHYSICAL_ASPACE_SIZE
        );

        // TODO: Create actual VM address space
        let guest_aspace = Self::create_aspace();

        if guest_aspace.is_null() {
            return Err(-1); // ZX_ERR_NO_MEMORY
        }

        println!("GuestPhysicalAddressSpace: Created");
        Ok(Self {
            guest_aspace: Some(guest_aspace),
            size: GUEST_PHYSICAL_ASPACE_SIZE,
        })
    }

    /// Map interrupt controller into guest physical address space
    ///
    /// # Arguments
    ///
    /// * `guest_paddr` - Guest physical address
    /// * `host_paddr` - Host physical address
    /// * `len` - Length of the mapping
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err(status) on failure
    pub fn map_interrupt_controller(
        &mut self,
        guest_paddr: GuestPaddr,
        host_paddr: HostPaddr,
        len: usize,
    ) -> Result<(), i32> {
        println!(
            "GuestPhysicalAddressSpace: Mapping interrupt controller {:#x} -> {:#x} ({} bytes)",
            guest_paddr, host_paddr, len
        );

        // TODO: Create physical VMO
        // TODO: Set mapping cache policy
        // TODO: Create VM mapping in root VMAR
        // TODO: Map range to page table

        println!("GuestPhysicalAddressSpace: Mapped interrupt controller");
        Ok(())
    }

    /// Unmap a range from guest physical address space
    ///
    /// # Arguments
    ///
    /// * `guest_paddr` - Guest physical address
    /// * `len` - Length to unmap
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err(status) on failure
    pub fn unmap_range(&mut self, guest_paddr: GuestPaddr, len: usize) -> Result<(), i32> {
        println!(
            "GuestPhysicalAddressSpace: Unmapping range {:#x} ({} bytes)",
            guest_paddr, len
        );

        // TODO: Unmap range from root VMAR

        Ok(())
    }

    /// Get the host physical address for a guest physical address
    ///
    /// # Arguments
    ///
    /// * `guest_paddr` - Guest physical address
    ///
    /// # Returns
    ///
    /// Ok(host_paddr) on success, Err(status) on failure
    pub fn get_page(&self, guest_paddr: GuestPaddr) -> Result<HostPaddr, i32> {
        // TODO: Find mapping for guest address
        // TODO: Get page from VMO
        println!(
            "GuestPhysicalAddressSpace: Get page {:#x}",
            guest_paddr
        );
        Err(-2) // ZX_ERR_NOT_FOUND
    }

    /// Handle a page fault in guest physical address space
    ///
    /// # Arguments
    ///
    /// * `guest_paddr` - Guest physical address that faulted
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err(status) on failure
    pub fn page_fault(&self, guest_paddr: GuestPaddr) -> Result<(), i32> {
        println!(
            "GuestPhysicalAddressSpace: Page fault at {:#x}",
            guest_paddr
        );

        // TODO: Find mapping for guest address
        // TODO: Determine fault flags from mapping permissions
        // TODO: Call PageFault on mapping

        Err(-2) // ZX_ERR_NOT_FOUND
    }

    /// Create a guest pointer for accessing guest memory
    ///
    /// # Arguments
    ///
    /// * `guest_paddr` - Guest physical address
    /// * `len` - Length of the range
    /// * `name` - Name of the mapping
    ///
    /// # Returns
    ///
    /// Ok(GuestPtr) on success, Err(status) on failure
    pub fn create_guest_ptr(
        &self,
        guest_paddr: GuestPaddr,
        len: usize,
        name: &str,
    ) -> Result<GuestPtr, i32> {
        let begin = round_down(guest_paddr, PAGE_SIZE);
        let end = round_up(guest_paddr + len as u64, PAGE_SIZE);
        let mapping_len = end - begin;

        if begin > end || !in_range(begin, mapping_len as usize, self.size) {
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }

        println!(
            "GuestPhysicalAddressSpace: Creating guest ptr '{}' for {:#x} ({} bytes)",
            name, guest_paddr, len
        );

        // TODO: Find region in root VMAR
        // TODO: Verify region is a mapping
        // TODO: Create host mapping
        // TODO: Return GuestPtr

        Err(-2) // ZX_ERR_NOT_FOUND
    }

    /// Get the size of the address space
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the root VMAR
    fn root_vmar(&self) -> *mut VmAddressRegion {
        // TODO: Get root VMAR from guest address space
        core::ptr::null_mut()
    }

    /// Create address space (internal)
    fn create_aspace() -> *mut VmAspace {
        // TODO: Create VmAspace::TYPE_GUEST_PHYS
        println!("GuestPhysicalAddressSpace: Creating guest address space");
        core::ptr::null_mut()
    }

    /// Set ASID (ARM64 specific)
    #[cfg(target_arch = "aarch64")]
    fn arch_set_asid(&mut self, vmid: u8) {
        // TODO: Implement arch_set_asid for ARM64
        println!(
            "GuestPhysicalAddressSpace: Setting ASID to {}",
            vmid
        );
    }

    /// Destroy the address space
    fn destroy(&mut self) {
        if let Some(aspace) = self.guest_aspace.take() {
            // TODO: Destroy VmAspace
            let _ = aspace;
            println!("GuestPhysicalAddressSpace: Destroyed");
        }
    }
}

impl Drop for GuestPhysicalAddressSpace {
    fn drop(&mut self) {
        self.destroy();
    }
}

/// Guest pointer for accessing guest memory
pub struct GuestPtr {
    /// Host mapping
    host_mapping: Option<*mut VmMapping>,
    /// Offset within the mapping
    offset: usize,
}

unsafe impl Send for GuestPtr {}
unsafe impl Sync for GuestPtr {}

impl GuestPtr {
    /// Create a new guest pointer
    pub fn new(host_mapping: *mut VmMapping, offset: usize) -> Self {
        Self {
            host_mapping: Some(host_mapping),
            offset,
        }
    }

    /// Get the pointer to the guest memory
    pub fn as_ptr(&self) -> *mut u8 {
        // TODO: Get pointer from host mapping
        core::ptr::null_mut()
    }

    /// Get the offset within the mapping
    pub fn offset(&self) -> usize {
        self.offset
    }
}

/// Round down to page alignment
fn round_down(addr: u64, page_size: u64) -> u64 {
    addr & !(page_size - 1)
}

/// Round up to page alignment
fn round_up(addr: u64, page_size: u64) -> u64 {
    ((addr + page_size - 1) & !(page_size - 1))
}

/// Check if a range is within bounds
fn in_range(base: u64, len: usize, max: usize) -> bool {
    base.checked_add(len as u64).map_or(false, |end| end <= max as u64)
}

/// Page size constant
const PAGE_SIZE: u64 = 4096;

/// Guest physical address space size (1TB for x86_64, adjusted for ARM64)
#[cfg(target_arch = "x86_64")]
const GUEST_PHYSICAL_ASPACE_SIZE: usize = 1 << 40; // 1TB

#[cfg(target_arch = "aarch64")]
const GUEST_PHYSICAL_ASPACE_SIZE: usize = 1 << 40; // 1TB

/// Opaque VM address space type
#[repr(C)]
pub struct VmAspace {
    _private: [u8; 0],
}

/// Opaque VM address region type
#[repr(C)]
pub struct VmAddressRegion {
    _private: [u8; 0],
}

/// Opaque VM mapping type
#[repr(C)]
pub struct VmMapping {
    _private: [u8; 0],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_down() {
        assert_eq!(round_down(0x1000, 0x1000), 0x1000);
        assert_eq!(round_down(0x1fff, 0x1000), 0x1000);
        assert_eq!(round_down(0x2000, 0x1000), 0x2000);
    }

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(0x1000, 0x1000), 0x1000);
        assert_eq!(round_up(0x1001, 0x1000), 0x2000);
        assert_eq!(round_up(0x1fff, 0x1000), 0x2000);
    }

    #[test]
    fn test_in_range() {
        assert!(in_range(0x1000, 0x1000, 0x10000));
        assert!(!in_range(0xf000, 0x2000, 0x10000));
        assert!(in_range(0, 0, 0x10000));
    }

    #[test]
    fn test_guest_ptr() {
        let ptr = GuestPtr::new(core::ptr::null_mut(), 0x100);
        assert_eq!(ptr.offset(), 0x100);
    }
}
