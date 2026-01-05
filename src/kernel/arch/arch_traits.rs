// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture Abstraction Layer (AAL)
//!
//! This module defines traits that all architectures must implement
//! to provide a consistent interface across ARM64, x86-64, and RISC-V.
//!
//! The AAL enables the kernel core to be architecture-agnostic while
//! allowing architecture-specific optimizations where needed.

#![no_std]

use crate::rustux::types::*;

/// Virtual memory size (48-bit address space)
pub const ARCH_VADDR_SIZE_BITS: u8 = 48;

/// Page size (4KB)
pub const ARCH_PAGE_SIZE: usize = 4096;

/// Page shift (log2 of page size)
pub const ARCH_PAGE_SIZE_SHIFT: u32 = 12;

/// Page mask
pub const ARCH_PAGE_MASK: usize = ARCH_PAGE_SIZE - 1;

/// Architecture startup and initialization
pub trait ArchStartup {
    /// Early initialization called from assembly entry point
    ///
    /// This is called very early in boot, before most services are available.
    /// It should:
    /// - Initialize the boot CPU
    /// - Set up early exception vectors
    /// - Initialize the MMU for kernel execution
    /// - Set up kernel stack and basic CPU state
    unsafe fn early_init();

    /// Initialize MMU for kernel operation
    ///
    /// Sets up page tables for kernel address space and enables paging.
    unsafe fn init_mmu();

    /// Initialize exception/interrupt vectors
    ///
    /// Installs handlers for exceptions, interrupts, and syscalls.
    unsafe fn init_exceptions();

    /// Late initialization after core services are up
    ///
    /// Called after basic kernel services are initialized.
    unsafe fn late_init();
}

/// Architecture thread context management
pub trait ArchThreadContext {
    /// Thread context type
    type Context;

    /// Initialize a new thread context
    ///
    /// # Arguments
    ///
    /// * `thread` - Thread structure to initialize
    /// * `entry_point` - Function where thread should start
    /// * `arg` - Argument to pass to entry function
    /// * `stack_top` - Top of stack for this thread
    unsafe fn init_thread(
        thread: &mut crate::kernel::thread::Thread,
        entry_point: VAddr,
        arg: usize,
        stack_top: VAddr,
    );

    /// Save current thread context
    ///
    /// # Arguments
    ///
    /// * `context` - Where to save the context
    unsafe fn save_context(context: &mut Self::Context);

    /// Restore a saved thread context
    ///
    /// # Arguments
    ///
    /// * `context` - Context to restore
    ///
    /// # Safety
    ///
    /// This function does not return - it jumps to the saved context
    unsafe fn restore_context(context: &Self::Context) -> !;

    /// Context switch between threads
    ///
    /// # Arguments
    ///
    /// * `old_thread` - Current thread (save context here)
    /// * `new_thread` - Thread to switch to (restore context from here)
    unsafe fn context_switch(
        old_thread: &mut crate::kernel::thread::Thread,
        new_thread: &mut crate::kernel::thread::Thread,
    );

    /// Get pointer to current thread's stack pointer
    ///
    /// # Returns
    ///
    /// Current stack pointer value
    unsafe fn current_sp() -> usize;

    /// Set current thread's stack pointer
    ///
    /// # Arguments
    ///
    /// * `sp` - New stack pointer value
    unsafe fn set_sp(sp: usize);
}

/// Architecture timer interface
pub trait ArchTimer {
    /// Get current monotonic time
    ///
    /// Returns a timestamp that increments at a constant rate.
    /// The value may wrap around.
    ///
    /// # Returns
    ///
    /// Current timestamp in nanoseconds (or architecture-specific units)
    fn now_monotonic() -> u64;

    /// Set one-shot timer deadline
    ///
    /// # Arguments
    ///
    /// * `deadline` - Absolute time when timer should fire
    fn set_timer(deadline: u64);

    /// Cancel the one-shot timer
    fn cancel_timer();

    /// Get timer resolution
    ///
    /// # Returns
    ///
    /// Number of timer ticks per second
    fn get_frequency() -> u64;

    /// Convert timer ticks to nanoseconds
    fn ticks_to_nanos(ticks: u64) -> u64 {
        let freq = Self::get_frequency();
        if freq == 0 {
            return 0;
        }
        (ticks * 1_000_000_000) / freq
    }

