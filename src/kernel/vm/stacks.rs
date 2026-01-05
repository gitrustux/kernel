// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Per-Thread Kernel Stacks
//!
//! This module manages kernel stack allocation and guard pages for threads.
//! Each thread gets a dedicated kernel stack with overflow protection.
//!
//! # Design
//!
//! - Each thread has a dedicated kernel stack
//! - Guard page below stack detects overflow
//! - Stack lives in kernel VA space
//! - SP stored in thread context
//! - Stacks are aligned to 16-byte boundary (ARM64/x86-64 requirement)
//!
//! # Stack Layout
//!
//! ```text
//! +------------------+ <- stack_top (initial SP)
//! |                  |
//! |   Stack Data     |
//! |     (grows      |
//! |     downward)    |
//! |                  |
//! +------------------+
//! |   Guard Page     | <- (unmapped, causes fault on access)
//! +------------------+
//! ```

#![no_std]

use crate::kernel::vm::layout::*;
use crate::kernel::vm::aspace::*;
use crate::kernel::vm::{VmError, Result};
use crate::kernel::pmm;
use crate::kernel::sync::Mutex;
use core::sync::atomic::{AtomicU64, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Kernel Stack Configuration
/// ============================================================================

/// Default kernel stack size (64KB)
pub const DEFAULT_KERNEL_STACK_SIZE: usize = 64 * 1024;

/// Minimum kernel stack size
pub const MIN_KERNEL_STACK_SIZE: usize = 16 * 1024;

/// Maximum kernel stack size
pub const MAX_KERNEL_STACK_SIZE: usize = 256 * 1024;

/// Stack alignment requirement (16 bytes for ABI)
pub const STACK_ALIGN: usize = 16;

/// Number of guard pages
pub const GUARD_PAGES: usize = 1;

/// ============================================================================
/// Kernel Stack Descriptor
/// ============================================================================

/// Kernel stack information
#[repr(C)]
#[derive(Debug)]
pub struct KernelStack {
    /// Virtual address of the stack top (initial SP)
    pub top: VAddr,

    /// Virtual address of the stack base
    pub base: VAddr,

    /// Stack size in bytes
    pub size: usize,

    /// Physical address of stack pages
    pub phys_base: PAddr,

    /// Number of pages
    pub num_pages: usize,

    /// Stack guard virtual address
    pub guard_vaddr: VAddr,

    /// Thread ID that owns this stack
    pub owner_id: u64,
}

impl KernelStack {
    /// Create a new kernel stack
    fn new(
        top: VAddr,
        base: VAddr,
        size: usize,
        phys_base: PAddr,
        num_pages: usize,
        guard_vaddr: VAddr,
        owner_id: u64,
    ) -> Self {
        Self {
            top,
            base,
            size,
            phys_base,
            num_pages,
            guard_vaddr,
            owner_id,
        }
    }

    /// Get the bottom of the usable stack (just above guard)
    pub fn bottom(&self) -> VAddr {
        self.guard_vaddr + PAGE_SIZE
    }

    /// Get the current stack pointer location
    pub fn current_sp(&self) -> VAddr {
        self.top // Initially at top
    }

    /// Check if an address is within this stack
    pub fn contains(&self, vaddr: VAddr) -> bool {
        vaddr >= self.bottom() && vaddr < self.top
    }

    /// Check if an address is the guard page
    pub fn is_guard_page(&self, vaddr: VAddr) -> bool {
        vaddr >= self.guard_vaddr && vaddr < self.guard_vaddr + PAGE_SIZE
    }
}

/// ============================================================================
/// Stack Allocator
/// ============================================================================

/// Stack allocator statistics
#[repr(C)]
#[derive(Debug)]
pub struct StackStats {
    /// Total stacks allocated
    pub total_stacks: u64,

    /// Currently active stacks
    pub active_stacks: u64,

    /// Total memory used for stacks
    pub total_memory: u64,

    /// Number of stack overflows detected
    pub overflows: u64,
}

/// Stack allocator for kernel stacks
pub struct StackAllocator {
    /// Base virtual address for stacks
    base_vaddr: VAddr,

    /// Current offset from base
    next_offset: AtomicU64,

    /// Total size of stack region
    total_size: usize,

    /// Statistics
    stats: Mutex<StackStats>,
}

impl StackAllocator {
    /// Create a new stack allocator
    pub const fn new(base_vaddr: VAddr, total_size: usize) -> Self {
        Self {
            base_vaddr,
            next_offset: AtomicU64::new(0),
            total_size,
            stats: Mutex::new(StackStats {
                total_stacks: 0,
                active_stacks: 0,
                total_memory: 0,
                overflows: 0,
            }),
        }
    }

    /// Allocate a kernel stack
    pub fn alloc_stack(&self, owner_id: u64, size: usize) -> Result<KernelStack> {
        // Validate stack size
        if size < MIN_KERNEL_STACK_SIZE || size > MAX_KERNEL_STACK_SIZE {
            return Err(VmError::InvalidArgs);
        }

        // Align to page size
        let total_size = page_align_up(size) + (GUARD_PAGES * PAGE_SIZE);

        // Allocate from the stack region
        let offset = self.next_offset.fetch_add(total_size as u64, Ordering::Relaxed) as usize;

        // Check if we have enough space
        if offset + total_size > self.total_size {
            // Rollback
            self.next_offset.fetch_sub(total_size as u64, Ordering::Relaxed);
            return Err(VmError::NoMemory);
        }

        // Calculate virtual addresses
        let guard_vaddr = self.base_vaddr + offset;
        let stack_base = guard_vaddr + (GUARD_PAGES * PAGE_SIZE);
        let stack_top = stack_base + size;

        // Allocate physical pages (including guard page)
        let num_pages = total_size / PAGE_SIZE;
        let phys_base = pmm::pmm_alloc_contiguous(num_pages, pmm::PMM_ALLOC_FLAG_ANY, 12)?;

        // Map the stack into kernel address space
        // Note: This requires access to the kernel address space
        // For now, we'll create the descriptor
        let stack = KernelStack::new(
            stack_top,
            stack_base,
            size,
            phys_base,
            num_pages,
            guard_vaddr,
            owner_id,
        );

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.total_stacks += 1;
            stats.active_stacks += 1;
            stats.total_memory += total_size as u64;
        }

        log_debug!(
            "Allocated stack: owner={} top={:#x} base={:#x} size={}",
            owner_id,
            stack_top,
            stack_base,
            size
        );

        Ok(stack)
    }

    /// Free a kernel stack
    pub fn free_stack(&self, stack: KernelStack) {
        // Free physical pages
        for i in 0..stack.num_pages {
            let paddr = stack.phys_base + (i * PAGE_SIZE);
            pmm::pmm_free_page(paddr);
        }

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.active_stacks -= 1;
        }

        log_debug!(
            "Freed stack: owner={} base={:#x}",
            stack.owner_id,
            stack.base
        );
    }

    /// Get allocator statistics
    pub fn stats(&self) -> StackStats {
        *self.stats.lock()
    }
}

