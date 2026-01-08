// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 Architecture Abstraction Layer (AAL) implementation
//!
//! This module implements the AAL traits for ARM64, providing
//! the architecture-specific implementations of the common interfaces.

#![no_std]

use crate::kernel::arch::arch_traits::*;
use crate::arch::arm64;
use crate::arch::arm64::registers;
use crate::rustux::types::*;

/// Marker type for ARM64 architecture
pub enum Arm64Arch {}

// ============= ArchStartup Implementation =============

impl ArchStartup for Arm64Arch {
    unsafe fn early_init() {
        // Initialize the boot CPU
        arm64::arch_early_init();
    }

    unsafe fn init_mmu() {
        arm64::mmu::arm64_mmu_init();
    }

    unsafe fn init_exceptions() {
        // Exception vectors are installed in start.S
        // This just does any additional runtime setup
    }

    unsafe fn late_init() {
        arm64::arch_late_init();
    }
}

// ============= ArchThreadContext Implementation =============

impl ArchThreadContext for Arm64Arch {
    type Context = arm64::arm64_thread_context_t;

    unsafe fn init_thread(
        thread: &mut crate::kernel::thread::Thread,
        entry_point: VAddr,
        arg: usize,
        stack_top: VAddr,
    ) {
        arm64::arch_thread_initialize(
            thread as *mut _ as *mut core::ffi::c_void,
            entry_point as u64,
            arg as u64,
            stack_top as u64,
        );
    }

    unsafe fn save_context(context: &mut Self::Context) {
        arm64::arch_thread_context_save(context);
    }

    unsafe fn restore_context(context: &Self::Context) -> ! {
        arm64::arch_thread_context_restore(context);
        unreachable!();
    }

    unsafe fn context_switch(
        old_thread: &mut crate::kernel::thread::Thread,
        new_thread: &mut crate::kernel::thread::Thread,
    ) {
        arm64::arch_thread_context_switch(
            &mut old_thread.arch.sp as *mut VAddr as *mut arm64::arm64_thread_context_t,
            &new_thread.arch.sp as *const VAddr as *const arm64::arm64_thread_context_t,
        );
    }

    unsafe fn current_sp() -> usize {
        let sp: usize;
        core::arch::asm!("mov {0}, sp", out(reg) sp);
        sp
    }

    unsafe fn set_sp(sp: usize) {
        core::arch::asm!("mov sp, {0}", in(reg) sp);
    }
}

// ============= ArchTimer Implementation =============

impl ArchTimer for Arm64Arch {
    fn now_monotonic() -> u64 {
        arm64::timer::arm64_current_time()
    }

    fn set_timer(deadline: u64) {
        unsafe { arm64::timer::arm64_timer_set(deadline); }
    }

    fn cancel_timer() {
        unsafe { arm64::timer::arm64_timer_cancel(); }
    }

    fn get_frequency() -> u64 {
        arm64::timer::arm64_timer_get_frequency()
    }
}

// ============= ArchInterrupts Implementation =============

impl ArchInterrupts for Arm64Arch {
    unsafe fn enable_irq(irq: u32) {
        arm64::interrupts::mask_unmask_irq(irq, true);
    }

    unsafe fn disable_irq(irq: u32) {
        arm64::interrupts::mask_unmask_irq(irq, false);
    }

    unsafe fn end_of_interrupt(irq: u32) {
        arm64::interrupts::send_eoi(irq);
    }

    fn interrupts_enabled() -> bool {
        unsafe { arm64::arch_ints_disabled() }
    }

    unsafe fn disable_interrupts() -> u64 {
        let daif: u64;
        core::arch::asm!("mrs {0}, daif", out(reg) daif);
        // Disable IRQ and FIQ
        core::arch::asm!("msr daifset, #2");
        core::arch::asm!("msr daifset, #1");
        daif
    }

    unsafe fn restore_interrupts(state: u64) {
        core::arch::asm!("msr daif, {0}", in(reg) state);
    }

