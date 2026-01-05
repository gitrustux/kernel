// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! VMAR System Calls
//!
//! This module implements the VMAR (Virtual Memory Address Region) system calls.
//! VMARs manage virtual memory address spaces and mappings.
//!
//! # Syscalls Implemented
//!
//! - `rx_vmar_allocate` - Allocate a new VMAR
//! - `rx_vmar_map` - Map a VMO into address space
//! - `rx_vmar_unmap` - Unmap a region
//! - `rx_vmar_protect` - Change memory protection
//! - `rx_vmar_destroy` - Destroy a VMAR and all children
//!
//! # Design
//!
//! - Hierarchical address regions (parent-child relationships)
//! - Region tree for efficient allocation and overlap detection
//! - VMO mapping with permissions and cache policy
//! - Protection flags (READ/WRITE/EXECUTE)
//! - Address space management with proper alignment

#![no_std]

use crate::kernel::object::vmo::{self, Vmo, VmoId};
use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::sync::Mutex;
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::kernel::usercopy::{copy_to_user, UserPtr};
use crate::kernel::vm::aspace::*;
use crate::kernel::vm::layout::*;
use crate::kernel::vm::page_table::*;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use crate::kernel::vm::MemProt;
use alloc::boxed::Box;
use alloc::collections::btree_map::Entry;
use alloc::sync::Arc;
use alloc::vec::Vec;

// Import logging macros
use crate::{log_debug, log_error, log_info};
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// ============================================================================
/// VMAR Options
/// ============================================================================

/// VMAR options
pub mod vmar_options {
    use crate::kernel::vm::MemProt;

    /// Permission flags
    pub const PERM_READ: u32 = 0x01;
    pub const PERM_WRITE: u32 = 0x02;
    pub const PERM_EXECUTE: u32 = 0x04;

    /// Capability flags
    pub const CAN_MAP_READ: u32 = 0x08;
    pub const CAN_MAP_WRITE: u32 = 0x10;
    pub const CAN_MAP_EXECUTE: u32 = 0x20;

    /// Specific flags
    pub const SPECIFIC: u32 = 0x100;
    pub const SPECIFIC_OVERWRITE: u32 = 0x200;
    pub const MAP_RANGE: u32 = 0x400;
    pub const REQUIRE_NON_RESIZABLE: u32 = 0x800;
    pub const ALLOW_NON_RESIZABLE: u32 = 0x1000;

    /// All permission flags
    pub const PERM_FLAGS: u32 = PERM_READ | PERM_WRITE | PERM_EXECUTE;

    /// All capability flags
    pub const CAN_MAP_FLAGS: u32 = CAN_MAP_READ | CAN_MAP_WRITE | CAN_MAP_EXECUTE;

    /// Convert permission flags to memory protection
    pub fn perm_to_prot(perm: u32) -> MemProt {
        let mut prot = MemProt::None;
        if perm & PERM_READ != 0 {
            prot |= MemProt::Read;
        }
        if perm & PERM_WRITE != 0 {
            prot |= MemProt::Write;
        }
        if perm & PERM_EXECUTE != 0 {
            prot |= MemProt::Execute;
        }
        prot
    }

    /// Convert memory protection to permission flags
    pub fn prot_to_perm(prot: MemProt) -> u32 {
        let mut perm = 0;
        if prot.can_read() {
            perm |= PERM_READ;
        }
        if prot.can_write() {
            perm |= PERM_WRITE;
        }
        if prot.can_execute() {
            perm |= PERM_EXECUTE;
        }
        perm
    }
}

/// ============================================================================
/// VMAR Types
/// ============================================================================

/// VMAR ID type
pub type VmarId = u64;

/// Invalid VMAR ID
const VMAR_ID_INVALID: VmarId = 0;

/// Mapping state within a VMAR
#[derive(Debug)]
pub enum VmarRegion {
    /// A child VMAR
    Vmar {
        /// Child VMAR reference
        vmar: Arc<Vmar>,
    },

    /// A VMO mapping
    Mapping {
        /// VMO being mapped
        vmo: Arc<Vmo>,

        /// Offset within the VMO
        vmo_offset: u64,

        /// Size of the mapping
        size: u64,

        /// Memory protection
        prot: MemProt,

        /// Cache policy
        cache_policy: vmo::CachePolicy,
    },
}

impl VmarRegion {
    /// Get the base address of this region
    pub fn base(&self) -> u64 {
        match self {
            VmarRegion::Vmar { vmar } => vmar.base,
            VmarRegion::Mapping { .. } => {
                // Base is stored separately in the region map
                0
            }
        }
    }

