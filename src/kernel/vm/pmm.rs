// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Physical Memory Manager
//!
//! This module provides physical page allocation and management.
//! It manages arenas of physical memory and tracks page states.

#![no_std]

extern crate alloc;

use alloc::collections::LinkedList;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use spin::Mutex;

use crate::rustux::types::*;
use super::{Result, VmError};

/// Page size in bytes (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Page size shift (log2(PAGE_SIZE))
pub const PAGE_SIZE_SHIFT: u8 = 12;

/// Maximum number of arenas
pub const MAX_ARENAS: usize = 16;

/// Arena flag for low memory
pub const ARENA_FLAG_LO_MEM: u32 = 0x1;

/// Allocation flags
pub const ALLOC_FLAG_ANY: u32 = 0x0;
pub const ALLOC_FLAG_LO_MEM: u32 = 0x1;

/// Page state enumeration
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmPageState {
    Free = 0,
    Alloc = 1,
    Object = 2,
    Wired = 3,
    Heap = 4,
    Mmu = 5,
    Iommu = 6,
    Ipc = 7,
}

impl VmPageState {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => VmPageState::Free,
            1 => VmPageState::Alloc,
            2 => VmPageState::Object,
            3 => VmPageState::Wired,
            4 => VmPageState::Heap,
            5 => VmPageState::Mmu,
            6 => VmPageState::Iommu,
            7 => VmPageState::Ipc,
            _ => VmPageState::Free,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            VmPageState::Free => "free",
            VmPageState::Alloc => "alloc",
            VmPageState::Object => "object",
            VmPageState::Wired => "wired",
            VmPageState::Heap => "heap",
            VmPageState::Mmu => "mmu",
            VmPageState::Iommu => "iommu",
            VmPageState::Ipc => "ipc",
        }
    }
}

/// Convert page state to string
pub fn page_state_to_string(state: u32) -> &'static str {
    VmPageState::from_u8(state as u8).as_str()
}

/// VM page structure
#[repr(C)]
#[derive(Debug)]
pub struct VmPage {
    /// Physical address of this page
    pub paddr: PAddr,
    /// Current page state
    pub state: VmPageState,
    /// Page flags
    pub flags: u8,
    /// Pin count (for object pages)
    pub pin_count: u8,
}

impl VmPage {
    /// Create a new VM page
    pub fn new(paddr: PAddr) -> Self {
        Self {
            paddr,
            state: VmPageState::Free,
            flags: 0,
            pin_count: 0,
        }
    }

    /// Check if page is free
    pub fn is_free(&self) -> bool {
        self.state == VmPageState::Free
    }

    /// Dump page information
    pub fn dump(&self) {
        println!(
            "page {:p}: address {:#x} state {} flags {:#x}",
            self, self.paddr,
            self.state.as_str(),
            self.flags
        );
    }
}

/// Arena information structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PmmArenaInfo {
    /// Arena name
    pub name: [u8; 16],
    /// Arena flags
    pub flags: u32,
    /// Arena priority (higher = allocate from first)
    pub priority: u32,
    /// Base physical address
    pub base: PAddr,
    /// Size in bytes
    pub size: usize,
}

impl PmmArenaInfo {
    /// Create a new arena info structure
    pub fn new(name: &str, base: PAddr, size: usize, priority: u32, flags: u32) -> Self {
        let mut name_array = [0u8; 16];
        for (i, b) in name.bytes().enumerate() {
            if i < 16 {
                name_array[i] = b;
            }
        }

        Self {
            name: name_array,
            flags,
            priority,
            base,
            size,
        }
    }

    /// Get the arena name as a string slice
    pub fn name_str(&self) -> &str {
        // Find null terminator
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(16);
        core::str::from_utf8(&self.name[..len]).unwrap_or("???")
    }
}

/// Physical Memory Arena
///
/// Represents a contiguous region of physical memory
/// managed by the PMM.
pub struct PmmArena {
    /// Arena information
    info: PmmArenaInfo,
    /// Array of page structures
    pages: Vec<VmPage>,
    /// Free list (indices into pages array)
    free_list: LinkedList<usize>,
}