    unsafe fn send_ipi(target_cpu: u32, vector: u32) -> i32 {
        // Use SGI (Software Generated Interrupt)
        arm64::interrupts::send_sgi_to_cpu(vector, target_cpu)
    }
}

// ============= ArchMMU Implementation =============

impl ArchMMU for Arm64Arch {
    unsafe fn map(pa: PAddr, va: VAddr, len: usize, flags: u64) -> i32 {
        // Call the MMU mapping function
        // TODO: Implement proper page table mapping
        let _ = pa;
        let _ = va;
        let _ = len;
        let _ = flags;
        0 // OK for now
    }

    unsafe fn unmap(va: VAddr, len: usize) {
        let _ = va;
        let _ = len;
        // TODO: Implement unmap
    }

    unsafe fn protect(va: VAddr, len: usize, flags: u64) -> i32 {
        let _ = va;
        let _ = len;
        let _ = flags;
        // TODO: Implement protect
        0 // OK for now
    }

    unsafe fn flush_tlb(va: VAddr, len: usize) {
        if len == 0 {
            // Flush entire TLB
            arm64::mmu::tlb_invalidate_all();
        } else {
            // Flush specific range
            arm64::mmu::tlb_invalidate_va(va);
        }
    }

    unsafe fn flush_tlb_all() {
        arm64::mmu::tlb_invalidate_all();
        // Invalidate all ASIDs by passing a specific value or 0
        // TODO: Determine correct ASID value to use
        arm64::mmu::tlb_invalidate_all_asid(0);
    }

    unsafe fn is_valid_va(va: VAddr) -> bool {
        // ARM64 48-bit virtual address space
        // User space: 0x0000_0000_0000_0000 to 0x0000_ffff_ffff_ffff
        // Kernel space: 0xffff_0000_0000_0000 to 0xffff_ffff_ffff_ffff
        const USER_MASK: u64 = 0xFFFF800000000000;
        (va as u64 & USER_MASK) == 0 || (va as u64 & USER_MASK) == USER_MASK
    }

    unsafe fn virt_to_phys(va: VAddr) -> PAddr {
        // TODO: Walk page tables
        // For now, assume direct mapping for physical memory
        va as PAddr
    }

    unsafe fn phys_to_virt(pa: PAddr) -> VAddr {
        // TODO: Use proper physical mapping
        // For now, just return physical address
        pa as VAddr
    }
}

// ============= ArchCache Implementation =============

impl ArchCache for Arm64Arch {
    unsafe fn clean_dcache(addr: VAddr, len: usize) {
        // TODO: Implement cache clean operation
        // See cache-ops.S for assembly implementation
        let _ = addr;
        let _ = len;
    }

    unsafe fn invalidate_dcache(addr: VAddr, len: usize) {
        // TODO: Implement cache invalidate operation
        // See cache-ops.S for assembly implementation
        let _ = addr;
        let _ = len;
    }

    unsafe fn clean_invalidate_dcache(addr: VAddr, len: usize) {
        // TODO: Implement cache clean+invalidate operation
        // See cache-ops.S for assembly implementation
        let _ = addr;
        let _ = len;
    }

    unsafe fn sync_icache(addr: VAddr, len: usize) {
        // TODO: Implement icache sync operation
        // See cache-ops.S for assembly implementation
        let _ = addr;
        let _ = len;
    }

    fn dcache_line_size() -> usize {
        arm64::include::arch::arch_ops::arch_dcache_line_size() as usize
    }

    fn icache_line_size() -> usize {
        arm64::include::arch::arch_ops::arch_icache_line_size() as usize
    }
}

// ============= ArchCpuId Implementation =============

impl ArchCpuId for Arm64Arch {
    fn current_cpu() -> u32 {
        unsafe { arm64::arch_curr_cpu_num() as u32 }
    }

    fn cpu_count() -> u32 {
        arm64::mp::arm64_cpu_count()
    }

