// Copyright 2025 Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use core::ptr;
use core::ffi::{c_void, c_char};

use crate::arch::arm64::el2_state::*;
use crate::arch::arm64::{arm64_el2_tlbi_vmid, arm64_el2_tlbi_ipa, arm64_zva_size};
use crate::arch::arm64::include::arch::arch_ops::{arch_interrupt_save, arch_interrupt_restore};
// Don't use wildcard imports - they cause issues
// use crate::arch::arm64::mmu::*;
use crate::arch::aspace::*;
// Don't use wildcard import from arch::mmu
// use crate::arch::mmu::*;
use crate::bitmap::raw_bitmap::*;
use crate::bitmap::storage::*;
use crate::bits::*;
use crate::debug::*;
use crate::err::*;
use crate::fbl::atomic::*;
use crate::fbl::auto_call::*;
use crate::fbl::auto_lock::*;
use crate::kernel::mutex::*;
use crate::lib::heap::*;
use crate::lib::ktrace::*;
use crate::rand::*;
// arch_zero_page is defined locally in this module
// use crate::vm::arch_zero_page;
use crate::vm::aspace;
use crate::vm::physmap::*;
use crate::vm::pmm::*;
use crate::vm::vm::*;
use crate::vm::{vaddr_to_paddr, phys_to_virt, is_user_address};
use crate::vm::page_table::PageTableFlags;
// Import stub types from hypervisor module
use crate::arch::arm64::include::arch::hypervisor::{RawBitmapGeneric, FixedStorage};

// Import asm macro from core::arch
use core::arch::asm;

/// Early initialization for ARM64 address space
impl ArmArchVmAspace {
    pub fn new() -> Self {
        ArmArchVmAspace {
            lock: Mutex::new(()),
            canary: Canary::new(),
            flags: 0,
            base: 0,
            size: 0,
            tt_virt: ptr::null_mut(),
            tt_phys: 0,
            asid: 0,
            pt_pages: 0,
        }
    }

    pub fn init(&mut self) -> rx_status_t {
        self.canary.assert();
        ltrace!("aspace {:p}\n", self);

        let _guard = self.lock.lock();

        // Allocate a page for the page table
        self.tt_phys = 0; // Will be set by the caller
        self.tt_virt = crate::vm::phys_to_virt(self.tt_phys) as *mut pte_t;

        unsafe {
            arch_zero_page(self.tt_virt as *mut c_void);
        }
        self.pt_pages = 1;

        ltrace!("tt_phys {:#x} tt_virt {:p}\n", self.tt_phys, self.tt_virt);

        RX_OK
    }

    pub fn destroy(&mut self) -> rx_status_t {
        self.canary.assert();
        ltrace!("aspace {:p}\n", self);

        let _guard = self.lock.lock();

        debug_assert!((self.flags & ARCH_ASPACE_FLAG_KERNEL) == 0);

        // XXX make sure it's not mapped

        // pmm_free_page expects a physical address, not a page pointer
        pmm_free_page(self.tt_phys);

        if self.flags & ARCH_ASPACE_FLAG_GUEST != 0 {
            let vttbr = arm64_vttbr(self.asid, self.tt_phys);
            let status = arm64_el2_tlbi_vmid(vttbr);
            debug_assert!(status == RX_OK);
        } else {
            ARM64_TLBI!(ASIDE1IS, self.asid);
            unsafe { get_asid_allocator().free(self.asid); }
            self.asid = MMU_ARM64_UNUSED_ASID;
        }

        RX_OK
    }

    pub fn is_valid_vaddr(&self, vaddr: vaddr_t) -> bool {
        // Check if the address is within the valid range for this address space
        if self.flags & ARCH_ASPACE_FLAG_KERNEL != 0 {
            // Kernel address space has a specific range
            vaddr >= self.base && vaddr < ((self.base as u64) + (self.size as u64)) as vaddr_t
        } else if self.flags & ARCH_ASPACE_FLAG_GUEST != 0 {
            // Guest address space validation
            vaddr < (1u64 << MMU_GUEST_SIZE_SHIFT) as vaddr_t
        } else {
            // User address space validation
            vaddr < (1u64 << MMU_USER_SIZE_SHIFT) as vaddr_t
        }
    }

    pub fn pick_spot(&self, base: vaddr_t, _prev_region_mmu_flags: u32,
                     _end: vaddr_t, _next_region_mmu_flags: u32,
                     _align: vaddr_t, _size: size_t, _mmu_flags: u32) -> vaddr_t {
        self.canary.assert();
        page_align(base as u64) as vaddr_t
    }
}

pub fn context_switch(old_aspace: Option<&ArmArchVmAspace>, aspace: Option<&ArmArchVmAspace>) {
    if TRACE_CONTEXT_SWITCH {
        trace!("aspace {:?}\n", aspace);
    }

    let mut tcr: u64;
    let mut ttbr: u64;

    if let Some(aspace_ref) = aspace {
        aspace_ref.canary.assert();
        debug_assert!((aspace_ref.flags & (ARCH_ASPACE_FLAG_KERNEL | ARCH_ASPACE_FLAG_GUEST)) == 0);

        tcr = MMU_TCR_FLAGS_USER;
        ttbr = ((aspace_ref.asid as u64) << 48) | aspace_ref.tt_phys;
        unsafe {
            __arm_wsr64(b"ttbr0_el1\0".as_ptr(), ttbr);
            __isb(ARM_MB_SY);
        }

        if TRACE_CONTEXT_SWITCH {
            trace!("ttbr {:#x}, tcr {:#x}\n", ttbr, tcr);
        }
    } else {
        tcr = MMU_TCR_FLAGS_KERNEL;

        if TRACE_CONTEXT_SWITCH {
            trace!("tcr {:#x}\n", tcr);
        }
    }

    unsafe {
        __arm_wsr64(b"tcr_el1\0".as_ptr(), tcr);
        __isb(ARM_MB_SY);
    }
}

// Helper functions
fn is_page_aligned(addr: u64) -> bool {
    (addr & (PAGE_SIZE - 1)) == 0
}

