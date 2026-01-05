// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86_64 address space management
//!
//! This module provides implementations of virtual memory address spaces
//! for x86_64, including both normal MMU page tables and Extended Page Tables (EPT)
//! used for virtualization.

use crate::kernel::arch::amd64::ioport::IoBitmap;
use crate::kernel::arch::amd64::mmu::*;
use crate::kernel::arch::amd64::page_tables::*;
use crate::kernel::arch::amd64::page_tables::{X86PageTableBase, PageTableLevel, PtFlags, IntermediatePtFlags, PendingTlbInvalidation};
use crate::kernel::arch::amd64::page_tables::mmu_flags;
use crate::fbl::atomic::AtomicInt;
use crate::fbl::canary::Canary;
use crate::kernel::vm::arch_vm_aspace::ArchVmAspaceInterface;
use crate::rustux::types::*;
use crate::rustux::types::status;
use core::cmp::max;
use core::mem::{size_of, align_of, MaybeUninit};
use core::sync::atomic::Ordering;

/// Magic number for X86ArchVmAspace canary
const VAAS_MAGIC: u32 = 0x56414153; // "VAAS"

/// Implementation of page tables used by x86-64 CPUs
pub struct X86PageTableMmu {
    /// Base page table implementation
    base: X86PageTableBase,
    /// If true, all mappings will have the global bit set
    use_global_mappings: bool,
}

impl X86PageTableMmu {
    /// Create a new MMU page table instance
    pub fn new() -> Self {
        Self {
            base: X86PageTableBase::new(),
            use_global_mappings: false,
        }
    }

    /// Initialize the page table with the given context
    pub fn init(&mut self, ctx: *mut core::ffi::c_void) -> RxStatus {
        self.base.init(ctx)
    }

    /// Destroy the page table
    pub fn destroy(&mut self) -> RxStatus {
        self.base.destroy()
    }

    /// Initialize the kernel page table, assigning the given context to it.
    ///
    /// This X86PageTable will be special in that its mappings will all have
    /// the G (global) bit set, and are expected to be aliased across all page
    /// tables used in the normal MMU. See `alias_kernel_mappings`.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Context pointer for the page table
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn init_kernel(&mut self, ctx: *mut core::ffi::c_void) -> RxStatus {
        self.use_global_mappings = true;
        self.base.init(ctx)
    }

    /// Used for normal MMU page tables so they can share the high kernel mapping
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn alias_kernel_mappings(&mut self) -> RxStatus {
        unsafe { sys_x86_page_table_mmu_alias_kernel_mappings(&mut self.base) }
    }

    /// Get the top level of the page table hierarchy
    fn top_level(&self) -> PageTableLevel {
        PageTableLevel::PML4_L
    }

    /// Check if the given flags are allowed
    fn allowed_flags(&self, flags: u32) -> bool {
        (flags & mmu_flags::ARCH_MMU_FLAG_PERM_READ) != 0
    }

    /// Check if the physical address is valid
    fn check_paddr(&self, paddr: PAddr) -> bool {
        unsafe { sys_x86_mmu_check_paddr(paddr) }
    }

    /// Check if the virtual address is valid
    fn check_vaddr(&self, vaddr: VAddr) -> bool {
        unsafe { sys_x86_is_vaddr_canonical(vaddr) }
    }

    /// Check if the page size is supported at the given level
    fn supports_page_size(&self, level: PageTableLevel) -> bool {
        unsafe { sys_x86_page_table_mmu_supports_page_size(level) }
    }

    /// Get flags for intermediate page table entries
    fn intermediate_flags(&self) -> IntermediatePtFlags {
        unsafe { sys_x86_page_table_mmu_intermediate_flags() }
    }

    /// Get flags for terminal page table entries
    fn terminal_flags(&self, level: PageTableLevel, flags: u32) -> PtFlags {
        unsafe { sys_x86_page_table_mmu_terminal_flags(level, flags) }
    }

    /// Get flags for a split page table entry
    fn split_flags(&self, level: PageTableLevel, flags: PtFlags) -> PtFlags {
        unsafe { sys_x86_page_table_mmu_split_flags(level, flags) }
    }

    /// Invalidate TLB entries
    fn tlb_invalidate(&self, pending: &mut PendingTlbInvalidation) {
        unsafe { sys_x86_page_table_mmu_tlb_invalidate(&self.base, pending) }
    }