    /// Convert nanoseconds to timer ticks
    fn nanos_to_ticks(nanos: u64) -> u64 {
        let freq = Self::get_frequency();
        if freq == 1_000_000_000 {
            return nanos;
        }
        (nanos * freq) / 1_000_000_000
    }
}

/// Architecture interrupt controller interface
pub trait ArchInterrupts {
    /// Enable a specific IRQ
    ///
    /// # Arguments
    ///
    /// * `irq` - IRQ number to enable
    unsafe fn enable_irq(irq: u32);

    /// Disable a specific IRQ
    ///
    /// # Arguments
    ///
    /// * `irq` - IRQ number to disable
    unsafe fn disable_irq(irq: u32);

    /// Send end-of-interrupt signal
    ///
    /// Called after handling an interrupt to acknowledge completion.
    ///
    /// # Arguments
    ///
    /// * `irq` - IRQ number being completed
    unsafe fn end_of_interrupt(irq: u32);

    /// Check if interrupts are currently enabled
    ///
    /// # Returns
    ///
    /// true if interrupts are enabled
    fn interrupts_enabled() -> bool;

    /// Disable interrupts and return previous state
    ///
    /// # Returns
    ///
    /// State that can be passed to `restore_interrupts`
    unsafe fn disable_interrupts() -> u64;

    /// Restore interrupt state
    ///
    /// # Arguments
    ///
    /// * `state` - State returned from `disable_interrupts`
    unsafe fn restore_interrupts(state: u64);

    /// Send inter-processor interrupt (IPI)
    ///
    /// # Arguments
    ///
    /// * `target_cpu` - CPU number to send IPI to
    /// * `vector` - IPI vector/type
    ///
    /// # Returns
    ///
    /// 0 on success, negative on error
    unsafe fn send_ipi(target_cpu: u32, vector: u32) -> i32;
}

/// Architecture MMU interface
pub trait ArchMMU {
    /// Map physical pages to virtual addresses
    ///
    /// # Arguments
    ///
    /// * `pa` - Physical address to map
    /// * `va` - Virtual address to map to
    /// * `len` - Size of mapping in bytes (must be page-aligned)
    /// * `flags` - Page table flags (read, write, execute, user, etc.)
    ///
    /// # Returns
    ///
    /// 0 on success, negative error code on failure
    unsafe fn map(pa: PAddr, va: VAddr, len: usize, flags: u64) -> i32;

    /// Unmap virtual pages
    ///
    /// # Arguments
    ///
    /// * `va` - Virtual address to unmap
    /// * `len` - Size of region to unmap (must be page-aligned)
    unsafe fn unmap(va: VAddr, len: usize);

    /// Change protection flags for a mapping
    ///
    /// # Arguments
    ///
    /// * `va` - Virtual address
    /// * `len` - Size of region
    /// * `flags` - New protection flags
    ///
    /// # Returns
    ///
    /// 0 on success, negative error code on failure
    unsafe fn protect(va: VAddr, len: usize, flags: u64) -> i32;

    /// Flush TLB entries for a virtual address range
    ///
    /// # Arguments
    ///
    /// * `va` - Virtual address to flush
    /// * `len` - Size of region (0 for full TLB flush)
    unsafe fn flush_tlb(va: VAddr, len: usize);

    /// Flush entire TLB
    unsafe fn flush_tlb_all();

    /// Check if a virtual address is valid (mapped)
    ///
    /// # Arguments
    ///
    /// * `va` - Virtual address to check
    ///
    /// # Returns
    ///
    /// true if the address is mapped
    unsafe fn is_valid_va(va: VAddr) -> bool;

    /// Get physical address for a virtual address
    ///
    /// # Arguments
    ///
    /// * `va` - Virtual address
    ///
    /// # Returns
    ///
    /// Physical address, or 0 if not mapped
    unsafe fn virt_to_phys(va: VAddr) -> PAddr;

    /// Convert physical to virtual address (direct mapping)
    ///
    /// # Arguments
    ///
    /// * `pa` - Physical address
    ///
    /// # Returns
    ///
    /// Virtual address in direct mapping region
    unsafe fn phys_to_virt(pa: PAddr) -> VAddr;
}

/// Architecture cache operations
pub trait ArchCache {
    /// Clean data cache for a memory range
    ///
    /// Ensures dirty cache lines are written to memory.
    ///
    /// # Arguments
    ///
    /// * `addr` - Start address
    /// * `len` - Length of range
    unsafe fn clean_dcache(addr: VAddr, len: usize);

