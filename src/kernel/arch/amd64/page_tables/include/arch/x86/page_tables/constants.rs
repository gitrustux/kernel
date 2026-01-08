// Rustux Authors 2025
//! This file contains various x86 MMU-related constants and macros in Rustux.


const X86_MMU_PG_P: u64 = 0x0001;   // P Valid
const X86_MMU_PG_RW: u64 = 0x0002;  // R/W Read/Write
const X86_MMU_PG_U: u64 = 0x0004;   // U/S User/Supervisor
const X86_MMU_PG_WT: u64 = 0x0008;  // WT Write-through
const X86_MMU_PG_CD: u64 = 0x0010;  // CD Cache disable
const X86_MMU_PG_A: u64 = 0x0020;   // A Accessed
const X86_MMU_PG_D: u64 = 0x0040;   // D Dirty
const X86_MMU_PG_PS: u64 = 0x0080;  // PS Page size (0=4k,1=4M)
const X86_MMU_PG_PTE_PAT: u64 = 0x0080; // PAT PAT index for 4k pages
const X86_MMU_PG_LARGE_PAT: u64 = 0x1000; // PAT PAT index otherwise
const X86_MMU_PG_G: u64 = 0x0100;   // G Global
const X86_DIRTY_ACCESS_MASK: u64 = 0xf9f;

// Macros for converting from PAT index to appropriate page table flags
macro_rules! common_selector {
    ($x:expr) => {
        (($x & 0x2) * X86_MMU_PG_CD) | (($x & 0x1) * X86_MMU_PG_WT)
    };
}

macro_rules! pte_selector {
    ($x:expr) => {
        (($x & 0x4) * X86_MMU_PG_PTE_PAT) | common_selector!($x)
    };
}

macro_rules! large_selector {
    ($x:expr) => {
        (($x & 0x4) * X86_MMU_PG_LARGE_PAT) | common_selector!($x)
    };
}

const X86_MMU_PTE_PAT_MASK: u64 = pte_selector!(0x7);
const X86_MMU_LARGE_PAT_MASK: u64 = large_selector!(0x7);

// Physical memory is mapped at the base of the kernel address space
const KERNEL_ASPACE_BASE: usize = 0xFFFF800000000000; // Example value, should be defined based on target architecture
#[inline]
pub fn x86_phys_to_virt(x: usize) -> usize {
    x + KERNEL_ASPACE_BASE
}

#[inline]
pub fn x86_virt_to_phys(x: usize) -> usize {
    x - KERNEL_ASPACE_BASE
}

#[inline]
pub fn is_page_present(pte: u64) -> bool {
    pte & X86_MMU_PG_P != 0
}

#[inline]
pub fn is_large_page(pte: u64) -> bool {
    pte & X86_MMU_PG_PS != 0
}

// Address shifts and offsets for page tables
const PML4_SHIFT: usize = 39;
const PDP_SHIFT: usize = 30;
const PD_SHIFT: usize = 21;
const PT_SHIFT: usize = 12;
const ADDR_OFFSET: usize = 9;
const PDPT_ADDR_OFFSET: usize = 2;
const NO_OF_PT_ENTRIES: usize = 512;

const X86_FLAGS_MASK: u64 = 0x8000000000000fffu64;
const X86_LARGE_FLAGS_MASK: u64 = 0x8000000000001fffu64;
const X86_PDPT_ADDR_MASK: u64 = 0x00000000ffffffe0u64;
const X86_HUGE_PAGE_FRAME: u64 = 0x000fffffc0000000u64;
const X86_LARGE_PAGE_FRAME: u64 = 0x000fffffffe00000u64;
const X86_PG_FRAME: u64 = 0x000ffffffffff000u64;
const PAGE_OFFSET_MASK_4KB: usize = (1 << PT_SHIFT) - 1;
const PAGE_OFFSET_MASK_LARGE: usize = (1 << PD_SHIFT) - 1;
const PAGE_OFFSET_MASK_HUGE: usize = (1 << PDP_SHIFT) - 1;

// Macros for address calculation
#[inline]
pub fn vaddr_to_pml4_index(vaddr: usize) -> usize {
    (vaddr >> PML4_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

#[inline]
pub fn vaddr_to_pdp_index(vaddr: usize) -> usize {
    (vaddr >> PDP_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

#[inline]
pub fn vaddr_to_pd_index(vaddr: usize) -> usize {
    (vaddr >> PD_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

#[inline]
pub fn vaddr_to_pt_index(vaddr: usize) -> usize {
    (vaddr >> PT_SHIFT) & ((1 << ADDR_OFFSET) - 1)
}

// Architecture-specific kernel constants
pub const KERNEL_BASE: u64 = 0xFFFF_8000_0000_0000;
pub const KERNEL_LOAD_OFFSET: u64 = 0;
pub const PHYSMAP_BASE: u64 = 0xFFFF_8800_0000_0000;

// PAT (Page Attribute Table) selector functions
/// Get PTE PAT selector value for a given PAT index
#[inline]
pub const fn x86_pat_pte_selector(pat_index: u64) -> u64 {
    ((pat_index & 0x4) * X86_MMU_PG_PTE_PAT) | common_selector!(pat_index)
}

/// Get LARGE page PAT selector value for a given PAT index
#[inline]
pub const fn x86_pat_large_selector(pat_index: u64) -> u64 {
    ((pat_index & 0x4) * X86_MMU_PG_LARGE_PAT) | common_selector!(pat_index)
}

/// Compatibility alias for PTE selector
pub use x86_pat_pte_selector as X86_PAT_PTE_SELECTOR;

/// Compatibility alias for LARGE selector
pub use x86_pat_large_selector as X86_PAT_LARGE_SELECTOR;