    /// Convert page table flags to MMU flags
    fn pt_flags_to_mmu_flags(&self, flags: PtFlags, level: PageTableLevel) -> u32 {
        unsafe { sys_x86_page_table_mmu_pt_flags_to_mmu_flags(flags, level) }
    }

    /// Check if cache flushes are needed
    fn needs_cache_flushes(&self) -> bool {
        false
    }
}

/// Implementation of Intel's Extended Page Tables, for use in virtualization
pub struct X86PageTableEpt {
    /// Base page table implementation
    base: X86PageTableBase,
}

impl X86PageTableEpt {
    /// Create a new EPT page table instance
    pub fn new() -> Self {
        Self {
            base: X86PageTableBase::new(),
        }
    }

    /// Initialize the page table with the given context
    pub fn init(&mut self, ctx: *mut core::ffi::c_void) -> RxStatus {
        self.base.init(ctx)
    }

    /// Destroy the page table
    pub fn destroy(&mut self) -> RxStatus {
        self.base.destroy()
    }

    /// Get the top level of the page table hierarchy
    fn top_level(&self) -> PageTableLevel {
        PageTableLevel::PML4_L
    }

    /// Check if the given flags are allowed
    fn allowed_flags(&self, flags: u32) -> bool {
        unsafe { sys_x86_page_table_ept_allowed_flags(flags) }
    }

    /// Check if the physical address is valid
    fn check_paddr(&self, paddr: PAddr) -> bool {
        unsafe { sys_x86_page_table_ept_check_paddr(paddr) }
    }

    /// Check if the virtual address is valid
    fn check_vaddr(&self, vaddr: VAddr) -> bool {
        unsafe { sys_x86_page_table_ept_check_vaddr(vaddr) }
    }

    /// Check if the page size is supported at the given level
    fn supports_page_size(&self, level: PageTableLevel) -> bool {
        unsafe { sys_x86_page_table_ept_supports_page_size(level) }
    }

    /// Get flags for intermediate page table entries
    fn intermediate_flags(&self) -> IntermediatePtFlags {
        unsafe { sys_x86_page_table_ept_intermediate_flags() }
    }

    /// Get flags for terminal page table entries
    fn terminal_flags(&self, level: PageTableLevel, flags: u32) -> PtFlags {
        unsafe { sys_x86_page_table_ept_terminal_flags(level, flags) }
    }

    /// Get flags for a split page table entry
    fn split_flags(&self, level: PageTableLevel, flags: PtFlags) -> PtFlags {
        unsafe { sys_x86_page_table_ept_split_flags(level, flags) }
    }

    /// Invalidate TLB entries
    fn tlb_invalidate(&self, pending: &mut PendingTlbInvalidation) {
        unsafe { sys_x86_page_table_ept_tlb_invalidate(&self.base, pending) }
    }

    /// Convert page table flags to MMU flags
    fn pt_flags_to_mmu_flags(&self, flags: PtFlags, level: PageTableLevel) -> u32 {
        unsafe { sys_x86_page_table_ept_pt_flags_to_mmu_flags(flags, level) }
    }

    /// Check if cache flushes are needed
    fn needs_cache_flushes(&self) -> bool {
        false
    }
}

/// x86 architecture-specific virtual memory address space
pub struct X86ArchVmAspace {
    /// Canary for detecting use-after-free
    canary: Canary,
    /// I/O port bitmap for this address space
    io_bitmap: IoBitmap,
    /// Storage for either an MMU page table or an EPT
    page_table_storage: PageTableStorage,
    /// Pointer to the page table (either normal or EPT)
    pt: *mut X86PageTableBase,
    /// Flags for this address space
    flags: u32,
    /// Base address of the address space
    base: VAddr,
    /// Size of the address space
    size: usize,
    /// CPUs that are currently executing in this address space
    active_cpus: AtomicInt,
}

/// Storage for either an MMU page table or an EPT
#[repr(C, align(16))]  // Assuming 16 is enough for alignment; adjust as needed
struct PageTableStorage {
    /// Raw storage bytes
    storage: [u8; PageTableStorage::SIZE],
}

impl PageTableStorage {
    /// Size of the storage needed for a page table
    const SIZE: usize = {
        const MMU_SIZE: usize = size_of::<X86PageTableMmu>();
        const EPT_SIZE: usize = size_of::<X86PageTableEpt>();
        if MMU_SIZE > EPT_SIZE { MMU_SIZE } else { EPT_SIZE }
    };