    /// Get the size of this region
    pub fn size(&self) -> u64 {
        match self {
            VmarRegion::Vmar { vmar } => vmar.size,
            VmarRegion::Mapping { size, .. } => *size,
        }
    }
}

/// ============================================================================
/// VMAR Structure
/// ============================================================================

/// Virtual Memory Address Region
///
/// VMARs represent a region of virtual address space that can contain
/// child VMARs or VMO mappings. They form a hierarchical tree structure.
pub struct Vmar {
    /// VMAR ID
    id: VmarId,

    /// Base address of this VMAR (relative to parent)
    pub base: u64,

    /// Size of this VMAR
    pub size: u64,

    /// Parent VMAR (None for root VMAR)
    parent: Option<*const Vmar>,

    /// Child regions keyed by base address
    children: Mutex<BTreeMap<u64, VmarRegion>>,

    /// VMAR state flags
    flags: VmarFlags,

    /// Alignment mask for allocations (0 = no alignment requirement)
    align_mask: u64,

    /// Reference count
    ref_count: AtomicU64,
}

/// VMAR flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VmarFlags {
    /// Can map with READ permission
    can_map_read: bool,

    /// Can map with WRITE permission
    can_map_write: bool,

    /// Can map with EXECUTE permission
    can_map_execute: bool,

    /// Is this a root VMAR?
    is_root: bool,

    /// Has this VMAR been destroyed?
    destroyed: bool,
}

impl VmarFlags {
    /// Create empty flags
    const fn new() -> Self {
        Self {
            can_map_read: false,
            can_map_write: false,
            can_map_execute: false,
            is_root: false,
            destroyed: false,
        }
    }

    /// Create from VMAR options
    fn from_options(options: u32, is_root: bool) -> Self {
        Self {
            can_map_read: (options & vmar_options::CAN_MAP_READ) != 0,
            can_map_write: (options & vmar_options::CAN_MAP_WRITE) != 0,
            can_map_execute: (options & vmar_options::CAN_MAP_EXECUTE) != 0,
            is_root,
            destroyed: false,
        }
    }

    /// Check if a mapping with given permissions is allowed
    fn can_map(&self, perm: u32) -> bool {
        if (perm & vmar_options::PERM_READ) != 0 && !self.can_map_read {
            return false;
        }
        if (perm & vmar_options::PERM_WRITE) != 0 && !self.can_map_write {
            return false;
        }
        if (perm & vmar_options::PERM_EXECUTE) != 0 && !self.can_map_execute {
            return false;
        }
        true
    }
}

unsafe impl Send for Vmar {}
unsafe impl Sync for Vmar {}

impl Vmar {
    /// Create a new root VMAR
    pub fn new_root(base: u64, size: u64) -> Arc<Self> {
        let id = Self::alloc_id();

        Arc::new(Self {
            id,
            base,
            size,
            parent: None,
            children: Mutex::new(BTreeMap::new()),
            flags: VmarFlags {
                can_map_read: true,
                can_map_write: true,
                can_map_execute: true,
                is_root: true,
                destroyed: false,
            },
            align_mask: 0,
            ref_count: AtomicU64::new(1),
        })
    }

    /// Create a new child VMAR
    pub fn new_child(
        parent: &Arc<Vmar>,
        offset: u64,
        size: u64,
        options: u32,
        align_mask: u64,
    ) -> Result<Arc<Self>> {
        // Validate offset is within parent
        if offset + size > parent.size {
            return Err(RX_ERR_INVALID_ARGS);
        }

        // Validate alignment
        if offset & align_mask != 0 {
            return Err(RX_ERR_INVALID_ARGS);
        }
        if size & align_mask != (align_mask & (align_mask + 1).saturating_sub(1)) {
            // Size must be aligned to alignment
            return Err(RX_ERR_INVALID_ARGS);
        }

        let id = Self::alloc_id();

        let child = Arc::new(Self {
            id,
            base: offset,
            size,
            parent: Some(Arc::as_ptr(parent) as *const Vmar),
            children: Mutex::new(BTreeMap::new()),
            flags: VmarFlags::from_options(options, false),
            align_mask,
            ref_count: AtomicU64::new(1),
        });

        // Add to parent's children
        let mut parent_children = parent.children.lock();
        if parent_children.contains_key(&offset) {
            return Err(RX_ERR_ALREADY_EXISTS);
        }
        parent_children.insert(offset, VmarRegion::Vmar { vmar: child.clone() });

        Ok(child)
    }