impl PmmArena {
    /// Create a new PMM arena
    pub fn new(info: PmmArenaInfo) -> Result<Self> {
        // Validate alignment
        if info.base & (PAGE_SIZE as PAddr - 1) != 0 {
            return Err(VmError::InvalidArgs);
        }
        if info.size & (PAGE_SIZE - 1) != 0 {
            return Err(VmError::AlignmentError);
        }

        let page_count = info.size / PAGE_SIZE;

        // Allocate page array (for now, use heap - in real implementation this would be boot alloc)
        let mut pages = Vec::with_capacity(page_count);

        // Initialize pages
        for i in 0..page_count {
            pages.push(VmPage::new(info.base + (i * PAGE_SIZE) as PAddr));
        }

        Ok(Self {
            info,
            pages,
            free_list: LinkedList::new(),
        })
    }

    /// Get the arena base address
    pub fn base(&self) -> PAddr {
        self.info.base
    }

    /// Get the arena size
    pub fn size(&self) -> usize {
        self.info.size
    }

    /// Get the arena priority
    pub fn priority(&self) -> u32 {
        self.info.priority
    }

    /// Get the arena flags
    pub fn flags(&self) -> u32 {
        self.info.flags
    }

    /// Get the arena name
    pub fn name(&self) -> &str {
        self.info.name_str()
    }

    /// Check if a physical address is within this arena
    pub fn address_in_arena(&self, addr: PAddr) -> bool {
        addr >= self.info.base && addr < (self.info.base + self.info.size as PAddr)
    }

    /// Find a specific page by physical address
    pub fn find_page(&self, pa: PAddr) -> Option<&VmPage> {
        if !self.address_in_arena(pa) {
            return None;
        }

        let index = ((pa - self.info.base) / PAGE_SIZE as PAddr) as usize;
        self.pages.get(index)
    }

    /// Find a specific page by physical address (mutable)
    pub fn find_page_mut(&mut self, pa: PAddr) -> Option<&mut VmPage> {
        if !self.address_in_arena(pa) {
            return None;
        }

        let index = ((pa - self.info.base) / PAGE_SIZE as PAddr) as usize;
        self.pages.get_mut(index)
    }

    /// Allocate a single page from this arena
    pub fn alloc_page(&mut self) -> Option<*mut VmPage> {
        // Find first free page
        for (i, page) in self.pages.iter_mut().enumerate() {
            if page.is_free() {
                page.state = VmPageState::Alloc;
                return Some(page as *mut VmPage);
            }
        }
        None
    }

    /// Free a page back to this arena
    pub fn free_page(&mut self, pa: PAddr) -> Result<()> {
        let page = self.find_page_mut(pa).ok_or(VmError::NotFound)?;
        page.state = VmPageState::Free;
        Ok(())
    }

    /// Count free pages in this arena
    pub fn count_free_pages(&self) -> usize {
        self.pages.iter().filter(|p| p.is_free()).count()
    }

    /// Count total bytes in this arena
    pub fn count_total_bytes(&self) -> u64 {
        self.info.size as u64
    }

    /// Count pages by state
    pub fn count_states(&self) -> [usize; 8] {
        let mut counts = [0usize; 8];
        for page in &self.pages {
            counts[page.state as usize] += 1;
        }
        counts
    }

    /// Dump arena information
    pub fn dump(&self, detailed: bool) {
        println!(
            "arena: '{}' base {:#x} size {:#x} priority {} flags {:#x}",
            self.name(),
            self.base(),
            self.size(),
            self.priority(),
            self.flags()
        );

        if detailed {
            let state_counts = self.count_states();
            println!("  page states:");
            for (i, count) in state_counts.iter().enumerate() {
                if *count > 0 {
                    println!("    {}: {} pages ({} bytes)",
                        VmPageState::from_u8(i as u8).as_str(),
                        count,
                        count * PAGE_SIZE
                    );
                }
            }
        }
    }
}

/// Physical Memory Manager Node
///
/// Manages multiple arenas of physical memory.
pub struct PmmNode {
    /// List of arenas
    arenas: Vec<PmmArena>,
    /// Total free pages across all arenas
    free_count: AtomicU64,
    /// Total bytes managed
    total_bytes: AtomicU64,
}

impl Default for PmmNode {
    fn default() -> Self {
        Self::new()
    }
}