    /// Alignment of the storage needed for a page table
    const ALIGN: usize = {
        const MMU_ALIGN: usize = align_of::<X86PageTableMmu>();
        const EPT_ALIGN: usize = align_of::<X86PageTableEpt>();
        if MMU_ALIGN > EPT_ALIGN { MMU_ALIGN } else { EPT_ALIGN }
    };
    
    /// Create new uninitialized storage
    pub fn new() -> Self {
        Self {
            storage: [0; Self::SIZE],
        }
    }
}

impl X86ArchVmAspace {
    /// Create a new x86 address space
    pub fn new() -> Self {
        Self {
            canary: Canary::with_magic(VAAS_MAGIC),
            io_bitmap: IoBitmap::new(),
            page_table_storage: PageTableStorage::new(),
            pt: core::ptr::null_mut(),
            flags: 0,
            base: 0,
            size: 0,
            active_cpus: AtomicInt::new(0),
        }
    }

    /// Initialize the address space with the given parameters
    ///
    /// # Arguments
    ///
    /// * `base` - Base address of the address space
    /// * `size` - Size of the address space in bytes
    /// * `mmu_flags` - MMU flags for the address space
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn init(&mut self, base: VAddr, size: usize, flags: u32) -> RxStatus {
        self.base = base;
        self.size = size;
        self.flags = flags;

        // Initialize either an MMU page table or an EPT based on flags
        if (flags & mmu_flags::ARCH_ASPACE_FLAG_GUEST) != 0 {
            // Guest mode - use EPT
            let ept = self.page_table_storage.storage.as_mut_ptr() as *mut X86PageTableEpt;
            unsafe {
                ept.write(X86PageTableEpt::new());
                self.pt = ept as *mut X86PageTableBase;
                
                // Initialize the EPT
                let status = (*ept).init(self as *mut _ as *mut core::ffi::c_void);
                if status != status::OK {
                    return status;
                }
            }
        } else {
            // Normal mode - use MMU page table
            let mmu = self.page_table_storage.storage.as_mut_ptr() as *mut X86PageTableMmu;
            unsafe {
                mmu.write(X86PageTableMmu::new());
                self.pt = mmu as *mut X86PageTableBase;
                
                // Initialize the MMU page table
                let status = (*mmu).init(self as *mut _ as *mut core::ffi::c_void);
                if status != status::OK {
                    return status;
                }
                
                // Alias kernel mappings for normal address spaces
                let status = (*mmu).alias_kernel_mappings();
                if status != status::OK {
                    (*mmu).destroy();
                    return status;
                }
            }
        }
        
        status::OK
    }

    /// Destroy the address space
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn destroy(&mut self) -> RxStatus {
        if self.pt.is_null() {
            return status::ERR_BAD_STATE;
        }
        
        unsafe {
            // Call destroy on the page table
            let status = (*self.pt).destroy();
            if status != status::OK {
                return status;
            }
            
            // Clear the pointer
            self.pt = core::ptr::null_mut();
        }
        
        status::OK
    }

    /// Map a contiguous range of physical pages into the address space
    ///
    /// # Arguments
    ///
    /// * `vaddr` - Starting virtual address to map
    /// * `paddr` - Starting physical address to map
    /// * `count` - Number of pages to map
    /// * `mmu_flags` - MMU flags for the mapping
    /// * `mapped` - Optional output parameter to receive the number of pages mapped
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn map_contiguous(&mut self, vaddr: VAddr, paddr: PAddr, count: usize, 
                          mmu_flags: u32, mapped: Option<&mut usize>) -> RxStatus {
        unsafe { 
            sys_x86_arch_vm_aspace_map_contiguous(self, vaddr, paddr, count, mmu_flags, 
                                                 mapped.map_or(core::ptr::null_mut(), |m| m as *mut _)) 
        }
    }

    /// Map a range of physical pages into the address space
    ///
    /// # Arguments
    ///
    /// * `vaddr` - Starting virtual address to map
    /// * `phys` - Array of physical addresses to map
    /// * `count` - Number of pages to map
    /// * `mmu_flags` - MMU flags for the mapping
    /// * `mapped` - Optional output parameter to receive the number of pages mapped
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn map(&mut self, vaddr: VAddr, phys: &[PAddr], count: usize, 
              mmu_flags: u32, mapped: Option<&mut usize>) -> RxStatus {
        unsafe { 
            sys_x86_arch_vm_aspace_map(self, vaddr, phys.as_ptr(), count, mmu_flags, 
                                      mapped.map_or(core::ptr::null_mut(), |m| m as *mut _)) 
        }
    }