fn page_align(addr: u64) -> u64 {
    (addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

// Assembly functions
extern "C" {
    fn __arm_wsr64(reg: *const c_char, val: u64);
    fn __arm_rsr64(reg: *const c_char) -> u64;
    fn __isb(mb_type: u32);
    fn __dsb(mb_type: u32);
    fn __dmb(mb_type: u32);
}

// TLB operations
macro_rules! ARM64_TLBI {
    ($op:ident, $val:expr) => {
        unsafe {
            asm!(concat!("tlbi ", stringify!($op), ", {}"), in(reg) $val, options(preserves_flags, nostack));
        }
    };
}
use crate::rustux::types::*;

const LOCAL_TRACE: bool = false;
const TRACE_CONTEXT_SWITCH: bool = false;

// ktraces just local to this file
const LOCAL_KTRACE: bool = false;

macro_rules! local_ktrace0 {
    ($probe:expr) => {
        if LOCAL_KTRACE {
            crate::kernel::lib::ktrace::ktrace_probe0($probe);
        }
    };
}

macro_rules! local_ktrace2 {
    ($probe:expr, $x:expr, $y:expr) => {
        if LOCAL_KTRACE {
            crate::kernel::lib::ktrace::ktrace_probe2($probe, $x, $y);
        }
    };
}

macro_rules! local_ktrace64 {
    ($probe:expr, $x:expr) => {
        if LOCAL_KTRACE {
            crate::kernel::lib::ktrace::ktrace_probe64($probe, $x);
        }
    };
}

// Static assertions
// Note: These original assertions need verification for ARM64 address layout
// The static_assert macro causes overflow on false conditions, so we use build.rs checks instead
const _: () = assert!(MMU_KERNEL_SIZE_SHIFT <= 48);
const _: () = assert!(MMU_KERNEL_SIZE_SHIFT >= 25);
// TODO: Verify KERNEL_BASE and KERNEL_ASPACE_BASE alignment requirements

// Static relocated base to prepare for KASLR. Used at early boot and by gdb
// script to know the target relocated address.
// TODO(SEC-31): Choose it randomly.
#[cfg(DISABLE_KASLR)]
static mut kernel_relocated_base: u64 = KERNEL_BASE;
#[cfg(not(DISABLE_KASLR))]
static mut kernel_relocated_base: u64 = 0xffffffff10000000;

// The main translation table.
// Types - using global types from rustux and sys modules
pub use crate::rustux::types::VAddr as vaddr_t;
pub use crate::rustux::types::PAddr as paddr_t;
pub use crate::rustux::types::Size as size_t;
pub use crate::rustux::types::Status as rx_status_t;

// Page table entry type (ARM64 uses 64-bit PTEs)
pub type pte_t = u64;

// Constants - using global constants from rustux
pub use crate::rustux::types::err::RX_OK;
pub use crate::rustux::types::err::RX_ERR_INVALID_ARGS;
pub use crate::rustux::types::err::RX_ERR_NO_MEMORY;
// Additional error codes specific to MMU
pub const RX_ERR_OUT_OF_RANGE: rx_status_t = crate::rustux::types::err::RX_ERR_OUT_OF_RANGE;
pub const RX_ERR_INTERNAL: rx_status_t = crate::rustux::types::err::RX_ERR_INTERNAL;

const PAGE_SIZE: u64 = 4096;
const PAGE_SIZE_SHIFT: u32 = 12;
const PAGE_MASK: u64 = PAGE_SIZE - 1;

// Physical memory mapping window base (for identity mapping)
const KERNEL_PHYSMAP_BASE: u64 = 0xffff_0000_0000_0000;
const KERNEL_PHYSMAP_SIZE: u64 = 0x0000_ffff_ffff_f000;

/// Convert a physical address to its virtual address in the kernel's physical mapping window
///
/// This is used to access physical memory through the kernel's direct-mapped region.
///
/// # Arguments
///
/// * `paddr` - Physical address to convert
///
/// # Returns
///
/// Virtual address that can be used to access the physical memory
#[inline]
pub fn paddr_to_physmap(paddr: paddr_t) -> vaddr_t {
    // For now, we assume the kernel has an identity mapping or physical mapping window
    // In a real implementation, this would add the base of the PHYSMAP region
    (KERNEL_PHYSMAP_BASE + (paddr as u64)) as vaddr_t
}

pub const MMU_ARM64_ASID_BITS: u32 = 16;
pub const MMU_ARM64_GLOBAL_ASID: u32 = 0;
pub const MMU_ARM64_FIRST_USER_ASID: u16 = 1;
pub const MMU_ARM64_MAX_USER_ASID: u16 = ((1u32 << MMU_ARM64_ASID_BITS) - 2) as u16;
pub const MMU_ARM64_UNUSED_ASID: u16 = 0;

const KERNEL_BASE: u64 = 0xffffffff80000000;
const KERNEL_ASPACE_BASE: u64 = 0xffffff8000000000;

const MMU_KERNEL_SIZE_SHIFT: u32 = 39;
const MMU_KERNEL_PAGE_SIZE_SHIFT: u32 = 12;
const MMU_KERNEL_TOP_SHIFT: u32 = 25;

const MMU_USER_SIZE_SHIFT: u32 = 48;
const MMU_USER_PAGE_SIZE_SHIFT: u32 = 12;
const MMU_USER_TOP_SHIFT: u32 = 36;

const MMU_GUEST_SIZE_SHIFT: u32 = 48;
const MMU_GUEST_PAGE_SIZE_SHIFT: u32 = 12;
const MMU_GUEST_TOP_SHIFT: u32 = 39;

const MMU_KERNEL_PAGE_TABLE_ENTRIES_TOP: usize = 512;
const MMU_USER_PAGE_TABLE_ENTRIES_TOP: usize = 512;
const MMU_GUEST_PAGE_TABLE_ENTRIES_TOP: usize = 512;

const MMU_MAX_PAGE_SIZE_SHIFT: u32 = 48;

// MMU PTE attributes
const MMU_PTE_DESCRIPTOR_MASK: u64 = 0b11;
const MMU_PTE_DESCRIPTOR_INVALID: u64 = 0b00;
const MMU_PTE_L012_DESCRIPTOR_TABLE: u64 = 0b11;
const MMU_PTE_L012_DESCRIPTOR_BLOCK: u64 = 0b01;
const MMU_PTE_L3_DESCRIPTOR_PAGE: u64 = 0b11;
const MMU_PTE_DESCRIPTOR_BLOCK_MAX_SHIFT: u32 = 30;

const MMU_PTE_OUTPUT_ADDR_MASK: u64 = 0x000FFFFFFFFFF000;
const MMU_PTE_PERMISSION_MASK: u64 = 0xFFF0000000000FFC;

const MMU_PTE_ATTR_NON_SECURE: u64 = 1 << 5;
const MMU_PTE_ATTR_UXN: u64 = 1 << 54;
const MMU_PTE_ATTR_PXN: u64 = 1 << 53;
const MMU_PTE_ATTR_AF: u64 = 1 << 10;
const MMU_PTE_ATTR_NON_GLOBAL: u64 = 1 << 11;

// Attribute indexes
const MMU_PTE_ATTR_ATTR_INDEX_MASK: u64 = 0b111 << 2;
const MMU_PTE_ATTR_STRONGLY_ORDERED: u64 = 0b000 << 2;
const MMU_PTE_ATTR_DEVICE: u64 = 0b001 << 2;
const MMU_PTE_ATTR_NORMAL_UNCACHED: u64 = 0b010 << 2;
const MMU_PTE_ATTR_NORMAL_MEMORY: u64 = 0b011 << 2;

// Access permissions
const MMU_PTE_ATTR_AP_MASK: u64 = 0b11 << 6;
const MMU_PTE_ATTR_AP_P_RW_U_NA: u64 = 0b00 << 6;
const MMU_PTE_ATTR_AP_P_RW_U_RW: u64 = 0b01 << 6;
const MMU_PTE_ATTR_AP_P_RO_U_NA: u64 = 0b10 << 6;
const MMU_PTE_ATTR_AP_P_RO_U_RO: u64 = 0b11 << 6;

// Shareability attributes
const MMU_PTE_ATTR_SH_NON_SHAREABLE: u64 = 0b00 << 8;
const MMU_PTE_ATTR_SH_OUTER_SHAREABLE: u64 = 0b10 << 8;
const MMU_PTE_ATTR_SH_INNER_SHAREABLE: u64 = 0b11 << 8;

// Stage 2 attributes
const MMU_S2_PTE_ATTR_ATTR_INDEX_MASK: u64 = 0b111 << 2;
const MMU_S2_PTE_ATTR_STRONGLY_ORDERED: u64 = 0b000 << 2;
const MMU_S2_PTE_ATTR_DEVICE: u64 = 0b001 << 2;
const MMU_S2_PTE_ATTR_NORMAL_UNCACHED: u64 = 0b010 << 2;
const MMU_S2_PTE_ATTR_NORMAL_MEMORY: u64 = 0b011 << 2;

const MMU_S2_PTE_ATTR_XN: u64 = 1 << 54;
const MMU_S2_PTE_ATTR_S2AP_RO: u64 = 0b01 << 6;
const MMU_S2_PTE_ATTR_S2AP_RW: u64 = 0b11 << 6;

// MMU flags
const ARCH_MMU_FLAG_PERM_READ: u32 = 1 << 0;
const ARCH_MMU_FLAG_PERM_WRITE: u32 = 1 << 1;
const ARCH_MMU_FLAG_PERM_EXECUTE: u32 = 1 << 2;
const ARCH_MMU_FLAG_PERM_USER: u32 = 1 << 3;
const ARCH_MMU_FLAG_NS: u32 = 1 << 5;

const ARCH_MMU_FLAG_CACHED: u32 = 0 << 6;
const ARCH_MMU_FLAG_UNCACHED: u32 = 1 << 6;
const ARCH_MMU_FLAG_UNCACHED_DEVICE: u32 = 2 << 6;
const ARCH_MMU_FLAG_WRITE_COMBINING: u32 = 3 << 6;
const ARCH_MMU_FLAG_CACHE_MASK: u32 = 3 << 6;

// Aspace flags
const ARCH_ASPACE_FLAG_KERNEL: u32 = 1 << 0;
const ARCH_ASPACE_FLAG_GUEST: u32 = 1 << 1;

// Memory barrier types
const ARM_MB_SY: u32 = 15;
const ARM_MB_ISHST: u32 = 11;

// TCR flags
const MMU_TCR_FLAGS_KERNEL: u64 = 0;
const MMU_TCR_FLAGS_USER: u64 = 0;

// Spin lock flags
const ARCH_DEFAULT_SPIN_LOCK_FLAG_INTERRUPTS: u64 = 0;

// Structs and implementations
struct Canary {
    magic: u64,
}

impl Canary {
    fn new() -> Self {
        Canary { magic: 0xCAFEF00D }
    }
    
    fn assert(&self) {
        assert_eq!(self.magic, 0xCAFEF00D, "Canary assertion failed");
    }
}

struct Mutex<T> {
    data: core::cell::UnsafeCell<T>,
}

impl<T> Mutex<T> {
    fn new(data: T) -> Self {
        Mutex { data: core::cell::UnsafeCell::new(data) }
    }
    
    fn lock(&self) -> MutexGuard<T> {
        // In a real implementation, this would actually lock the mutex
        MutexGuard { mutex: self }
    }
}

struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        // In a real implementation, this would unlock the mutex
    }
}

// VM page types
struct vm_page_t {
    state: u32,
    // Other fields...
}

const VM_PAGE_STATE_MMU: u32 = 1;

// Kernel page table, aligned to 8 bytes
#[repr(C)]
struct AlignedPageTable {
    data: [pte_t; MMU_KERNEL_PAGE_TABLE_ENTRIES_TOP],
}

static mut arm64_kernel_translation_table: AlignedPageTable = AlignedPageTable {
    data: [0; MMU_KERNEL_PAGE_TABLE_ENTRIES_TOP],
};