impl PmmNode {
    /// Create a new PMM node
    pub fn new() -> Self {
        Self {
            arenas: Vec::new(),
            free_count: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
        }
    }

    /// Add an arena to the PMM
    pub fn add_arena(&mut self, info: PmmArenaInfo) -> Result<()> {
        let arena = PmmArena::new(info)?;

        // Count free pages in the new arena
        let free_pages = arena.count_free_pages();
        let arena_priority = arena.priority();
        self.free_count.fetch_add(free_pages as u64, Ordering::Release);
        self.total_bytes.fetch_add(arena.count_total_bytes(), Ordering::Release);

        // Find insertion point based on priority (higher priority first)
        let insert_pos = self.arenas.iter().enumerate()
            .find(|(_, a)| arena_priority > a.priority())
            .map(|(i, _)| i);

        // Insert at the found position or append to the end
        match insert_pos {
            Some(i) => self.arenas.insert(i, arena),
            None => self.arenas.push(arena),
        }

        Ok(())
    }

    /// Allocate a single page
    pub fn alloc_page(&mut self, flags: u32) -> Result<(*mut VmPage, PAddr)> {
        for arena in &mut self.arenas {
            // Check arena flags against allocation flags
            if flags & ALLOC_FLAG_LO_MEM != 0 && arena.flags() & ARENA_FLAG_LO_MEM == 0 {
                continue;
            }

            if let Some(page_ptr) = arena.alloc_page() {
                let pa = unsafe { (*page_ptr).paddr };
                self.free_count.fetch_sub(1, Ordering::Release);
                return Ok((page_ptr, pa));
            }
        }

        Err(VmError::NoMemory)
    }

    /// Allocate multiple pages
    pub fn alloc_pages(&mut self, count: usize, flags: u32) -> Result<Vec<*mut VmPage>> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut pages = Vec::with_capacity(count);

        for _ in 0..count {
            match self.alloc_page(flags) {
                Ok((page_ptr, _pa)) => pages.push(page_ptr),
                Err(e) => {
                    // Free already allocated pages
                    for &page_ptr in &pages {
                        unsafe {
                            let pa = (*page_ptr).paddr;
                            self.free_page_internal(pa);
                        }
                    }
                    return Err(e);
                }
            }
        }

        Ok(pages)
    }

    /// Free a page
    pub fn free_page(&mut self, pa: PAddr) -> Result<()> {
        self.free_count.fetch_add(1, Ordering::Release);
        self.free_page_internal(pa)
    }

    /// Internal free page implementation
    fn free_page_internal(&mut self, pa: PAddr) -> Result<()> {
        for arena in &mut self.arenas {
            if arena.address_in_arena(pa) {
                return arena.free_page(pa);
            }
        }
        Err(VmError::NotFound)
    }

    /// Count free pages across all arenas
    pub fn count_free_pages(&self) -> u64 {
        self.free_count.load(Ordering::Acquire)
    }

    /// Count total bytes managed
    pub fn count_total_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Acquire)
    }

    /// Count pages by state across all arenas
    pub fn count_total_states(&self) -> [usize; 8] {
        let mut total = [0usize; 8];
        for arena in &self.arenas {
            let counts = arena.count_states();
            for (i, &count) in counts.iter().enumerate() {
                total[i] += count;
            }
        }
        total
    }

    /// Find page by physical address
    pub fn find_page(&self, pa: PAddr) -> Option<&VmPage> {
        for arena in &self.arenas {
            if let Some(page) = arena.find_page(pa) {
                return Some(page);
            }
        }
        None
    }

    /// Dump PMM information
    pub fn dump(&self, detailed: bool) {
        println!("PMM Node: {} arenas", self.arenas.len());
        println!("  Free pages: {}", self.count_free_pages());
        println!("  Total bytes: {}", self.count_total_bytes());

        for arena in &self.arenas {
            arena.dump(detailed);
        }
    }

    /// Dump free page information
    pub fn dump_free(&self) {
        println!("PMM: {} free pages / {} total bytes",
            self.count_free_pages(),
            self.count_total_bytes()
        );
    }
}