    /// Invalidate data cache for a memory range
    ///
    /// Discards cache lines without writing back.
    ///
    /// # Arguments
    ///
    /// * `addr` - Start address
    /// * `len` - Length of range
    unsafe fn invalidate_dcache(addr: VAddr, len: usize);

    /// Clean and invalidate data cache
    ///
    /// Writes back and then discards cache lines.
    ///
    /// # Arguments
    ///
    /// * `addr` - Start address
    /// * `len` - Length of range
    unsafe fn clean_invalidate_dcache(addr: VAddr, len: usize);

    /// Synchronize instruction cache
    ///
    /// Ensures instruction cache sees modifications to code.
    ///
    /// # Arguments
    ///
    /// * `addr` - Start address
    /// * `len` - Length of range
    unsafe fn sync_icache(addr: VAddr, len: usize);

    /// Get data cache line size
    ///
    /// # Returns
    ///
    /// Cache line size in bytes
    fn dcache_line_size() -> usize;

    /// Get instruction cache line size
    ///
    /// # Returns
    ///
    /// Cache line size in bytes
    fn icache_line_size() -> usize;
}

/// Architecture CPU identification
pub trait ArchCpuId {
    /// Get current CPU number
    ///
    /// # Returns
    ///
    /// 0-based CPU index
    fn current_cpu() -> u32;

    /// Get total number of CPUs in the system
    ///
    /// # Returns
    ///
    /// Total CPU count
    fn cpu_count() -> u32;

    /// Get CPU feature flags
    ///
    /// # Returns
    ///
    /// Bitmask of supported CPU features
    fn get_features() -> u64;

    /// Check if a CPU feature is supported
    ///
    /// # Arguments
    ///
    /// * `feature` - Feature bit to check
    ///
    /// # Returns
    ///
    /// true if the feature is supported
    fn has_feature(feature: u64) -> bool {
        Self::get_features() & feature != 0
    }
}

/// Architecture memory barrier operations
pub trait ArchMemoryBarrier {
    /// Compiler barrier - prevents compiler reordering
    fn compiler_barrier() {
        unsafe { core::arch::asm!("", options(nostack, nomem)); }
    }

    /// Full memory barrier - loads and stores
    fn mb() {
        unsafe { core::arch::asm!("fence", options(nostack, nomem)); }
    }

    /// Read memory barrier - loads only
    fn rmb() {
        unsafe { core::arch::asm!("fence ir, ir", options(nostack, nomem)); }
    }

    /// Write memory barrier - stores only
    fn wmb() {
        unsafe { core::arch::asm!("fence ow, ow", options(nostack, nomem)); }
    }

    /// Acquire barrier
    fn acquire() {
        unsafe { core::arch::asm!("fence r, rw", options(nostack, nomem)); }
    }

    /// Release barrier
    fn release() {
        unsafe { core::arch::asm!("fence rw, w", options(nostack, nomem)); }
    }
}

/// CPU halt/wake operations
pub trait ArchHalt {
    /// Halt the CPU until interrupt
    ///
    /// Puts the CPU into a low-power state waiting for an interrupt.
    unsafe fn halt();

    /// Pause CPU (hint to CPU that we're spinning)
    fn pause() {
        unsafe { core::arch::asm!("pause", options(nostack)); }
    }

    /// Serialize instruction execution
    fn serialize() {
        unsafe {
            let _: u32;
            core::arch::asm!("cpuid", lateout("eax") _, options(nostack));
        }
    }
}

/// User space access operations
pub trait ArchUserAccess {
    /// Copy data from user space to kernel
    ///
    /// # Arguments
    ///
    /// * `dst` - Kernel destination
    /// * `src` - User source
    /// * `len` - Number of bytes
    ///
    /// # Returns
    ///
    /// Bytes copied, or negative on fault
    unsafe fn copy_from_user(dst: *mut u8, src: VAddr, len: usize) -> isize;

    /// Copy data from kernel to user space
    ///
    /// # Arguments
    ///
    /// * `dst` - User destination
    /// * `src` - Kernel source
    /// * `len` - Number of bytes
    ///
    /// # Returns
    ///
    /// Bytes copied, or negative on fault
    unsafe fn copy_to_user(dst: VAddr, src: *const u8, len: usize) -> isize;