pub fn arm64_get_kernel_ptable() -> *mut pte_t {
    unsafe { arm64_kernel_translation_table.data.as_mut_ptr() }
}

struct ArmArchVmAspace {
    lock: Mutex<()>,
    canary: Canary,
    flags: u32,
    base: vaddr_t,
    size: size_t,
    tt_virt: *mut pte_t,
    tt_phys: paddr_t,
    asid: u16,
    pt_pages: u32,
}

// Manual Debug implementation since Mutex doesn't implement Debug
impl core::fmt::Debug for ArmArchVmAspace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ArmArchVmAspace")
            .field("flags", &self.flags)
            .field("base", &self.base)
            .field("size", &self.size)
            .field("tt_virt", &self.tt_virt)
            .field("tt_phys", &self.tt_phys)
            .field("asid", &self.asid)
            .field("pt_pages", &self.pt_pages)
            .finish()
    }
}

struct AsidAllocator {
    lock: Mutex<()>,
    last: u16, // Guarded by lock
    bitmap: RawBitmapGeneric<FixedStorage<{ (MMU_ARM64_MAX_USER_ASID + 1) as usize }>>, // Guarded by lock
    asid: u16,
}

impl AsidAllocator {
    fn new() -> Self {
        let mut allocator = AsidAllocator {
            lock: Mutex::new(()),
            last: MMU_ARM64_FIRST_USER_ASID - 1,
            bitmap: RawBitmapGeneric::from(FixedStorage::default()),
            asid: 0,
        };
        allocator.bitmap.reset((MMU_ARM64_MAX_USER_ASID + 1) as usize);
        allocator
    }

    fn alloc(&mut self) -> rx_status_t {
        let mut new_asid: u16 = 0;

        // use the bitmap allocator to allocate ids in the range of
        // [MMU_ARM64_FIRST_USER_ASID, MMU_ARM64_MAX_USER_ASID]
        // start the search from the last found id + 1 and wrap when hitting the end of the range
        {
            let _guard = self.lock.lock();

            let mut val: usize = 0;
            let mut notfound = self.bitmap.get(self.last as usize + 1, (MMU_ARM64_MAX_USER_ASID + 1) as usize, &mut val);
            if notfound {
                // search again from the start
                notfound = self.bitmap.get(MMU_ARM64_FIRST_USER_ASID as usize, (MMU_ARM64_MAX_USER_ASID + 1) as usize, &mut val);
                if notfound {
                    trace!("ARM64: out of ASIDs\n");
                    return RX_ERR_NO_MEMORY;
                }
            }
            self.bitmap.set_one(val);

            debug_assert!(val <= u16::MAX as usize);

            new_asid = val as u16;
            self.last = new_asid;
        }

        ltrace!("new asid {:#x}\n", new_asid);

        self.asid = new_asid;

        RX_OK
    }

    fn free(&mut self, asid: u16) -> rx_status_t {
        ltrace!("free asid {:#x}\n", asid);

        let _guard = self.lock.lock();

        self.bitmap.clear_one(asid as usize);

        RX_OK
    }
}

// Global ASID allocator
static mut ASID: core::mem::MaybeUninit<AsidAllocator> = core::mem::MaybeUninit::uninit();
static mut ASID_INITIALIZED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

// Helper to get the ASID allocator
fn get_asid_allocator() -> &'static mut AsidAllocator {
    unsafe {
        // Initialize on first use
        if !ASID_INITIALIZED.load(core::sync::atomic::Ordering::Acquire) {
            ASID.write(AsidAllocator::new());
            ASID_INITIALIZED.store(true, core::sync::atomic::Ordering::Release);
        }
        ASID.assume_init_mut()
    }
}

// Convert user level mmu flags to flags that go in L1 descriptors.
fn mmu_flags_to_s1_pte_attr(flags: u32) -> pte_t {
    let mut attr: pte_t = MMU_PTE_ATTR_AF;

    match flags & ARCH_MMU_FLAG_CACHE_MASK {
        ARCH_MMU_FLAG_CACHED => {
            attr |= MMU_PTE_ATTR_NORMAL_MEMORY | MMU_PTE_ATTR_SH_INNER_SHAREABLE;
        },
        ARCH_MMU_FLAG_WRITE_COMBINING => {
            attr |= MMU_PTE_ATTR_NORMAL_UNCACHED | MMU_PTE_ATTR_SH_INNER_SHAREABLE;
        },
        ARCH_MMU_FLAG_UNCACHED => {
            attr |= MMU_PTE_ATTR_STRONGLY_ORDERED;
        },
        ARCH_MMU_FLAG_UNCACHED_DEVICE => {
            attr |= MMU_PTE_ATTR_DEVICE;
        },
        _ => {
            panic!("Unimplemented");
        }
    }

    match flags & (ARCH_MMU_FLAG_PERM_USER | ARCH_MMU_FLAG_PERM_WRITE) {
        0 => {
            attr |= MMU_PTE_ATTR_AP_P_RO_U_NA;
        },
        ARCH_MMU_FLAG_PERM_WRITE => {
            attr |= MMU_PTE_ATTR_AP_P_RW_U_NA;
        },
        ARCH_MMU_FLAG_PERM_USER => {
            attr |= MMU_PTE_ATTR_AP_P_RO_U_RO;
        },
        ARCH_MMU_FLAG_PERM_USER | ARCH_MMU_FLAG_PERM_WRITE => {
            attr |= MMU_PTE_ATTR_AP_P_RW_U_RW;
        },
        _ => {}
    }

    if (flags & ARCH_MMU_FLAG_PERM_EXECUTE) == 0 {
        attr |= MMU_PTE_ATTR_UXN | MMU_PTE_ATTR_PXN;
    }
    if flags & ARCH_MMU_FLAG_NS != 0 {
        attr |= MMU_PTE_ATTR_NON_SECURE;
    }

    attr
}

fn s1_pte_attr_to_mmu_flags(pte: pte_t, mmu_flags: &mut u32) {
    match pte & MMU_PTE_ATTR_ATTR_INDEX_MASK {
        MMU_PTE_ATTR_STRONGLY_ORDERED => {
            *mmu_flags |= ARCH_MMU_FLAG_UNCACHED;
        },
        MMU_PTE_ATTR_DEVICE => {
            *mmu_flags |= ARCH_MMU_FLAG_UNCACHED_DEVICE;
        },
        MMU_PTE_ATTR_NORMAL_UNCACHED => {
            *mmu_flags |= ARCH_MMU_FLAG_WRITE_COMBINING;
        },
        MMU_PTE_ATTR_NORMAL_MEMORY => {
            *mmu_flags |= ARCH_MMU_FLAG_CACHED;
        },
        _ => {
            panic!("Unimplemented");
        }
    }

    *mmu_flags |= ARCH_MMU_FLAG_PERM_READ;
    match pte & MMU_PTE_ATTR_AP_MASK {
        MMU_PTE_ATTR_AP_P_RW_U_NA => {
            *mmu_flags |= ARCH_MMU_FLAG_PERM_WRITE;
        },
        MMU_PTE_ATTR_AP_P_RW_U_RW => {
            *mmu_flags |= ARCH_MMU_FLAG_PERM_USER | ARCH_MMU_FLAG_PERM_WRITE;
        },
        MMU_PTE_ATTR_AP_P_RO_U_NA => {},
        MMU_PTE_ATTR_AP_P_RO_U_RO => {
            *mmu_flags |= ARCH_MMU_FLAG_PERM_USER;
        },
        _ => {}
    }

    if !((pte & MMU_PTE_ATTR_UXN != 0) && (pte & MMU_PTE_ATTR_PXN != 0)) {
        *mmu_flags |= ARCH_MMU_FLAG_PERM_EXECUTE;
    }
    if pte & MMU_PTE_ATTR_NON_SECURE != 0 {
        *mmu_flags |= ARCH_MMU_FLAG_NS;
    }
}

fn mmu_flags_to_s2_pte_attr(flags: u32) -> pte_t {
    let mut attr: pte_t = MMU_PTE_ATTR_AF;

    match flags & ARCH_MMU_FLAG_CACHE_MASK {
        ARCH_MMU_FLAG_CACHED => {
            attr |= MMU_S2_PTE_ATTR_NORMAL_MEMORY | MMU_PTE_ATTR_SH_INNER_SHAREABLE;
        },
        ARCH_MMU_FLAG_WRITE_COMBINING => {
            attr |= MMU_S2_PTE_ATTR_NORMAL_UNCACHED | MMU_PTE_ATTR_SH_INNER_SHAREABLE;
        },
        ARCH_MMU_FLAG_UNCACHED => {
            attr |= MMU_S2_PTE_ATTR_STRONGLY_ORDERED;
        },
        ARCH_MMU_FLAG_UNCACHED_DEVICE => {
            attr |= MMU_S2_PTE_ATTR_DEVICE;
        },
        _ => {
            panic!("Unimplemented");
        }
    }

    if flags & ARCH_MMU_FLAG_PERM_WRITE != 0 {
        attr |= MMU_S2_PTE_ATTR_S2AP_RW;
    } else {
        attr |= MMU_S2_PTE_ATTR_S2AP_RO;
    }
    if (flags & ARCH_MMU_FLAG_PERM_EXECUTE) == 0 {
        attr |= MMU_S2_PTE_ATTR_XN;
    }

    attr
}