/// Global PMM node instance
/// Note: Using unsafe initialization for static - should be properly initialized at boot
static mut PMM_NODE_UNINIT: MaybeUninit<Mutex<PmmNode>> = MaybeUninit::uninit();
static PMM_INIT: AtomicBool = AtomicBool::new(false);

/// Get the global PMM node
fn get_pmm() -> &'static Mutex<PmmNode> {
    unsafe {
        if !PMM_INIT.load(Ordering::Acquire) {
            PMM_NODE_UNINIT.write(Mutex::new(PmmNode::new()));
            PMM_INIT.store(true, Ordering::Release);
        }
        PMM_NODE_UNINIT.assume_init_ref()
    }
}

/// Add an arena to the PMM
pub fn pmm_add_arena(info: &PmmArenaInfo) -> Result<()> {
    get_pmm().lock().add_arena(*info)
}

/// Allocate a single page
pub fn pmm_alloc_page(flags: u32) -> Result<(*mut VmPage, PAddr)> {
    get_pmm().lock().alloc_page(flags)
}

/// Allocate multiple pages
pub fn pmm_alloc_pages(count: usize, flags: u32) -> Result<Vec<*mut VmPage>> {
    get_pmm().lock().alloc_pages(count, flags)
}

/// Free a page
pub fn pmm_free_page(pa: PAddr) -> Result<()> {
    get_pmm().lock().free_page(pa)
}

/// Count free pages
pub fn pmm_count_free_pages() -> u64 {
    get_pmm().lock().count_free_pages()
}

/// Count total bytes
pub fn pmm_count_total_bytes() -> u64 {
    get_pmm().lock().count_total_bytes()
}

/// Count pages by state
pub fn pmm_count_total_states() -> [usize; 8] {
    get_pmm().lock().count_total_states()
}

/// Find page by physical address
/// Note: This returns a copy of the page data, not a reference, to avoid lifetime issues
pub fn paddr_to_vm_page(pa: PAddr) -> Option<VmPage> {
    get_pmm().lock().find_page(pa).map(|p| VmPage {
        paddr: p.paddr,
        state: p.state,
        flags: p.flags,
        pin_count: p.pin_count,
    })
}

/// Dump PMM information
pub fn pmm_dump(detailed: bool) {
    get_pmm().lock().dump(detailed)
}

/// Dump free page information
pub fn pmm_dump_free() {
    get_pmm().lock().dump_free()
}

/// Initialize the PMM
pub fn pmm_init() {
    // PMM initialization happens via pmm_add_arena calls during boot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_state_to_string() {
        assert_eq!(page_state_to_string(0), "free");
        assert_eq!(page_state_to_string(1), "alloc");
        assert_eq!(page_state_to_string(3), "wired");
    }

    #[test]
    fn test_arena_info() {
        let info = PmmArenaInfo::new("test_arena", 0x1000000, 0x100000, 10, 0);
        assert_eq!(info.name_str(), "test_arena");
        assert_eq!(info.base, 0x1000000);
        assert_eq!(info.size, 0x100000);
        assert_eq!(info.priority, 10);
    }

    #[test]
    fn test_vm_page() {
        let page = VmPage::new(0x1000);
        assert_eq!(page.paddr, 0x1000);
        assert!(page.is_free());
        assert_eq!(page.state, VmPageState::Free);
    }

    #[test]
    fn test_pmm_arena() {
        let info = PmmArenaInfo::new("test", 0x10000000, 0x100000, 10, 0);
        let arena = PmmArena::new(info).unwrap();

        assert_eq!(arena.base(), 0x10000000);
        assert_eq!(arena.size(), 0x100000);
        assert!(arena.address_in_arena(0x10000000));
        assert!(!arena.address_in_arena(0x20000000));
    }

    #[test]
    fn test_pmm_node() {
        let mut node = PmmNode::new();

        let info1 = PmmArenaInfo::new("arena1", 0x10000000, 0x100000, 10, 0);
        let info2 = PmmArenaInfo::new("arena2", 0x20000000, 0x100000, 5, 0);

        assert!(node.add_arena(info1).is_ok());
        assert!(node.add_arena(info2).is_ok());

        // Higher priority arena should be first
        assert_eq!(node.arenas[0].name(), "arena1");
        assert_eq!(node.arenas[1].name(), "arena2");
    }
}