    /// Unmap a range of virtual addresses from the address space
    ///
    /// # Arguments
    ///
    /// * `vaddr` - Starting virtual address to unmap
    /// * `count` - Number of pages to unmap
    /// * `unmapped` - Optional output parameter to receive the number of pages unmapped
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn unmap(&mut self, vaddr: VAddr, count: usize, 
                unmapped: Option<&mut usize>) -> RxStatus {
        unsafe { 
            sys_x86_arch_vm_aspace_unmap(self, vaddr, count, 
                                       unmapped.map_or(core::ptr::null_mut(), |u| u as *mut _)) 
        }
    }

    /// Change the protection flags of a range of virtual addresses
    ///
    /// # Arguments
    ///
    /// * `vaddr` - Starting virtual address to protect
    /// * `count` - Number of pages to protect
    /// * `mmu_flags` - New MMU flags for the range
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn protect(&mut self, vaddr: VAddr, count: usize, mmu_flags: u32) -> RxStatus {
        unsafe { sys_x86_arch_vm_aspace_protect(self, vaddr, count, mmu_flags) }
    }

    /// Query the attributes of a virtual address
    ///
    /// # Arguments
    ///
    /// * `vaddr` - Virtual address to query
    /// * `paddr` - Optional output parameter to receive the physical address
    /// * `mmu_flags` - Optional output parameter to receive the MMU flags
    ///
    /// # Returns
    ///
    /// A status code indicating success or the type of failure
    pub fn query(&mut self, vaddr: VAddr, paddr: Option<&mut PAddr>, 
                mmu_flags: Option<&mut u32>) -> RxStatus {
        unsafe { 
            sys_x86_arch_vm_aspace_query(self, vaddr, 
                                       paddr.map_or(core::ptr::null_mut(), |p| p as *mut _),
                                       mmu_flags.map_or(core::ptr::null_mut(), |m| m as *mut _)) 
        }
    }

    /// Find a suitable location for a memory mapping
    ///
    /// # Arguments
    ///
    /// * `base` - Base address to start searching from
    /// * `prev_region_mmu_flags` - MMU flags of the previous region
    /// * `end` - End address to stop searching at
    /// * `next_region_mmu_flags` - MMU flags of the next region
    /// * `align` - Required alignment of the mapping
    /// * `size` - Size of the mapping in bytes
    /// * `mmu_flags` - MMU flags for the mapping
    ///
    /// # Returns
    ///
    /// The virtual address selected for the mapping
    pub fn pick_spot(&self, base: VAddr, prev_region_mmu_flags: u32,
                    end: VAddr, next_region_mmu_flags: u32,
                    align: VAddr, size: usize, mmu_flags: u32) -> VAddr {
        unsafe { 
            sys_x86_arch_vm_aspace_pick_spot(self, base, prev_region_mmu_flags,
                                          end, next_region_mmu_flags,
                                          align, size, mmu_flags) 
        }
    }

    /// Get the physical address of the page table root
    pub fn arch_table_phys(&self) -> PAddr {
        if self.pt.is_null() {
            0
        } else {
            unsafe { (*self.pt).phys() }
        }
    }

    /// Get the physical address of the page table root
    pub fn pt_phys(&self) -> PAddr {
        self.arch_table_phys()
    }

    /// Get the number of pages used by the page table
    pub fn pt_pages(&self) -> usize {
        if self.pt.is_null() {
            0
        } else {
            unsafe { (*self.pt).pages() }
        }
    }

    /// Get the active CPUs mask
    pub fn active_cpus(&self) -> i32 {
        self.active_cpus.load(Ordering::Acquire)
    }

    /// Get a reference to the IO bitmap
    pub fn io_bitmap(&mut self) -> &mut IoBitmap {
        &mut self.io_bitmap
    }

    /// Test if the virtual address is valid for this address space
    fn is_valid_vaddr(&self, vaddr: VAddr) -> bool {
        vaddr >= self.base && vaddr <= self.base + self.size - 1
    }