fn s2_pte_attr_to_mmu_flags(pte: pte_t, mmu_flags: &mut u32) {
    match pte & MMU_S2_PTE_ATTR_ATTR_INDEX_MASK {
        MMU_S2_PTE_ATTR_STRONGLY_ORDERED => {
            *mmu_flags |= ARCH_MMU_FLAG_UNCACHED;
        },
        MMU_S2_PTE_ATTR_DEVICE => {
            *mmu_flags |= ARCH_MMU_FLAG_UNCACHED_DEVICE;
        },
        MMU_S2_PTE_ATTR_NORMAL_UNCACHED => {
            *mmu_flags |= ARCH_MMU_FLAG_WRITE_COMBINING;
        },
        MMU_S2_PTE_ATTR_NORMAL_MEMORY => {
            *mmu_flags |= ARCH_MMU_FLAG_CACHED;
        },
        _ => {
            panic!("Unimplemented");
        }
    }

    *mmu_flags |= ARCH_MMU_FLAG_PERM_READ;
    match pte & MMU_PTE_ATTR_AP_MASK {
        MMU_S2_PTE_ATTR_S2AP_RO => {},
        MMU_S2_PTE_ATTR_S2AP_RW => {
            *mmu_flags |= ARCH_MMU_FLAG_PERM_WRITE;
        },
        _ => {
            panic!("Unimplemented");
        }
    }

    if pte & MMU_S2_PTE_ATTR_XN != 0 {
        *mmu_flags |= ARCH_MMU_FLAG_PERM_EXECUTE;
    }
}

impl ArmArchVmAspace {
    pub fn query(&self, vaddr: vaddr_t, paddr: &mut paddr_t, mmu_flags: &mut u32) -> rx_status_t {
        let _guard = self.lock.lock();
        self.query_locked(vaddr, Some(paddr), Some(mmu_flags))
    }

    fn query_locked(&self, vaddr: vaddr_t, paddr: Option<&mut paddr_t>, mmu_flags: Option<&mut u32>) -> rx_status_t {
        let mut index: usize;
        let mut index_shift: u32;
        let mut page_size_shift: u32;
        let mut pte: pte_t;
        let mut pte_addr: pte_t;
        let mut descriptor_type: u64;
        let mut page_table: *mut pte_t;
        let mut vaddr_rem: vaddr_t;

        self.canary.assert();
        ltrace!("aspace {:p}, vaddr {:#x}\n", self, vaddr);

        debug_assert!(self.tt_virt != ptr::null_mut());

        debug_assert!(self.is_valid_vaddr(vaddr));
        if !self.is_valid_vaddr(vaddr) {
            return RX_ERR_OUT_OF_RANGE;
        }

        // Compute shift values based on if this address space is for kernel or user space.
        if self.flags & ARCH_ASPACE_FLAG_KERNEL != 0 {
            index_shift = MMU_KERNEL_TOP_SHIFT;
            page_size_shift = MMU_KERNEL_PAGE_SIZE_SHIFT;

            let kernel_base = (!0u64 << MMU_KERNEL_SIZE_SHIFT) as vaddr_t;
            vaddr_rem = vaddr - kernel_base;

            index = (vaddr_rem >> index_shift) as usize;
            assert!(index < MMU_KERNEL_PAGE_TABLE_ENTRIES_TOP);
        } else if self.flags & ARCH_ASPACE_FLAG_GUEST != 0 {
            index_shift = MMU_GUEST_TOP_SHIFT;
            page_size_shift = MMU_GUEST_PAGE_SIZE_SHIFT;

            vaddr_rem = vaddr;
            index = (vaddr_rem >> index_shift) as usize;
            assert!(index < MMU_GUEST_PAGE_TABLE_ENTRIES_TOP);
        } else {
            index_shift = MMU_USER_TOP_SHIFT;
            page_size_shift = MMU_USER_PAGE_SIZE_SHIFT;

            vaddr_rem = vaddr;
            index = (vaddr_rem >> index_shift) as usize;
            assert!(index < MMU_USER_PAGE_TABLE_ENTRIES_TOP);
        }

        page_table = self.tt_virt;

        loop {
            index = (vaddr_rem >> index_shift) as usize;
            vaddr_rem -= ((index as u64) << index_shift) as vaddr_t;
            unsafe { pte = *page_table.add(index) };
            descriptor_type = pte & MMU_PTE_DESCRIPTOR_MASK;
            pte_addr = pte & MMU_PTE_OUTPUT_ADDR_MASK;

            ltrace!("va {:#x}, index {}, index_shift {}, rem {:#x}, pte {:#x}\n",
                   vaddr, index, index_shift, vaddr_rem, pte);

            if descriptor_type == MMU_PTE_DESCRIPTOR_INVALID {
                return RX_ERR_NOT_FOUND;
            }

            if descriptor_type == (if index_shift > page_size_shift { 
                MMU_PTE_L012_DESCRIPTOR_BLOCK 
            } else { 
                MMU_PTE_L3_DESCRIPTOR_PAGE 
            }) {
                break;
            }

            if index_shift <= page_size_shift || descriptor_type != MMU_PTE_L012_DESCRIPTOR_TABLE {
                panic!("Unimplemented");
            }

            page_table = paddr_to_physmap(pte_addr) as *mut pte_t;
            index_shift -= page_size_shift - 3;
        }

        // SAFETY: We have exclusive access to modify these values through the Option<&mut>
        unsafe {
            if let Some(paddr_ref) = paddr {
                *paddr_ref = pte_addr + (vaddr_rem as u64);
            }

            if let Some(mmu_flags_ref) = mmu_flags {
                *mmu_flags_ref = 0;
                if self.flags & ARCH_ASPACE_FLAG_GUEST != 0 {
                    s2_pte_attr_to_mmu_flags(pte, mmu_flags_ref);
                } else {
                    s1_pte_attr_to_mmu_flags(pte, mmu_flags_ref);
                }
            }
        }

        // Note: We can't use paddr and mmu_flags for logging here since they were moved above
        // The values have been modified in place, so we skip the debug trace here
        ltrace!("va {:#x}, query complete\n", vaddr);
               
        0
    }

    fn alloc_page_table(&mut self, paddrp: &mut paddr_t, page_size_shift: u32) -> rx_status_t {
        ltrace!("page_size_shift {}\n", page_size_shift);

        // currently we only support allocating a single page
        debug_assert!(page_size_shift == PAGE_SIZE_SHIFT);

        let (page, paddr) = match pmm_alloc_page(0) {
            Ok((page, paddr)) => (page, paddr),
            Err(_) => return RX_ERR_NO_MEMORY,
        };
        if page.is_null() {
            return RX_ERR_NO_MEMORY;
        }
        *paddrp = paddr;

        // TODO: Set page state when VM page management is implemented
        // unsafe { (*page).state = VM_PAGE_STATE_MMU; }
        self.pt_pages += 1;

        // local_ktrace0!("page table alloc"); // TODO: Fix trace tag type

        ltrace!("allocated {:#x}\n", *paddrp);
        0
    }

    fn free_page_table(&mut self, vaddr: *mut c_void, paddr: paddr_t, page_size_shift: u32) {
        ltrace!("vaddr {:p} paddr {:#x} page_size_shift {}\n", vaddr, paddr, page_size_shift);

        // currently we only support freeing a single page
        debug_assert!(page_size_shift == PAGE_SIZE_SHIFT);

        // TODO: Implement proper VM page management
        // Use fully qualified path to avoid ambiguity
        let _page = crate::vm::physmap::paddr_to_vm_page(paddr);
        // local_ktrace0!("page table free"); // TODO: Fix trace tag type

        // pmm_free_page(_page as *mut u8); // TODO: Implement when VM page management is ready

        self.pt_pages -= 1;
    }