    /// Allocate a VMAR ID
    fn alloc_id() -> VmarId {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// Get the VMAR ID
    pub fn id(&self) -> VmarId {
        self.id
    }

    /// Check if this VMAR has been destroyed
    pub fn is_destroyed(&self) -> bool {
        self.flags.destroyed
    }

    /// Destroy this VMAR and all children
    pub fn destroy(&self) -> Result {
        if self.flags.is_root {
            return Err(RX_ERR_ACCESS_DENIED);
        }

        // Mark as destroyed
        // Note: In a real implementation, we'd need to atomically set this
        // and properly handle concurrent access

        Ok(())
    }

    /// Find a free region in this VMAR
    fn find_free_region(&self, size: u64, alignment: u64) -> Option<u64> {
        let children = self.children.lock();
        let mut prev_end = 0u64;

        for (&base, region) in children.iter() {
            let region_size = region.size();
            let region_end = base + region_size;

            // Check if there's space between prev_end and this region
            let aligned_start = (prev_end + alignment - 1) & !(alignment - 1);
            if aligned_start + size <= base {
                return Some(aligned_start);
            }

            prev_end = region_end;
        }

        // Check space after last region
        let aligned_start = (prev_end + alignment - 1) & !(alignment - 1);
        if aligned_start + size <= self.size {
            Some(aligned_start)
        } else {
            None
        }
    }

    /// Check if a range overlaps with any existing regions
    fn check_overlap(&self, offset: u64, size: u64) -> Result {
        let children = self.children.lock();
        let end = offset + size;

        for (&base, region) in children.iter() {
            let region_end = base + region.size();

            // Check for overlap: [offset, end) overlaps [base, region_end)
            // if offset < region_end && end > base
            if offset < region_end && end > base {
                return Err(RX_ERR_ALREADY_EXISTS);
            }
        }

        Ok(())
    }

    /// Map a VMO into this VMAR (without address space binding)
    pub fn map(
        &self,
        vmo: Arc<Vmo>,
        vmar_offset: u64,
        vmo_offset: u64,
        size: u64,
        prot: MemProt,
        cache_policy: vmo::CachePolicy,
        options: u32,
    ) -> Result<u64> {
        // Check if VMAR is destroyed
        if self.flags.destroyed {
            return Err(RX_ERR_BAD_STATE);
        }

        // Check permissions
        let perm = vmar_options::prot_to_perm(prot);
        if !self.flags.can_map(perm) {
            return Err(RX_ERR_ACCESS_DENIED);
        }

        // Page-align size
        let aligned_size = (size + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);

        // Find or validate offset
        let offset = if options & vmar_options::SPECIFIC != 0 {
            // User specified a specific address
            vmar_offset
        } else {
            // Find a free region
            self.find_free_region(aligned_size, PAGE_SIZE as u64)
                .ok_or(RX_ERR_NO_RESOURCES)?
        };

        // Check for overlap
        self.check_overlap(offset, aligned_size)?;

        // Validate VMO offset
        if vmo_offset + size > vmo.size() {
            return Err(RX_ERR_INVALID_ARGS);
        }

        // Create mapping region
        let mapping = VmarRegion::Mapping {
            vmo,
            vmo_offset,
            size: aligned_size,
            prot,
            cache_policy,
        };

        // Insert into children
        let mut children = self.children.lock();
        children.insert(offset, mapping);

        Ok(offset)
    }

    /// Map a VMO into this VMAR and bind to an address space
    ///
    /// This method creates a mapping and performs the actual page table
    /// manipulation to map the VMO's pages into the specified address space.
    pub fn map_to_aspace(
        &self,
        aspace: &crate::kernel::vm::aspace::AddressSpace,
        vmo: Arc<Vmo>,
        vmar_offset: u64,
        vmo_offset: u64,
        size: u64,
        prot: MemProt,
        _cache_policy: vmo::CachePolicy,
        options: u32,
    ) -> Result<u64> {
        // Create the mapping in VMAR
        let offset = self.map(vmo.clone(), vmar_offset, vmo_offset, size, prot, vmo::CachePolicy::Default, options)?;

        // Calculate the virtual address to map at
        // The VMAR offset is relative to the VMAR's base
        let vaddr = (self.base + offset) as usize;

        // Actually map the pages into the address space
        let page_count = ((size + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64) as usize;

        // For each page in the mapping
        for i in 0..page_count {
            let page_offset = vmo_offset as usize + (i * PAGE_SIZE);
            let current_vaddr = vaddr + (i * PAGE_SIZE);

            // Get or allocate the physical page from VMO
            let paddr = match vmo.pages.get(page_offset / PAGE_SIZE) {
                Some(paddr) => paddr,
                None => {
                    // Allocate on demand - this is lazy allocation
                    // In a real implementation, we'd use page faults
                    log_debug!("VMAR: Lazy allocation for page offset {:#x}", page_offset);
                    continue;
                }
            };

            // Map into address space
            // Convert virtual address back to physical for mapping
            if let Err(err) = aspace.map(current_vaddr, paddr, 1, prot) {
                log_error!("VMAR: Failed to map page at {:#x}: {:?}", current_vaddr, err);
                // Clean up previously mapped pages
                for j in 0..i {
                    let prev_vaddr = vaddr + (j * PAGE_SIZE);
                    let _ = aspace.unmap(prev_vaddr, 1);
                }
                return Err(err);
            }
        }

        // Flush TLB for the mapped region
        aspace.flush_tlb();

        log_debug!(
            "VMAR: Mapped VMO at vaddr={:#x} pages={} prot={:?}",
            vaddr,
            page_count,
            prot
        );

        Ok(vaddr as u64)
    }

    /// Unmap a region from this VMAR
    pub fn unmap(&self, offset: u64, size: u64) -> Result {
        // Check if VMAR is destroyed
        if self.flags.destroyed {
            return Err(RX_ERR_BAD_STATE);
        }

        let mut children = self.children.lock();

        // Find overlapping mappings
        let start = offset;
        let end = offset + size;

        // Collect keys to remove
        let keys_to_remove: Vec<u64> = children
            .range(..=end)
            .filter(|(&base, region)| {
                let region_end = base + region.size();
                base < end && region_end > start
            })
            .map(|(&base, _)| base)
            .collect();

        // Remove mappings
        for key in keys_to_remove {
            children.remove(&key);
        }

        Ok(())
    }

    /// Unmap a region from this VMAR and address space
    pub fn unmap_from_aspace(
        &self,
        aspace: &crate::kernel::vm::aspace::AddressSpace,
        offset: u64,
        size: u64,
    ) -> Result {
        // Calculate the virtual address
        let vaddr = (self.base + offset) as usize;

        // Unmap from address space first
        let page_count = (size / PAGE_SIZE) as usize;
        if page_count > 0 {
            if let Err(err) = aspace.unmap(vaddr, page_count) {
                log_error!("VMAR: Failed to unmap pages: {:?}", err);
                return Err(err);
            }

            // Flush TLB for the unmapped region
            aspace.flush_tlb(vaddr, page_count * PAGE_SIZE);

            log_debug!("VMAR: Unmapped vaddr={:#x} pages={}", vaddr, page_count);
        }

        // Remove from VMAR
        self.unmap(offset, size)?;

        Ok(())
    }

    /// Change protection for a region
    pub fn protect(&self, offset: u64, size: u64, new_prot: MemProt) -> Result {
        // Check if VMAR is destroyed
        if self.flags.destroyed {
            return Err(RX_ERR_BAD_STATE);
        }

        // Check permissions
        let perm = vmar_options::prot_to_perm(new_prot);
        if !self.flags.can_map(perm) {
            return Err(RX_ERR_ACCESS_DENIED);
        }

        let mut children = self.children.lock();

        // Find the mapping at this offset
        if let Some(region) = children.get_mut(&offset) {
            match region {
                VmarRegion::Mapping { prot, .. } => {
                    *prot = new_prot;
                }
                VmarRegion::Vmar { .. } => {
                    return Err(RX_ERR_WRONG_TYPE);
                }
            }
        } else {
            return Err(RX_ERR_NOT_FOUND);
        }

        Ok(())
    }

    /// Change protection for a region in address space
    pub fn protect_in_aspace(
        &self,
        aspace: &crate::kernel::vm::aspace::AddressSpace,
        offset: u64,
        size: u64,
        new_prot: MemProt,
    ) -> Result {
        // Update protection in VMAR
        self.protect(offset, size, new_prot)?;

        // Calculate the virtual address
        let vaddr = (self.base + offset) as usize;

        // Update protection in address space
        let page_count = (size / PAGE_SIZE) as usize;
        if page_count > 0 {
            if let Err(err) = aspace.protect(vaddr, page_count, new_prot) {
                log_error!("VMAR: Failed to update protections: {:?}", err);
                return Err(err);
            }

            // Flush TLB for the region with changed protections
            aspace.flush_tlb(vaddr, page_count * PAGE_SIZE);

            log_debug!(
                "VMAR: Protected vaddr={:#x} pages={} prot={:?}",
                vaddr,
                page_count,
                new_prot
            );
        }

        Ok(())
    }
}

/// ============================================================================
/// VMAR Registry
/// ============================================================================

/// Maximum number of VMARs in the system
const MAX_VMARS: usize = 65536;

/// VMAR registry
struct VmarRegistry {
    /// VMAR entries
    entries: [Option<Arc<Vmar>>; MAX_VMARS],

    /// Next index to check
    next_index: AtomicUsize,

    /// Number of active VMARs
    count: AtomicUsize,
}

impl VmarRegistry {
    const fn new() -> Self {
        const INIT: Option<Arc<Vmar>> = None;
        Self {
            entries: [INIT; MAX_VMARS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    fn insert(&mut self, vmar: Arc<Vmar>) -> Result {
        let id = vmar.id();
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (id as usize) % MAX_VMARS;

        loop {
            if self.entries[idx].is_none() {
                self.entries[idx] = Some(vmar);
                self.count.fetch_add(1, Ordering::Relaxed);
                self.next_index.store((idx + 1) % MAX_VMARS, Ordering::Relaxed);
                return Ok(());
            }

            idx = (idx + 1) % MAX_VMARS;
            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    fn get(&self, id: VmarId) -> Option<Arc<Vmar>> {
        let idx = (id as usize) % MAX_VMARS;
        self.entries[idx].as_ref().filter(|v| v.id() == id).cloned()
    }

    fn remove(&mut self, id: VmarId) -> Result {
        let idx = (id as usize) % MAX_VMARS;
        if self.entries[idx].is_some() {
            self.entries[idx] = None;
            self.count.fetch_sub(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NOT_FOUND)
        }
    }

    fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global VMAR registry
static VMAR_REGISTRY: VmarRegistry = VmarRegistry::new();

/// ============================================================================
/// Root VMAR
/// ============================================================================

/// Root VMAR for user address space
static mut ROOT_USER_VMAR: Option<Arc<Vmar>> = None;

/// Initialize the root VMAR
pub fn init_root_user_vmar() {
    unsafe {
        // Create root VMAR covering the entire user address space
        #[cfg(target_arch = "aarch64")]
        let (base, size) = (0x0000_0000_1000usize, 0x0000_0100_0000_0000usize);

        #[cfg(target_arch = "x86_64")]
        let (base, size) = (0x0000_1000usize, 0x0000_8000_0000usize);

        #[cfg(target_arch = "riscv64")]
        let (base, size) = (0x0000_1000usize, 0x0000_8000_0000usize);

        ROOT_USER_VMAR = Some(Vmar::new_root(base, size));
    }
}

/// Get the root user VMAR
fn get_root_user_vmar() -> Option<&'static Arc<Vmar>> {
    unsafe { ROOT_USER_VMAR.as_ref() }
}

/// ============================================================================
/// Handle to VMAR Resolution
/// ============================================================================

/// Look up a VMAR from a handle value
fn lookup_vmar_from_handle(
    handle_val: u32,
    required_rights: Rights,
) -> Result<Arc<Vmar>> {
    // For handle value 0, return the root VMAR
    if handle_val == 0 {
        return get_root_user_vmar()
            .cloned()
            .ok_or(RX_ERR_NOT_SUPPORTED);
    }

    // TODO: Proper handle table lookup
    // For now, try to look up directly from VMAR registry
    let vmar_id = handle_val as VmarId;
    VMAR_REGISTRY
        .get(vmar_id)
        .ok_or(RX_ERR_INVALID_ARGS)
}

/// ============================================================================
/// Syscall: VMAR Allocate
/// ============================================================================

/// Allocate a new VMAR syscall handler
///
/// # Arguments
///
/// * `parent_handle` - Parent VMAR handle
/// * `options` - VMAR options (permissions, etc.)
/// * `offset` - Offset within parent (0 for any)
/// * `size` - Size of the VMAR
/// * `child_addr_out` - User pointer to store child address
///
/// # Returns
///
/// * On success: Child VMAR handle (encoded with address)
/// * On error: Negative error code
pub fn sys_vmar_allocate_impl(
    parent_handle: u32,
    options: u32,
    offset: u64,
    size: u64,
    child_addr_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_vmar_allocate: parent={:#x} options={:#x} offset={:#x} size={:#x}",
        parent_handle, options, offset, size
    );

    // Validate size
    if size == 0 {
        log_error!("sys_vmar_allocate: size must be non-zero");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Page-align size
    if size & 0xFFF != 0 {
        log_error!("sys_vmar_allocate: size must be page-aligned");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up parent VMAR
    let parent_vmar = match lookup_vmar_from_handle(parent_handle, Rights::DUPLICATE) {
        Ok(vmar) => vmar,
        Err(err) => {
            log_error!("sys_vmar_allocate: failed to lookup parent VMAR: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Calculate alignment from options
    let align_mask = 0xFFF; // Page-aligned by default

    // Determine offset
    let actual_offset = if options & vmar_options::SPECIFIC != 0 {
        offset
    } else {
        // Find a free region
        match parent_vmar.find_free_region(size, PAGE_SIZE as u64) {
            Some(off) => off,
            None => {
                log_error!("sys_vmar_allocate: no free region");
                return err_to_ret(RX_ERR_NO_RESOURCES);
            }
        }
    };

    // Create child VMAR
    let child_vmar = match Vmar::new_child(
        &parent_vmar,
        actual_offset,
        size,
        options,
        align_mask,
    ) {
        Ok(vmar) => vmar,
        Err(err) => {
            log_error!("sys_vmar_allocate: failed to create child VMAR: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Register the VMAR
    if let Err(err) = VMAR_REGISTRY.insert(child_vmar.clone()) {
        log_error!("sys_vmar_allocate: failed to register VMAR: {:?}", err);
        return err_to_ret(err);
    }

    // Write child address to user space
    if child_addr_out != 0 {
        let user_ptr = UserPtr::<u64>::new(child_addr_out);
        unsafe {
            if let Err(err) = copy_to_user(
                user_ptr,
                &actual_offset as *const u64 as *const u8,
                8
            ) {
                log_error!("sys_vmar_allocate: copy_to_user failed: {:?}", err);
                // Clean up VMAR
                VMAR_REGISTRY.remove(child_vmar.id());
                return err_to_ret(err.into());
            }
        }
    }

    // Return the VMAR ID as the handle
    let handle_value = child_vmar.id() as u32;

    log_debug!(
        "sys_vmar_allocate: success vmar={:#x} base={:#x}",
        handle_value,
        actual_offset
    );

    ok_to_ret(handle_value as usize)
}

/// ============================================================================
/// Syscall: VMAR Map
/// ============================================================================

/// Look up a VMO from a handle
fn lookup_vmo_from_handle(handle_val: u32, _required_rights: Rights) -> Option<Arc<Vmo>> {
    // TODO: Integrate with proper VMO handle table lookup
    // For now, return None to indicate not implemented
    // In a real implementation, this would:
    // 1. Look up the handle in the current process's handle table
    // 2. Validate it's a VMO handle with the required rights
    // 3. Return the VMO object
    None
}

/// Map a VMO into a VMAR syscall handler
///
/// # Arguments
///
/// * `vmar_handle` - VMAR handle
/// * `options` - Mapping options (permissions, etc.)
/// * `vmar_offset` - Offset within VMAR
/// * `vmo_handle` - VMO handle to map
/// * `vmo_offset` - Offset within VMO
/// * `len` - Length to map
/// * `mapped_addr_out` - User pointer to store mapped address
///
/// # Returns
///
/// * On success: Mapped address
/// * On error: Negative error code
pub fn sys_vmar_map_impl(
    vmar_handle: u32,
    options: u32,
    vmar_offset: u64,
    vmo_handle: u32,
    vmo_offset: u64,
    len: u64,
    mapped_addr_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_vmar_map: vmar={:#x} options={:#x} vmar_offset={:#x} vmo={:#x} vmo_offset={:#x} len={:#x}",
        vmar_handle, options, vmar_offset, vmo_handle, vmo_offset, len
    );

    // Validate length
    if len == 0 {
        log_error!("sys_vmar_map: len must be non-zero");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Page-align length
    if len & 0xFFF != 0 {
        log_error!("sys_vmar_map: len must be page-aligned");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate protection flags
    let perm_flags = options & vmar_options::PERM_FLAGS;
    if perm_flags & !vmar_options::PERM_FLAGS != 0 {
        log_error!("sys_vmar_map: invalid permission flags");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up VMAR
    let vmar = match lookup_vmar_from_handle(vmar_handle, Rights::WRITE) {
        Ok(v) => v,
        Err(err) => {
            log_error!("sys_vmar_map: failed to lookup VMAR: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Convert options to protection
    let prot = vmar_options::perm_to_prot(options);
    let cache_policy = vmo::CachePolicy::Default;

    // Look up VMO from handle
    let vmo = match lookup_vmo_from_handle(vmo_handle, Rights::READ) {
        Some(v) => v,
        None => {
            log_error!("sys_vmar_map: VMO lookup not implemented");
            return err_to_ret(RX_ERR_NOT_SUPPORTED);
        }
    };

    // Perform the mapping
    let mapped_addr = match vmar.map(
        vmo,
        vmar_offset,
        vmo_offset,
        len,
        prot,
        cache_policy,
        options,
    ) {
        Ok(addr) => addr,
        Err(err) => {
            log_error!("sys_vmar_map: map failed: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Write mapped address to user space
    if mapped_addr_out != 0 {
        let user_ptr = UserPtr::<u64>::new(mapped_addr_out);
        unsafe {
            if let Err(err) = copy_to_user(
                user_ptr,
                &mapped_addr as *const u64 as *const u8,
                8
            ) {
                log_error!("sys_vmar_map: copy_to_user failed: {:?}", err);
                // Unmap the region
                let _ = vmar.unmap(mapped_addr, len);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_vmar_map: success addr={:#x}", mapped_addr);

    ok_to_ret(mapped_addr as usize)
}

/// ============================================================================
/// Syscall: VMAR Unmap
/// ============================================================================

/// Unmap a region syscall handler
///
/// # Arguments
///
/// * `vmar_handle` - VMAR handle
/// * `addr` - Address to unmap
/// * `len` - Length to unmap
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_vmar_unmap_impl(vmar_handle: u32, addr: u64, len: u64) -> SyscallRet {
    log_debug!(
        "sys_vmar_unmap: vmar={:#x} addr={:#x} len={:#x}",
        vmar_handle, addr, len
    );

    // Validate length
    if len == 0 {
        log_error!("sys_vmar_unmap: len must be non-zero");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Page-align length and address
    if len & 0xFFF != 0 || addr & 0xFFF != 0 {
        log_error!("sys_vmar_unmap: addr and len must be page-aligned");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up VMAR
    let vmar = match lookup_vmar_from_handle(vmar_handle, Rights::WRITE) {
        Ok(v) => v,
        Err(err) => {
            log_error!("sys_vmar_unmap: failed to lookup VMAR: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Perform the unmap
    if let Err(err) = vmar.unmap(addr, len) {
        log_error!("sys_vmar_unmap: unmap failed: {:?}", err);
        return err_to_ret(err);
    }

    log_debug!("sys_vmar_unmap: success");

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: VMAR Protect
/// ============================================================================

/// Change memory protection syscall handler
///
/// # Arguments
///
/// * `vmar_handle` - VMAR handle
/// * `options` - New protection flags
/// * `addr` - Address to protect
/// * `len` - Length to protect
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_vmar_protect_impl(vmar_handle: u32, options: u32, addr: u64, len: u64) -> SyscallRet {
    log_debug!(
        "sys_vmar_protect: vmar={:#x} options={:#x} addr={:#x} len={:#x}",
        vmar_handle, options, addr, len
    );

    // Validate length
    if len == 0 {
        log_error!("sys_vmar_protect: len must be non-zero");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Page-align length and address
    if len & 0xFFF != 0 || addr & 0xFFF != 0 {
        log_error!("sys_vmar_protect: addr and len must be page-aligned");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate protection flags
    let perm_flags = options & vmar_options::PERM_FLAGS;
    if perm_flags & !vmar_options::PERM_FLAGS != 0 {
        log_error!("sys_vmar_protect: invalid permission flags");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Convert options to protection
    let prot = vmar_options::perm_to_prot(options);

    // Look up VMAR
    let vmar = match lookup_vmar_from_handle(vmar_handle, Rights::WRITE) {
        Ok(v) => v,
        Err(err) => {
            log_error!("sys_vmar_protect: failed to lookup VMAR: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Perform the protect operation
    if let Err(err) = vmar.protect(addr, len, prot) {
        log_error!("sys_vmar_protect: protect failed: {:?}", err);
        return err_to_ret(err);
    }

    log_debug!("sys_vmar_protect: success");

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: VMAR Destroy
/// ============================================================================

/// Destroy a VMAR and all children syscall handler
///
/// # Arguments
///
/// * `vmar_handle` - VMAR handle to destroy
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_vmar_destroy_impl(vmar_handle: u32) -> SyscallRet {
    log_debug!("sys_vmar_destroy: vmar={:#x}", vmar_handle);

    // Look up VMAR
    let vmar = match lookup_vmar_from_handle(vmar_handle, Rights::DUPLICATE) {
        Ok(v) => v,
        Err(err) => {
            log_error!("sys_vmar_destroy: failed to lookup VMAR: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Perform the destroy
    if let Err(err) = vmar.destroy() {
        log_error!("sys_vmar_destroy: destroy failed: {:?}", err);
        return err_to_ret(err);
    }

    // Remove from registry
    let _ = VMAR_REGISTRY.remove(vmar.id());

    log_debug!("sys_vmar_destroy: success");

    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get VMAR subsystem statistics
pub fn get_stats() -> VmarStats {
    VmarStats {
        total_vmars: VMAR_REGISTRY.count(),
        total_mappings: 0, // TODO: Track mappings across all VMARs
        mapped_bytes: 0,    // TODO: Track mapped bytes
    }
}

/// VMAR subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmarStats {
    /// Total number of VMARs
    pub total_vmars: usize,

    /// Total number of mappings
    pub total_mappings: usize,

    /// Total bytes mapped
    pub mapped_bytes: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the VMAR syscall subsystem
pub fn init() {
    log_info!("VMAR syscall subsystem initialized");
    log_info!("  Max VMARs: {}", MAX_VMARS);

    // Initialize root user VMAR
    init_root_user_vmar();
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vmar_options() {
        // Test permission flags
        assert_eq!(vmar_options::PERM_READ, 0x01);
        assert_eq!(vmar_options::PERM_WRITE, 0x02);
        assert_eq!(vmar_options::PERM_EXECUTE, 0x04);

        // Test capability flags
        assert_eq!(vmar_options::CAN_MAP_READ, 0x08);
        assert_eq!(vmar_options::CAN_MAP_WRITE, 0x10);
        assert_eq!(vmar_options::CAN_MAP_EXECUTE, 0x20);
    }

    #[test]
    fn test_perm_prot_conversion() {
        // Test perm to prot conversion
        let prot = vmar_options::perm_to_prot(
            vmar_options::PERM_READ | vmar_options::PERM_WRITE
        );
        assert!(prot.can_read());
        assert!(prot.can_write());
        assert!(!prot.can_execute());

        // Test prot to perm conversion
        let perm = vmar_options::prot_to_perm(MemProt::Read | MemProt::Execute);
        assert_eq!(perm & vmar_options::PERM_READ, vmar_options::PERM_READ);
        assert_eq!(perm & vmar_options::PERM_EXECUTE, vmar_options::PERM_EXECUTE);
        assert_eq!(perm & vmar_options::PERM_WRITE, 0);
    }

    #[test]
    fn test_vmar_root_creation() {
        let root = Vmar::new_root(0x1000, 0x10000);
        assert_eq!(root.base, 0x1000);
        assert_eq!(root.size, 0x10000);
        assert!(root.flags.is_root);
        assert!(!root.flags.destroyed);
    }

    #[test]
    fn test_vmar_child_creation() {
        let root = Vmar::new_root(0x1000, 0x100000);
        let child = Vmar::new_child(&root, 0x10000, 0x1000, 0, 0xFFF);

        assert!(child.is_ok());
        let child = child.unwrap();
        assert_eq!(child.base, 0x10000);
        assert_eq!(child.size, 0x1000);
        assert!(!child.flags.is_root);
    }

    #[test]
    fn test_vmar_find_free_region() {
        let root = Vmar::new_root(0x1000, 0x100000);

        // Initially, entire space is free
        let addr = root.find_free_region(0x1000, 0x1000);
        assert_eq!(addr, Some(0));

        // After creating a child at 0, next free region should be after it
        let _child = Vmar::new_child(&root, 0, 0x1000, 0, 0xFFF).unwrap();
        let addr = root.find_free_region(0x1000, 0x1000);
        assert_eq!(addr, Some(0x1000));
    }

    #[test]
    fn test_vmar_allocate_validation() {
        // Invalid: zero size
        assert!(sys_vmar_allocate_impl(0, 0, 0, 0, 0) < 0);

        // Invalid: non-page-aligned size
        assert!(sys_vmar_allocate_impl(0, 0, 0, 0x1001, 0) < 0);
    }

    #[test]
    fn test_vmar_map_validation() {
        // Invalid: zero length
        assert!(sys_vmar_map_impl(0, 0, 0, 0, 0, 0, 0) < 0);

        // Invalid: non-page-aligned length
        assert!(sys_vmar_map_impl(0, 0, 0, 0, 0, 0x1001, 0) < 0);
    }

    #[test]
    fn test_vmar_unmap_validation() {
        // Invalid: zero length
        assert!(sys_vmar_unmap_impl(0, 0x1000, 0) < 0);

        // Invalid: non-page-aligned address
        assert!(sys_vmar_unmap_impl(0, 0x1001, 0x1000) < 0);
    }

    #[test]
    fn test_vmar_protect_validation() {
        // Invalid: zero length
        assert!(sys_vmar_protect_impl(0, 0, 0x1000, 0) < 0);

        // Invalid: non-page-aligned address
        assert!(sys_vmar_protect_impl(0, 0, 0x1001, 0x1000) < 0);
    }
}