/// ============================================================================
/// Global Stack Allocator
/// ============================================================================

/// Global kernel stack allocator
///
/// This is initialized during kernel boot.
static mut GLOBAL_STACK_ALLOCATOR: Option<StackAllocator> = None;

/// Initialize the global stack allocator
///
/// # Safety
///
/// Must be called exactly once during kernel initialization.
pub unsafe fn init_stacks() {
    #[cfg(target_arch = "aarch64")]
    let (base, size) = {
        let stacks_base = arm64::KERNEL_HEAP_BASE + arm64::KERNEL_HEAP_SIZE;
        let stacks_size = 256 * KERNEL_STACK_SIZE; // Space for 256 stacks
        (stacks_base, stacks_size)
    };

    #[cfg(target_arch = "x86_64")]
    let (base, size) = {
        let stacks_base = amd64::KERNEL_HEAP_BASE + amd64::KERNEL_HEAP_SIZE;
        let stacks_size = 256 * KERNEL_STACK_SIZE;
        (stacks_base, stacks_size)
    };

    #[cfg(target_arch = "riscv64")]
    let (base, size) = {
        let stacks_base = riscv::KERNEL_HEAP_BASE + riscv::KERNEL_HEAP_SIZE;
        let stacks_size = 256 * KERNEL_STACK_SIZE;
        (stacks_base, stacks_size)
    };

    GLOBAL_STACK_ALLOCATOR = Some(StackAllocator::new(base, size));

    log_info!("Kernel stack allocator initialized");
    log_info!("  Base: {:#x}", base);
    log_info!("  Capacity: {} stacks", size / DEFAULT_KERNEL_STACK_SIZE);
}