    fn get_page_table(&mut self, index: vaddr_t, page_size_shift: u32,
                      page_table: *mut pte_t) -> *mut pte_t {
        debug_assert!(page_size_shift <= MMU_MAX_PAGE_SIZE_SHIFT);

        let pte: pte_t = unsafe { *page_table.add(index as usize) };
        match pte & MMU_PTE_DESCRIPTOR_MASK {
            MMU_PTE_DESCRIPTOR_INVALID => {
                let mut paddr: paddr_t = 0;
                let ret = self.alloc_page_table(&mut paddr, page_size_shift);
                if ret != 0 {
                    trace!("failed to allocate page table\n");
                    return ptr::null_mut();
                }
                let vaddr = paddr_to_physmap(paddr) as *mut c_void;

                ltrace!("allocated page table, vaddr {:p}, paddr {:#x}\n", vaddr, paddr);
                unsafe { ptr::write_bytes(vaddr, MMU_PTE_DESCRIPTOR_INVALID as u8, 1 << page_size_shift); }

                // ensure that the zeroing is observable from hardware page table walkers
                unsafe { __dmb(ARM_MB_ISHST); }

                let new_pte = paddr | MMU_PTE_L012_DESCRIPTOR_TABLE;
                unsafe { *page_table.add(index as usize) = new_pte; }
                ltrace!("pte {:p}[{:#x}] = {:#x}\n",
                       page_table, index, new_pte);
                vaddr as *mut pte_t
            },
            MMU_PTE_L012_DESCRIPTOR_TABLE => {
                let paddr = pte & MMU_PTE_OUTPUT_ADDR_MASK;
                ltrace!("found page table {:#x}\n", paddr);
                paddr_to_physmap(paddr) as *mut pte_t
            },
            MMU_PTE_L012_DESCRIPTOR_BLOCK => {
                ptr::null_mut()
            },
            _ => {
                panic!("Unimplemented");
            }
        }
    }
    
    // Rest of implementation...
}

fn page_table_is_clear(page_table: *const pte_t, page_size_shift: u32) -> bool {
    let count = 1u32 << (page_size_shift - 3);
    
    for i in 0..count as usize {
        let pte = unsafe { *page_table.add(i) };
        if pte != MMU_PTE_DESCRIPTOR_INVALID {
            ltrace!("page_table at {:p} still in use, index {} is {:#x}\n",
                   page_table, i, pte);
            return false;
        }
    }

    ltrace!("page table at {:p} is clear\n", page_table);
    true
}

// Use the appropriate TLB flush instruction to globally flush the modified entry
// terminal is set when flushing at the final level of the page table.
impl ArmArchVmAspace {
    fn flush_tlb_entry(&self, vaddr: vaddr_t, terminal: bool) {
        if self.flags & ARCH_ASPACE_FLAG_GUEST != 0 {
            let vttbr = arm64_vttbr(self.asid, self.tt_phys);
            let _status = arm64_el2_tlbi_ipa(vttbr);
            // TODO: Handle terminal parameter when EL2 TLBI is fully implemented
            let _terminal = terminal;
            debug_assert!(_status == RX_OK);
        } else if self.asid == MMU_ARM64_GLOBAL_ASID as u16 {
            // flush this address on all ASIDs
            if terminal {
                ARM64_TLBI!(vaale1is, vaddr >> 12);
            } else {
                ARM64_TLBI!(vaae1is, vaddr >> 12);
            }
        } else {
            // flush this address for the specific asid
            if terminal {
                ARM64_TLBI!(vale1is, (vaddr >> 12) | ((self.asid as vaddr_t) << 48));
            } else {
                ARM64_TLBI!(vae1is, (vaddr >> 12) | ((self.asid as vaddr_t) << 48));
            }
        }
    }

    // NOTE: caller must DSB afterwards to ensure TLB entries are flushed
    fn unmap_page_table(&mut self, vaddr: vaddr_t, vaddr_rel: vaddr_t,
                        size: size_t, index_shift: u32,
                        page_size_shift: u32,
                        page_table: *mut pte_t) -> isize {
        let mut next_page_table: *mut pte_t;
        let mut index: vaddr_t;
        let mut chunk_size: size_t;
        let mut vaddr_rem: vaddr_t;
        let mut block_size: vaddr_t;
        let mut block_mask: vaddr_t;
        let mut pte: pte_t;
        let mut page_table_paddr: paddr_t;
        let mut unmap_size: size_t = 0;

        ltrace!("vaddr {:#x}, vaddr_rel {:#x}, size {:#x}, index shift {}, page_size_shift {}, page_table {:p}\n",
                vaddr, vaddr_rel, size, index_shift, page_size_shift, page_table);

        let mut remaining_size = size;
        let mut current_vaddr = vaddr;
        let mut current_vaddr_rel = vaddr_rel;

        while remaining_size > 0 {
            block_size = (1u64 << index_shift) as vaddr_t;
            block_mask = block_size - 1;
            vaddr_rem = current_vaddr_rel & block_mask;
            chunk_size = core::cmp::min(remaining_size, block_size - vaddr_rem as size_t);
            index = current_vaddr_rel >> index_shift;

            unsafe { pte = *page_table.add(index as usize); }

            if index_shift > page_size_shift &&
               (pte & MMU_PTE_DESCRIPTOR_MASK) == MMU_PTE_L012_DESCRIPTOR_TABLE {
                page_table_paddr = pte & MMU_PTE_OUTPUT_ADDR_MASK;
                next_page_table = paddr_to_physmap(page_table_paddr) as *mut pte_t;
                let inner_unmap_size = self.unmap_page_table(
                    current_vaddr, vaddr_rem, chunk_size,
                    index_shift - (page_size_shift - 3),
                    page_size_shift, next_page_table);
                
                if chunk_size == block_size as size_t ||
                   page_table_is_clear(next_page_table, page_size_shift) {
                    ltrace!("pte {:p}[{:#x}] = 0 (was page table)\n", page_table, index);
                    unsafe { *page_table.add(index as usize) = MMU_PTE_DESCRIPTOR_INVALID; }

                    // ensure that the update is observable from hardware page table walkers
                    unsafe { __dmb(ARM_MB_ISHST); }

                    // flush the non terminal TLB entry
                    self.flush_tlb_entry(current_vaddr, false);

                    self.free_page_table(next_page_table as *mut c_void, page_table_paddr, page_size_shift);
                }
            } else if pte != 0 {
                ltrace!("pte {:p}[{:#x}] = 0\n", page_table, index);
                unsafe { *page_table.add(index as usize) = MMU_PTE_DESCRIPTOR_INVALID; }

                // ensure that the update is observable from hardware page table walkers
                unsafe { __dmb(ARM_MB_ISHST); }

                // flush the terminal TLB entry
                self.flush_tlb_entry(current_vaddr, true);
            } else {
                ltrace!("pte {:p}[{:#x}] already clear\n", page_table, index);
            }
            
            current_vaddr += chunk_size as vaddr_t;
            current_vaddr_rel += chunk_size as vaddr_t;
            remaining_size -= chunk_size;
            unmap_size += chunk_size;
        }

        unmap_size as isize
    }

    // NOTE: caller must DSB afterwards to ensure TLB entries are flushed
    fn map_page_table(&mut self, vaddr_in: vaddr_t, vaddr_rel_in: vaddr_t,
                      paddr_in: paddr_t, size_in: size_t,
                      attrs: pte_t, index_shift: u32,
                      page_size_shift: u32,
                      page_table: *mut pte_t) -> isize {
        let mut next_page_table: *mut pte_t;
        let mut index: vaddr_t;
        let mut vaddr = vaddr_in;
        let mut vaddr_rel = vaddr_rel_in;
        let mut paddr = paddr_in;
        let mut size = size_in;
        let mut chunk_size: size_t;
        let mut vaddr_rem: vaddr_t;
        let mut block_size: vaddr_t;
        let mut block_mask: vaddr_t;
        let mut pte: pte_t;
        let mut mapped_size: size_t = 0;

        ltrace!("vaddr {:#x}, vaddr_rel {:#x}, paddr {:#x}, size {:#x}, attrs {:#x}, index shift {}, page_size_shift {}, page_table {:p}\n",
                vaddr, vaddr_rel, paddr, size, attrs,
                index_shift, page_size_shift, page_table);

        if ((vaddr_rel as u64) | paddr | size as u64) & ((1u64 << page_size_shift) - 1) != 0 {
            trace!("not page aligned\n");
            return RX_ERR_INVALID_ARGS as isize;
        }

        while size > 0 {
            block_size = (1u64 << index_shift) as vaddr_t;
            block_mask = block_size - 1;
            vaddr_rem = vaddr_rel & block_mask;
            chunk_size = core::cmp::min(size, (block_size - vaddr_rem) as size_t);
            index = vaddr_rel >> index_shift;

            if (((vaddr_rel as u64) | paddr) & (block_mask as u64)) != 0 ||
               (chunk_size != block_size as size_t) ||
               (index_shift > MMU_PTE_DESCRIPTOR_BLOCK_MAX_SHIFT) {
                next_page_table = self.get_page_table(index, page_size_shift, page_table);
                if next_page_table.is_null() {
                    goto_err!();
                }

                let ret = self.map_page_table(vaddr, vaddr_rem, paddr, chunk_size, attrs,
                           index_shift - (page_size_shift - 3),
                           page_size_shift, next_page_table);
                if ret < 0 {
                    self.unmap_page_table(vaddr_in, vaddr_rel_in, size_in - size, index_shift,
                                        page_size_shift, page_table);
                    return RX_ERR_INTERNAL as isize;
                }
            } else {
                unsafe { pte = *page_table.add(index as usize); }
                if pte != 0 {
                    trace!("page table entry already in use, index {:#x}, {:#x}\n",
                           index, pte);
                    self.unmap_page_table(vaddr_in, vaddr_rel_in, size_in - size, index_shift,
                                        page_size_shift, page_table);
                    return RX_ERR_INTERNAL as isize;
                }

                pte = paddr | attrs;
                if index_shift > page_size_shift {
                    pte |= MMU_PTE_L012_DESCRIPTOR_BLOCK;
                } else {
                    pte |= MMU_PTE_L3_DESCRIPTOR_PAGE;
                }
                if (self.flags & ARCH_ASPACE_FLAG_GUEST) == 0 {
                    pte |= MMU_PTE_ATTR_NON_GLOBAL;
                }
                ltrace!("pte {:p}[{:#x}] = {:#x}\n",
                        page_table, index, pte);
                unsafe { *page_table.add(index as usize) = pte; }
            }
            vaddr += chunk_size as vaddr_t;
            vaddr_rel += chunk_size as vaddr_t;
            paddr += (chunk_size as vaddr_t) as u64;
            size -= chunk_size;
            mapped_size += chunk_size;
        }

        return mapped_size as isize;
    }