    /// Switch from one address space to another
    ///
    /// # Arguments
    ///
    /// * `from` - Address space to switch from (may be NULL)
    /// * `to` - Address space to switch to
    pub fn context_switch(from: Option<&mut Self>, to: &mut Self) {
        unsafe {
            match from {
                Some(from_aspace) => sys_x86_arch_vm_aspace_context_switch(from_aspace, to),
                None => sys_x86_arch_vm_aspace_context_switch(core::ptr::null_mut(), to),
            }
        }
    }
}

// FFI declarations for the system implementations
extern "C" {
    // X86PageTableMmu system functions
    fn sys_x86_page_table_mmu_alias_kernel_mappings(pt: *mut X86PageTableBase) -> RxStatus;
    fn sys_x86_page_table_mmu_supports_page_size(level: PageTableLevel) -> bool;
    fn sys_x86_page_table_mmu_intermediate_flags() -> IntermediatePtFlags;
    fn sys_x86_page_table_mmu_terminal_flags(level: PageTableLevel, flags: u32) -> PtFlags;
    fn sys_x86_page_table_mmu_split_flags(level: PageTableLevel, flags: PtFlags) -> PtFlags;
    fn sys_x86_page_table_mmu_tlb_invalidate(pt: *const X86PageTableBase, pending: *mut PendingTlbInvalidation);
    fn sys_x86_page_table_mmu_pt_flags_to_mmu_flags(flags: PtFlags, level: PageTableLevel) -> u32;

    // X86PageTableEpt system functions
    fn sys_x86_page_table_ept_allowed_flags(flags: u32) -> bool;
    fn sys_x86_page_table_ept_check_paddr(paddr: PAddr) -> bool;
    fn sys_x86_page_table_ept_check_vaddr(vaddr: VAddr) -> bool;
    fn sys_x86_page_table_ept_supports_page_size(level: PageTableLevel) -> bool;
    fn sys_x86_page_table_ept_intermediate_flags() -> IntermediatePtFlags;
    fn sys_x86_page_table_ept_terminal_flags(level: PageTableLevel, flags: u32) -> PtFlags;
    fn sys_x86_page_table_ept_split_flags(level: PageTableLevel, flags: PtFlags) -> PtFlags;
    fn sys_x86_page_table_ept_tlb_invalidate(pt: *const X86PageTableBase, pending: *mut PendingTlbInvalidation);
    fn sys_x86_page_table_ept_pt_flags_to_mmu_flags(flags: PtFlags, level: PageTableLevel) -> u32;

    // X86ArchVmAspace system functions
    fn sys_x86_arch_vm_aspace_map_contiguous(
        aspace: *mut X86ArchVmAspace,
        vaddr: VAddr,
        paddr: PAddr,
        count: usize,
        mmu_flags: u32,
        mapped: *mut usize,
    ) -> RxStatus;

    fn sys_x86_arch_vm_aspace_map(
        aspace: *mut X86ArchVmAspace,
        vaddr: VAddr,
        phys: *const PAddr,
        count: usize,
        mmu_flags: u32,
        mapped: *mut usize,
    ) -> RxStatus;

    fn sys_x86_arch_vm_aspace_unmap(
        aspace: *mut X86ArchVmAspace,
        vaddr: VAddr,
        count: usize,
        unmapped: *mut usize,
    ) -> RxStatus;

    fn sys_x86_arch_vm_aspace_protect(
        aspace: *mut X86ArchVmAspace,
        vaddr: VAddr,
        count: usize,
        mmu_flags: u32,
    ) -> RxStatus;

    fn sys_x86_arch_vm_aspace_query(
        aspace: *mut X86ArchVmAspace,
        vaddr: VAddr,
        paddr: *mut PAddr,
        mmu_flags: *mut u32,
    ) -> RxStatus;

    fn sys_x86_arch_vm_aspace_pick_spot(
        aspace: *const X86ArchVmAspace,
        base: VAddr,
        prev_region_mmu_flags: u32,
        end: VAddr,
        next_region_mmu_flags: u32,
        align: VAddr,
        size: usize,
        mmu_flags: u32,
    ) -> VAddr;

    fn sys_x86_arch_vm_aspace_context_switch(
        from: *mut X86ArchVmAspace,
        to: *mut X86ArchVmAspace,
    );

    // General helper functions
    fn sys_x86_is_vaddr_canonical(vaddr: VAddr) -> bool;
    fn sys_x86_mmu_check_paddr(paddr: PAddr) -> bool;
}

// Type alias for architecture-specific VM address space
pub type ArchVmAspace = X86ArchVmAspace;