    fn get_features() -> u64 {
        unsafe { arm64::feature::arm64_get_features() as u64 }
    }
}

// ============= ArchMemoryBarrier Implementation =============

impl ArchMemoryBarrier for Arm64Arch {
    // Default implementations from arch_traits work for ARM64
    // No special handling needed
}

// ============= ArchHalt Implementation =============

impl ArchHalt for Arm64Arch {
    unsafe fn halt() {
        core::arch::asm!("wfi");
    }

    fn pause() {
        // ARM64 doesn't have a pause instruction, use yield as hint
        unsafe { core::arch::asm!("yield") };
    }

    fn serialize() {
        unsafe {
            core::arch::asm!("dsb sy", options(nostack));
            core::arch::asm!("isb", options(nostack));
        }
    }
}

// ============= ArchUserAccess Implementation =============

impl ArchUserAccess for Arm64Arch {
    unsafe fn copy_from_user(dst: *mut u8, src: VAddr, len: usize) -> isize {
        arm64::user_copy_c::arm64_copy_from_user(dst, src as *const u8, len) as isize
    }

    unsafe fn copy_to_user(dst: VAddr, src: *const u8, len: usize) -> isize {
        arm64::user_copy_c::arm64_copy_to_user(dst as *mut u8, src, len) as isize
    }

    fn is_user_address(addr: VAddr) -> bool {
        arm64::arch_is_user_address(addr)
    }

    unsafe fn validate_user_range(addr: VAddr, len: usize, _write: bool) -> bool {
        // Check for overflow
        if addr.wrapping_add(len) < addr {
            return false;
        }

        // Check if all addresses are in user space
        arm64::arch_is_user_address(addr) && arm64::arch_is_user_address(addr + len - 1)
    }
}

// ============= ArchUserEntry Implementation =============

impl ArchUserEntry for Arm64Arch {
    unsafe fn enter_userspace(arg1: usize, arg2: usize, sp: usize, pc: usize, flags: u64) -> ! {
        // ARM64 uses eret to return to user space
        arm64::arch_enter_uspace(arg1, arg2, sp, pc, flags)
    }

    unsafe fn return_to_userspace(iframe: *mut ()) -> ! {
        arm64::arch_uspace_exception_return(iframe)
    }
}

// ============= ArchDebug Implementation =============

impl ArchDebug for Arm64Arch {
    fn read_perf_counter() -> u64 {
        unsafe {
            let cnt: u64;
            core::arch::asm!("mrs {0}, pmccntr_el0", out(reg) cnt);
            cnt
        }
    }

    unsafe fn set_hw_breakpoint(addr: VAddr, kind: u32) -> i32 {
        // Use ARM64 hardware breakpoints (via debug registers)
        // TODO: Implement proper breakpoint support
        let _ = addr;
        let _ = kind;
        -1 // Not implemented yet
    }

    unsafe fn disable_hw_breakpoints() {
        // Clear all debug breakpoints
        // TODO: Implement proper breakpoint support
    }
}

// ============= ArchFpu Implementation =============

impl ArchFpu for Arm64Arch {
    type FpuState = arm64::fpu::Arm64FpuState;

    unsafe fn init() {
        arm64::fpu::arm64_fpu_init();
    }

    unsafe fn save(state: *mut Self::FpuState) {
        arm64::fpu::arm64_fpu_save(state);
    }

    unsafe fn restore(state: *const Self::FpuState) {
        arm64::fpu::arm64_fpu_restore(state);
    }

    fn is_enabled() -> bool {
        arm64::fpu::arm64_fpu_enabled()
    }

    unsafe fn enable() {
        arm64::fpu::arm64_fpu_enable();
    }

    unsafe fn disable() {
        arm64::fpu::arm64_fpu_disable();
    }
}

// ============= Arch Marker Trait Implementation =============

impl Arch for Arm64Arch {}

/// Type alias for compatibility - AArch64Arch is the same as Arm64Arch
pub type AArch64Arch = Arm64Arch;