/// Allocate a kernel stack
pub fn alloc_kernel_stack(owner_id: u64) -> Result<KernelStack> {
    unsafe {
        match &GLOBAL_STACK_ALLOCATOR {
            Some(alloc) => alloc.alloc_stack(owner_id, DEFAULT_KERNEL_STACK_SIZE),
            None => {
                log_error!("Stack allocator not initialized");
                Err(VmError::BadState)
            }
        }
    }
}

/// Allocate a kernel stack with custom size
pub fn alloc_kernel_stack_with_size(owner_id: u64, size: usize) -> Result<KernelStack> {
    unsafe {
        match &GLOBAL_STACK_ALLOCATOR {
            Some(alloc) => alloc.alloc_stack(owner_id, size),
            None => {
                log_error!("Stack allocator not initialized");
                Err(VmError::BadState)
            }
        }
    }
}

/// Free a kernel stack
pub fn free_kernel_stack(stack: KernelStack) {
    unsafe {
        match &GLOBAL_STACK_ALLOCATOR {
            Some(alloc) => alloc.free_stack(stack),
            None => {
                log_error!("Stack allocator not initialized");
            }
        }
    }
}

/// Get stack allocator statistics
pub fn get_stack_stats() -> Option<StackStats> {
    unsafe {
        GLOBAL_STACK_ALLOCATOR.as_ref().map(|alloc| alloc.stats())
    }
}

/// ============================================================================
/// Stack Guard Fault Handling
/// ============================================================================

/// Handle a stack guard page fault
///
/// Called when a page fault occurs on a guard page.
/// This indicates a stack overflow.
pub fn handle_stack_guard_fault(vaddr: VAddr) -> Result {
    log_error!("Stack guard page hit at {:#x}", vaddr);
    log_error!("  This indicates a stack overflow!");

    unsafe {
        match &GLOBAL_STACK_ALLOCATOR {
            Some(alloc) => {
                let mut stats = alloc.stats.lock();
                stats.overflows += 1;
            }
            None => {}
        }
    }

    Err(VmError::PageFault)
}

/// ============================================================================
/// Helper Functions
/// ============================================================================

/// Validate stack pointer alignment
pub fn validate_stack_sp(sp: VAddr) -> bool {
    (sp % STACK_ALIGN) == 0
}

/// Check if a stack pointer is within valid stack range
pub fn is_valid_stack_sp(sp: VAddr, stack: &KernelStack) -> bool {
    sp >= stack.bottom() && sp <= stack.top
}

/// Get the amount of stack space remaining
pub fn stack_remaining(stack: &KernelStack, current_sp: VAddr) -> usize {
    if current_sp > stack.top || current_sp < stack.bottom() {
        return 0;
    }

    stack.top - current_sp
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_size_constants() {
        assert!(MIN_KERNEL_STACK_SIZE < DEFAULT_KERNEL_STACK_SIZE);
        assert!(DEFAULT_KERNEL_STACK_SIZE <= MAX_KERNEL_STACK_SIZE);
        assert_eq!(DEFAULT_KERNEL_STACK_SIZE, 64 * 1024);
    }

    #[test]
    fn test_stack_alignment() {
        assert_eq!(STACK_ALIGN, 16);
        assert!(validate_stack_sp(0x1000));
        assert!(validate_stack_sp(0x100000000000));
        assert!(!validate_stack_sp(0x1001));
    }

    #[test]
    fn test_stack_region() {
        let guard = 0x8000_0000;
        let base = guard + PAGE_SIZE;
        let top = base + DEFAULT_KERNEL_STACK_SIZE;

        // Test stack contains
        assert!(!KernelStack::new(
            top, base, DEFAULT_KERNEL_STACK_SIZE, 0, 0, guard, 0
        ).contains(guard)); // Guard page
        assert!(KernelStack::new(
            top, base, DEFAULT_KERNEL_STACK_SIZE, 0, 0, guard, 0
        ).contains(base)); // Bottom
        assert!(KernelStack::new(
            top, base, DEFAULT_KERNEL_STACK_SIZE, 0, 0, guard, 0
        ).contains(top - 8)); // Near top
    }
}