    // NOTE: caller must DSB afterwards to ensure TLB entries are flushed
    fn protect_page_table(&mut self, vaddr_in: vaddr_t, vaddr_rel_in: vaddr_t,
                         size_in: size_t, attrs: pte_t,
                         index_shift: u32, page_size_shift: u32,
                         page_table: *mut pte_t) -> rx_status_t {
        let mut next_page_table: *mut pte_t;
        let mut index: vaddr_t;
        let mut vaddr = vaddr_in;
        let mut vaddr_rel = vaddr_rel_in;
        let mut size = size_in;
        let mut chunk_size: size_t;
        let mut vaddr_rem: vaddr_t;
        let mut block_size: vaddr_t;
        let mut block_mask: vaddr_t;
        let mut page_table_paddr: paddr_t;
        let mut pte: pte_t;

        ltrace!("vaddr {:#x}, vaddr_rel {:#x}, size {:#x}, attrs {:#x}, index shift {}, page_size_shift {}, page_table {:p}\n",
                vaddr, vaddr_rel, size, attrs,
                index_shift, page_size_shift, page_table);

        if ((vaddr_rel as u64) | size as u64) & ((1u64 << page_size_shift) - 1) != 0 {
            trace!("not page aligned\n");
            return RX_ERR_INVALID_ARGS;
        }

        while size > 0 {
            block_size = (1u64 << index_shift) as vaddr_t;
            block_mask = block_size - 1;
            vaddr_rem = vaddr_rel & block_mask;
            chunk_size = core::cmp::min(size, (block_size - vaddr_rem) as size_t);
            index = vaddr_rel >> index_shift;
            unsafe { pte = *page_table.add(index as usize); }

            if index_shift > page_size_shift &&
               (pte & MMU_PTE_DESCRIPTOR_MASK) == MMU_PTE_L012_DESCRIPTOR_TABLE {
                page_table_paddr = pte & MMU_PTE_OUTPUT_ADDR_MASK;
                next_page_table = paddr_to_physmap(page_table_paddr) as *mut pte_t;
                let ret = self.protect_page_table(vaddr, vaddr_rem, chunk_size, attrs,
                                               index_shift - (page_size_shift - 3),
                                               page_size_shift, next_page_table);
                if ret != 0 {
                    goto_err!();
                }
            } else if pte != 0 {
                pte = (pte & !MMU_PTE_PERMISSION_MASK) | attrs;
                ltrace!("pte {:p}[{:#x}] = {:#x}\n",
                        page_table, index, pte);
                unsafe { *page_table.add(index as usize) = pte; }

                // ensure that the update is observable from hardware page table walkers
                unsafe { __dmb(ARM_MB_ISHST); }

                // flush the terminal TLB entry
                self.flush_tlb_entry(vaddr, true);
            } else {
                ltrace!("page table entry does not exist, index {:#x}, {:#x}\n",
                        index, pte);
            }
            vaddr += chunk_size as vaddr_t;
            vaddr_rel += chunk_size as vaddr_t;
            size -= chunk_size;
        }

        return 0;
    }

    // internal routine to map a run of pages
    fn map_pages(&mut self, vaddr: vaddr_t, paddr: paddr_t, size: size_t,
                attrs: pte_t, vaddr_base: vaddr_t, top_size_shift: u32,
                top_index_shift: u32, page_size_shift: u32) -> isize {
        let vaddr_rel = vaddr - vaddr_base;
        let vaddr_rel_max = (1u64 << top_size_shift) as vaddr_t;

        ltrace!("vaddr {:#x}, paddr {:#x}, size {:#x}, attrs {:#x}, asid {:#x}\n",
                vaddr, paddr, size, attrs, self.asid);

        if vaddr_rel > vaddr_rel_max - size || size > vaddr_rel_max {
            trace!("vaddr {:#x}, size {:#x} out of range vaddr {:#x}, size {:#x}\n",
                   vaddr, size, vaddr_base, vaddr_rel_max);
            return RX_ERR_INVALID_ARGS as isize;
        }

        // local_ktrace64!("mmu map", ((vaddr as u64) & !PAGE_MASK) | (((size >> PAGE_SIZE_SHIFT) as u64) & PAGE_MASK)); // TODO: Fix trace tag type
        let ret = self.map_page_table(vaddr, vaddr_rel, paddr, size, attrs,
                                   top_index_shift, page_size_shift, self.tt_virt);
        unsafe { __dsb(ARM_MB_SY); }
        ret
    }

    fn unmap_pages(&mut self, vaddr: vaddr_t, size: size_t,
                  vaddr_base: vaddr_t,
                  top_size_shift: u32,
                  top_index_shift: u32,
                  page_size_shift: u32) -> isize {
        let vaddr_rel = vaddr - vaddr_base;
        let vaddr_rel_max = (1u64 << top_size_shift) as vaddr_t;

        ltrace!("vaddr {:#x}, size {:#x}, asid {:#x}\n", vaddr, size, self.asid);

        if vaddr_rel > vaddr_rel_max - size || size > vaddr_rel_max {
            trace!("vaddr {:#x}, size {:#x} out of range vaddr {:#x}, size {:#x}\n",
                   vaddr, size, vaddr_base, vaddr_rel_max);
            return RX_ERR_INVALID_ARGS as isize;
        }

        // local_ktrace64!("mmu unmap", ((vaddr as u64) & !PAGE_MASK) | (((size >> PAGE_SIZE_SHIFT) as u64) & PAGE_MASK)); // TODO: Fix trace tag type

        let ret = self.unmap_page_table(vaddr, vaddr_rel, size, top_index_shift,
                                     page_size_shift, self.tt_virt);
        unsafe { __dsb(ARM_MB_SY); }
        ret
    }

    fn protect_pages(&mut self, vaddr: vaddr_t, size: size_t, attrs: pte_t,
                    vaddr_base: vaddr_t, top_size_shift: u32,
                    top_index_shift: u32, page_size_shift: u32) -> rx_status_t {
        let vaddr_rel = vaddr - vaddr_base;
        let vaddr_rel_max = (1u64 << top_size_shift) as vaddr_t;

        ltrace!("vaddr {:#x}, size {:#x}, attrs {:#x}, asid {:#x}\n",
                vaddr, size, attrs, self.asid);

        if vaddr_rel > vaddr_rel_max - size || size > vaddr_rel_max {
            trace!("vaddr {:#x}, size {:#x} out of range vaddr {:#x}, size {:#x}\n",
                   vaddr, size, vaddr_base, vaddr_rel_max);
            return RX_ERR_INVALID_ARGS;
        }

        // local_ktrace64!("mmu protect", ((vaddr as u64) & !PAGE_MASK) | (((size >> PAGE_SIZE_SHIFT) as u64) & PAGE_MASK)); // TODO: Fix trace tag type

        let ret = self.protect_page_table(vaddr, vaddr_rel, size, attrs,
                                       top_index_shift, page_size_shift,
                                       self.tt_virt);
        unsafe { __dsb(ARM_MB_SY); }
        ret
    }

    fn mmu_params_from_flags(&self, mmu_flags: u32,
                            attrs: Option<&mut pte_t>, 
                            vaddr_base: &mut vaddr_t,
                            top_size_shift: &mut u32, 
                            top_index_shift: &mut u32,
                            page_size_shift: &mut u32) {
        if self.flags & ARCH_ASPACE_FLAG_KERNEL != 0 {
            if let Some(attrs_val) = attrs {
                *attrs_val = mmu_flags_to_s1_pte_attr(mmu_flags);
            }
            *vaddr_base = (!0u64 << MMU_KERNEL_SIZE_SHIFT) as vaddr_t;
            *top_size_shift = MMU_KERNEL_SIZE_SHIFT;
            *top_index_shift = MMU_KERNEL_TOP_SHIFT;
            *page_size_shift = MMU_KERNEL_PAGE_SIZE_SHIFT;
        } else if self.flags & ARCH_ASPACE_FLAG_GUEST != 0 {
            if let Some(attrs_val) = attrs {
                *attrs_val = mmu_flags_to_s2_pte_attr(mmu_flags);
            }
            *vaddr_base = 0;
            *top_size_shift = MMU_GUEST_SIZE_SHIFT;
            *top_index_shift = MMU_GUEST_TOP_SHIFT;
            *page_size_shift = MMU_GUEST_PAGE_SIZE_SHIFT;
        } else {
            if let Some(attrs_val) = attrs {
                *attrs_val = mmu_flags_to_s1_pte_attr(mmu_flags);
            }
            *vaddr_base = 0;
            *top_size_shift = MMU_USER_SIZE_SHIFT;
            *top_index_shift = MMU_USER_TOP_SHIFT;
            *page_size_shift = MMU_USER_PAGE_SIZE_SHIFT;
        }
    }