    /// Check if address is in user space
    ///
    /// # Arguments
    ///
    /// * `addr` - Address to check
    ///
    /// # Returns
    ///
    /// true if address is in user space
    fn is_user_address(addr: VAddr) -> bool;

    /// Validate user address range
    ///
    /// # Arguments
    ///
    /// * `addr` - Start of range
    /// * `len` - Length of range
    /// * `write` - true if checking for write access
    ///
    /// # Returns
    ///
    /// true if range is accessible
    unsafe fn validate_user_range(addr: VAddr, len: usize, write: bool) -> bool;
}

/// User space entry/exit
pub trait ArchUserEntry {
    /// Enter user space
    ///
    /// # Arguments
    ///
    /// * `arg1` - First argument to user function
    /// * `arg2` - Second argument to user function
    /// * `sp` - User stack pointer
    /// * `pc` - User program counter (entry point)
    /// * `flags` - User flags (e.g., interrupt enable)
    ///
    /// # Safety
    ///
    /// This function never returns
    unsafe fn enter_userspace(arg1: usize, arg2: usize, sp: usize, pc: usize, flags: u64) -> !;

    /// Return from exception/syscall to user space
    ///
    /// # Arguments
    ///
    /// * `iframe` - Interrupt frame with user state
    ///
    /// # Safety
    ///
    /// This function never returns
    unsafe fn return_to_userspace(iframe: *mut ()) -> !;
}

/// Debug and profiling support
pub trait ArchDebug {
    /// Read performance counter
    ///
    /// # Returns
    ///
    /// Current performance counter value
    fn read_perf_counter() -> u64 {
        0 // Default: no performance counter
    }

    /// Enable hardware breakpoints
    ///
    /// # Arguments
    ///
    /// * `addr` - Address to break on
    /// * `kind` - Breakpoint type (0=execute, 1=write, 2=access)
    ///
    /// # Returns
    ///
    /// 0 on success, negative if not supported
    unsafe fn set_hw_breakpoint(addr: VAddr, kind: u32) -> i32 {
        -1 // Default: not supported
    }

    /// Disable hardware breakpoints
    unsafe fn disable_hw_breakpoints() {
        // Default: do nothing
    }
}

/// FPU state management
pub trait ArchFpu {
    /// FPU state type
    type FpuState;

    /// Initialize FPU for current CPU
    unsafe fn init();

    /// Save FPU state
    ///
    /// # Arguments
    ///
    /// * `state` - Where to save state
    unsafe fn save(state: *mut Self::FpuState);

    /// Restore FPU state
    ///
    /// # Arguments
    ///
    /// * `state` - State to restore
    unsafe fn restore(state: *const Self::FpuState);

    /// Check if FPU is enabled
    fn is_enabled() -> bool;

    /// Enable FPU access
    unsafe fn enable();

    /// Disable FPU access
    unsafe fn disable();
}

/// Generic architecture trait combining all traits
///
/// Each architecture should implement this marker trait
/// to indicate it provides all required functionality.
pub trait Arch:
    ArchStartup
    + ArchThreadContext
    + ArchTimer
    + ArchInterrupts
    + ArchMMU
    + ArchCache
    + ArchCpuId
    + ArchMemoryBarrier
    + ArchHalt
    + ArchUserAccess
    + ArchUserEntry
    + ArchDebug
    + ArchFpu
{
}

/// Helper function to check if an address is aligned
///
/// # Arguments
///
/// * `addr` - Address to check
/// * `alignment` - Alignment requirement (must be power of 2)
///
/// # Returns
///
/// true if address is aligned
#[inline]
pub fn is_aligned(addr: usize, alignment: usize) -> bool {
    debug_assert!(alignment.is_power_of_two());
    addr & (alignment - 1) == 0
}

/// Align address down
///
/// # Arguments
///
/// * `addr` - Address to align
/// * `alignment` - Alignment (must be power of 2)
///
/// # Returns
///
/// Aligned address
#[inline]
pub fn align_down(addr: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    addr & !(alignment - 1)
}

/// Align address up
///
/// # Arguments
///
/// * `addr` - Address to align
/// * `alignment` - Alignment (must be power of 2)
///
/// # Returns
///
/// Aligned address
#[inline]
pub fn align_up(addr: usize, alignment: usize) -> usize {
    align_down(addr + alignment - 1, alignment)
}