    pub fn map_contiguous(&mut self, vaddr: vaddr_t, paddr: paddr_t, count: size_t,
                         mmu_flags: u32, mapped: Option<&mut size_t>) -> rx_status_t {
        self.canary.assert();
        ltrace!("vaddr {:#x} paddr {:#x} count {} flags {:#x}\n",
                vaddr, paddr, count, mmu_flags);

        debug_assert!(!self.tt_virt.is_null());

        debug_assert!(self.is_valid_vaddr(vaddr));
        if !self.is_valid_vaddr(vaddr) {
            return RX_ERR_OUT_OF_RANGE;
        }

        if (mmu_flags & ARCH_MMU_FLAG_PERM_READ) == 0 {
            return RX_ERR_INVALID_ARGS;
        }

        // paddr and vaddr must be aligned.
        debug_assert!(is_page_aligned(vaddr as u64));
        debug_assert!(is_page_aligned(paddr as u64));
        if !is_page_aligned(vaddr as u64) || !is_page_aligned(paddr as u64) {
            return RX_ERR_INVALID_ARGS;
        }

        if count == 0 {
            return RX_OK;
        }

        let mut attrs: pte_t = 0;
        let mut vaddr_base: vaddr_t = 0;
        let mut top_size_shift: u32 = 0;
        let mut top_index_shift: u32 = 0;
        let mut page_size_shift: u32 = 0;

        {
            let _guard = self.lock.lock();
            self.mmu_params_from_flags(mmu_flags, Some(&mut attrs), &mut vaddr_base, &mut top_size_shift,
                                     &mut top_index_shift, &mut page_size_shift);
        }
        // Guard is dropped here, allowing mutable access to self

        let ret = self.map_pages(vaddr, paddr, count * (PAGE_SIZE as size_t),
                               attrs, vaddr_base, top_size_shift,
                               top_index_shift, page_size_shift);

        if let Some(mapped_val) = mapped {
            *mapped_val = if ret > 0 { (ret / PAGE_SIZE as isize) as size_t } else { 0 };
            debug_assert!(*mapped_val <= count);
        }

        if ret < 0 { ret as rx_status_t } else { RX_OK }
    }

    pub fn map(&mut self, vaddr: vaddr_t, phys: &[paddr_t], count: size_t, mmu_flags: u32,
             mapped: Option<&mut size_t>) -> rx_status_t {
        self.canary.assert();
        ltrace!("vaddr {:#x} count {} flags {:#x}\n",
                vaddr, count, mmu_flags);

        debug_assert!(!self.tt_virt.is_null());

        debug_assert!(self.is_valid_vaddr(vaddr));
        if !self.is_valid_vaddr(vaddr) {
            return RX_ERR_OUT_OF_RANGE;
        }

        for i in 0..count {
            debug_assert!(is_page_aligned(phys[i as usize]));
            if !is_page_aligned(phys[i as usize]) {
                return RX_ERR_INVALID_ARGS;
            }
        }

        if (mmu_flags & ARCH_MMU_FLAG_PERM_READ) == 0 {
            return RX_ERR_INVALID_ARGS;
        }

        // vaddr must be aligned.
        debug_assert!(is_page_aligned(vaddr as u64));
        if !is_page_aligned(vaddr as u64) {
            return RX_ERR_INVALID_ARGS;
        }

        if count == 0 {
            return RX_OK;
        }

        let mut attrs: pte_t = 0;
        let mut vaddr_base: vaddr_t = 0;
        let mut top_size_shift: u32 = 0;
        let mut top_index_shift: u32 = 0;
        let mut page_size_shift: u32 = 0;

        {
            let _guard = self.lock.lock();
            self.mmu_params_from_flags(mmu_flags, Some(&mut attrs), &mut vaddr_base, &mut top_size_shift,
                                     &mut top_index_shift, &mut page_size_shift);
        }
        // Guard is dropped here

        let mut total_mapped: size_t = 0;
        let mut ret: isize;
        let mut idx: size_t = 0;
        let mut undo = false;

        let mut v = vaddr;
        for i in 0..count {
            let paddr = phys[i as usize];
            debug_assert!(is_page_aligned(paddr));
            // TODO: optimize by not DSBing inside each of these calls
            ret = self.map_pages(v, paddr, PAGE_SIZE as size_t,
                               attrs, vaddr_base, top_size_shift,
                               top_index_shift, page_size_shift);
            if ret < 0 {
                undo = true;
                break;
            }

            v += PAGE_SIZE as vaddr_t;
            total_mapped += (ret / PAGE_SIZE as isize) as size_t;
            idx += 1;
        }

        if undo && idx > 0 {
            let _ = self.unmap_pages(vaddr, idx * (PAGE_SIZE as size_t), vaddr_base, top_size_shift,
                                   top_index_shift, page_size_shift);
            return RX_ERR_INTERNAL;
        }
        
        debug_assert!(total_mapped <= count);

        if let Some(mapped_val) = mapped {
            *mapped_val = total_mapped;
        }

        RX_OK
    }

    pub fn unmap(&mut self, vaddr: vaddr_t, count: size_t, unmapped: Option<&mut size_t>) -> rx_status_t {
        self.canary.assert();
        ltrace!("vaddr {:#x} count {}\n", vaddr, count);

        debug_assert!(!self.tt_virt.is_null());

        debug_assert!(self.is_valid_vaddr(vaddr));

        if !self.is_valid_vaddr(vaddr) {
            return RX_ERR_OUT_OF_RANGE;
        }

        debug_assert!(is_page_aligned(vaddr as u64));
        if !is_page_aligned(vaddr as u64) {
            return RX_ERR_INVALID_ARGS;
        }

        let mut vaddr_base: vaddr_t = 0;
        let mut top_size_shift: u32 = 0;
        let mut top_index_shift: u32 = 0;
        let mut page_size_shift: u32 = 0;

        {
            let _guard = self.lock.lock();
            self.mmu_params_from_flags(0, None, &mut vaddr_base, &mut top_size_shift,
                                     &mut top_index_shift, &mut page_size_shift);
        }
        // Guard is dropped here

        let ret = self.unmap_pages(vaddr, count * (PAGE_SIZE as size_t),
                                     vaddr_base, top_size_shift,
                                     top_index_shift, page_size_shift);

        if let Some(unmapped_val) = unmapped {
            *unmapped_val = if ret > 0 { (ret / PAGE_SIZE as isize) as size_t } else { 0 };
            debug_assert!(*unmapped_val <= count);
        }

        if ret < 0 { ret as rx_status_t } else { 0 }
    }

    pub fn protect(&mut self, vaddr: vaddr_t, count: size_t, mmu_flags: u32) -> rx_status_t {
        self.canary.assert();

        if !self.is_valid_vaddr(vaddr) {
            return RX_ERR_INVALID_ARGS;
        }

        if !is_page_aligned(vaddr as u64) {
            return RX_ERR_INVALID_ARGS;
        }

        if (mmu_flags & ARCH_MMU_FLAG_PERM_READ) == 0 {
            return RX_ERR_INVALID_ARGS;
        }

        let mut attrs: pte_t = 0;
        let mut vaddr_base: vaddr_t = 0;
        let mut top_size_shift: u32 = 0;
        let mut top_index_shift: u32 = 0;
        let mut page_size_shift: u32 = 0;

        {
            let _guard = self.lock.lock();
            self.mmu_params_from_flags(mmu_flags, Some(&mut attrs), &mut vaddr_base, &mut top_size_shift,
                                     &mut top_index_shift, &mut page_size_shift);
        }
        // Guard is dropped here

        let ret = self.protect_pages(vaddr, count * (PAGE_SIZE as size_t),
                                   attrs, vaddr_base,
                                   top_size_shift, top_index_shift, page_size_shift);

        ret
    }

    pub fn init_with_flags(&mut self, base: vaddr_t, size: size_t, flags: u32) -> rx_status_t {
        self.canary.assert();
        ltrace!("aspace {:p}, base {:#x}, size {:#x}, flags {:#x}\n",
                self, base, size, flags);

        let _guard = self.lock.lock();

        // Validate that the base + size is sane and doesn't wrap.
        debug_assert!(size > PAGE_SIZE as size_t);
        debug_assert!((base as u64) + (size as u64) - 1 > (base as u64));

        self.flags = flags;
        if flags & ARCH_ASPACE_FLAG_KERNEL != 0 {
            // At the moment we can only deal with address spaces as globally defined.
            debug_assert!(base == (!0u64 << MMU_KERNEL_SIZE_SHIFT) as vaddr_t);
            debug_assert!(size == (1u64 << MMU_KERNEL_SIZE_SHIFT) as size_t);

            self.base = base;
            self.size = size;
            self.tt_virt = unsafe { arm64_kernel_translation_table.data.as_mut_ptr() };
            self.tt_phys = vaddr_to_paddr(self.tt_virt as u64);
            self.asid = MMU_ARM64_GLOBAL_ASID as u16;
        } else {
            if flags & ARCH_ASPACE_FLAG_GUEST != 0 {
                debug_assert!((base as u64) + (size as u64) <= 1u64 << MMU_GUEST_SIZE_SHIFT);
            } else {
                debug_assert!((base as u64) + (size as u64) <= 1u64 << MMU_USER_SIZE_SHIFT);
                if unsafe { get_asid_allocator().alloc() } != RX_OK {
                    return RX_ERR_NO_MEMORY;
                }
            }

            self.base = base;
            self.size = size;

            let mut pa: paddr_t = 0;
            let (page, paddr) = match pmm_alloc_page(0) {
                Ok((page, paddr)) => (page, paddr),
                Err(_) => return RX_ERR_NO_MEMORY,
            };
            if page.is_null() {
                return RX_ERR_NO_MEMORY;
            }
            pa = paddr;
            // TODO: Set page state when VM page management is implemented
            // unsafe { (*page).state = VM_PAGE_STATE_MMU; }

            // Use phys_to_virt to convert physical address to virtual
            let va = crate::vm::phys_to_virt(pa) as *mut pte_t;

            self.tt_virt = va;
            self.tt_phys = pa;

            // zero the top level translation table.
            // XXX remove when PMM starts returning pre-zeroed pages.
            arch_zero_page(self.tt_virt as *mut c_void);
        }

        RX_OK
    }
} // End of impl ArmArchVmAspace

pub fn arch_zero_page(ptr: *mut c_void) {
    let mut ptr_val = ptr as usize;
    let zva_size = unsafe { arm64_zva_size };
    let end_ptr = ptr_val + (PAGE_SIZE as usize);

    while ptr_val != end_ptr {
        unsafe {
            asm!("dc zva, {}", in(reg) ptr_val);
        }
        ptr_val += zva_size as usize;
    }
}

pub fn arm64_mmu_translate(va: vaddr_t, pa: &mut paddr_t, user: bool, write: bool) -> rx_status_t {
    // disable interrupts around this operation to make the at/par instruction combination atomic
    let state = arch_interrupt_save(ARCH_DEFAULT_SPIN_LOCK_FLAG_INTERRUPTS);

    unsafe {
        if user {
            if write {
                asm!("at s1e0w, {}", in(reg) va, options(preserves_flags, nostack));
            } else {
                asm!("at s1e0r, {}", in(reg) va, options(preserves_flags, nostack));
            }
        } else {
            if write {
                asm!("at s1e1w, {}", in(reg) va, options(preserves_flags, nostack));
            } else {
                asm!("at s1e1r, {}", in(reg) va, options(preserves_flags, nostack));
            }
        }

        let par: u64;
        asm!("mrs {}, par_el1", out(reg) par);

        arch_interrupt_restore(state, ARCH_DEFAULT_SPIN_LOCK_FLAG_INTERRUPTS);

        // if bit 0 is clear, the translation succeeded
        if bit!(par, 0) != 0 {
            return RX_ERR_NO_MEMORY;
        }

        // physical address is stored in bits [51..12], naturally aligned
        *pa = bits!(par, 51, 12) | ((va as u64) & (PAGE_SIZE - 1));
    }

    RX_OK
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Create VTTBR value from ASID and page table physical address
pub fn arm64_vttbr(asid: u16, tt_phys: paddr_t) -> u64 {
    ((asid as u64) << 48) | (tt_phys & 0x0000_FFFF_FFFF_F000)
}

/// Invalidate all TLB entries
pub fn tlb_invalidate_all() {
    unsafe {
        core::arch::asm!("tlbi vmalle1is");
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
    }
}

/// Invalidate TLB entries for a specific virtual address
pub fn tlb_invalidate_va(va: vaddr_t) {
    unsafe {
        core::arch::asm!("tlbi vae1is, {}", in(reg) (va >> 12));
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
    }
}

/// Invalidate TLB entries for a specific ASID
pub fn tlb_invalidate_all_asid(asid: u16) {
    unsafe {
        core::arch::asm!("tlbi aside1is, {}", in(reg) asid);
        core::arch::asm!("dsb ish");
        core::arch::asm!("isb");
    }
}

/// Set TTBR1_EL1 register
pub fn set_ttbr1_el1(ttbr: u64) {
    unsafe {
        core::arch::asm!("msr ttbr1_el1, {}", in(reg) ttbr);
        core::arch::asm!("isb");
    }
}
// ============================================================================
// Page Table Types
// ============================================================================

// Import VM types for proper error handling
use crate::kernel::vm::{VmError, Result as VmResult};
use crate::rustux::types::Result as RxResult;

/// ARM64 page table structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ArmPageTable {
    /// Page table entries
    pub entries: [pte_t; 512],
}

impl Default for ArmPageTable {
    fn default() -> Self {
        Self { entries: [0; 512] }
    }
}

impl ArmPageTable {
    /// Create a new empty page table
    pub fn new() -> Self {
        Self { entries: [0; 512] }
    }

    /// Create a new kernel page table with kernel mappings
    pub fn new_kernel() -> VmResult<Self> {
        // TODO: Implement kernel page table initialization
        Ok(Self { entries: [0; 512] })
    }

    /// Map a page in the page table
    pub fn map(&mut self, vaddr: usize, paddr: usize, flags: PageTableFlags) -> VmResult<()> {
        // Convert PageTableFlags to u64 for internal use
        let flags_bits = flags.bits();
        // TODO: Implement map using flags_bits
        Ok(())
    }

    /// Unmap a page from the page table
    pub fn unmap(&mut self, vaddr: usize) -> VmResult<()> {
        // TODO: Implement unmap
        Ok(())
    }

    /// Change protection flags for a page
    pub fn protect(&mut self, vaddr: usize, flags: PageTableFlags) -> VmResult<()> {
        // Convert PageTableFlags to u64 for internal use
        let flags_bits = flags.bits();
        // TODO: Implement protect using flags_bits
        Ok(())
    }

    /// Resolve a virtual address to physical address
    pub fn resolve(&self, vaddr: usize) -> Option<usize> {
        // TODO: Implement address resolution
        None
    }

    /// Flush TLB entries
    pub fn flush_tlb(&mut self, addr: Option<usize>) {
        // TODO: Implement TLB flush for specific address or all
        unsafe {
            if let Some(a) = addr {
                // Flush specific address
                core::arch::asm!("tlbi vaae1is, {}", in(reg) (a >> 12));
            } else {
                // Flush all
                core::arch::asm!("tlbi vmalle1is");
            }
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }
    }

    /// Get the root physical address of the page table
    pub fn root_phys(&self) -> usize {
        // TODO: Return actual physical address
        0
    }

    /// Get pointer to entries
    pub fn as_ptr(&mut self) -> *mut pte_t {
        self.entries.as_mut_ptr()
    }
}

/// Implement ArchPageTable trait for ArmPageTable
impl crate::vm::ArchPageTable for ArmPageTable {
    type Entry = crate::vm::page_table::GenericEntry;

    fn new() -> crate::vm::Result<Self> {
        Ok(Self::new())
    }

    fn map(&mut self, vaddr: crate::vm::VAddr, paddr: crate::vm::PAddr, flags: crate::vm::page_table::PageTableFlags) -> crate::vm::Result {
        self.map(vaddr, paddr, flags)
    }

    fn unmap(&mut self, vaddr: crate::vm::VAddr) -> crate::vm::Result {
        self.unmap(vaddr)
    }

    fn resolve(&self, vaddr: crate::vm::VAddr) -> Option<crate::vm::PAddr> {
        self.resolve(vaddr)
    }

    fn protect(&mut self, vaddr: crate::vm::VAddr, flags: crate::vm::page_table::PageTableFlags) -> crate::vm::Result {
        self.protect(vaddr, flags)
    }

    fn flush_tlb(&self, vaddr: Option<crate::vm::VAddr>) {
        // Create a mutable self reference for the flush
        // This is a bit awkward because flush_tlb takes &mut self but trait takes &self
        // For now, we'll just call the TLBI instructions inline here
        unsafe {
            if let Some(a) = vaddr {
                // Flush specific address
                core::arch::asm!("tlbi vaae1is, {}", in(reg) (a >> 12));
            } else {
                // Flush all
                core::arch::asm!("tlbi vmalle1is");
            }
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
        }
    }

    fn root_phys(&self) -> crate::vm::PAddr {
        self.root_phys()
    }
}

/// Initialize ARM64 MMU
pub fn arm64_mmu_init() {
    // TODO: Implement MMU initialization
}
