// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! AMD64 Architecture Abstraction Layer (AAL) implementation
//!
//! This module implements the AAL traits for AMD64 (x86-64),
//! providing the architecture-specific implementations.


use crate::kernel::arch::arch_traits::*;
use crate::kernel::arch::amd64;
use crate::rustux::types::*;

/// Marker type for AMD64 architecture
pub enum Amd64Arch {}

// ============= ArchStartup Implementation =============

impl ArchStartup for Amd64Arch {
    unsafe fn early_init() {
        amd64::arch_early_init();
    }

    unsafe fn init_mmu() {
        amd64::mmu::x86_mmu_init();
    }

    unsafe fn init_exceptions() {
        // Exception vectors are installed in start.S
    }

    unsafe fn late_init() {
        amd64::arch_late_init();
    }
}

// ============= ArchThreadContext Implementation =============

impl ArchThreadContext for Amd64Arch {
    type Context = amd64::X86ThreadStateGeneralRegs;

    unsafe fn init_thread(
        thread: &mut crate::kernel::thread::Thread,
        entry_point: VAddr,
        arg: usize,
        stack_top: VAddr,
    ) {
        amd64::arch_thread_initialize(thread);
    }

    unsafe fn save_context(context: &mut Self::Context) {
        // Context save is done inline during context switch
        let _ = context;
    }

    unsafe fn restore_context(context: &Self::Context) -> ! {
        // This would jump to saved context
        // For now, panic as this should be handled by context_switch
        panic!("restore_context should use context_switch instead");
    }

    unsafe fn context_switch(
        old_thread: &mut crate::kernel::thread::Thread,
        new_thread: &mut crate::kernel::thread::Thread,
    ) {
        amd64::arch_context_switch(old_thread, new_thread);
    }

    unsafe fn current_sp() -> usize {
        let sp: usize;
        core::arch::asm!("mov {0}, rsp", out(reg) sp);
        sp
    }

    unsafe fn set_sp(sp: usize) {
        core::arch::asm!("mov rsp, {0}", in(reg) sp);
    }
}

// ============= ArchTimer Implementation =============

impl ArchTimer for Amd64Arch {
    fn now_monotonic() -> u64 {
        amd64::timer::x86_rdtsc()
    }

    fn set_timer(deadline: u64) {
        // Use APIC timer
        unsafe { amd64::apic::apic_timer_set_tsc_deadline(deadline); }
    }

    fn cancel_timer() {
        unsafe { amd64::apic::apic_timer_stop(); }
    }

    fn get_frequency() -> u64 {
        amd64::timer::x86_tsc_frequency()
    }
}

// ============= ArchInterrupts Implementation =============

impl ArchInterrupts for Amd64Arch {
    unsafe fn enable_irq(irq: u32) {
        amd64::interrupts::x86_enable_irq(irq);
    }

    unsafe fn disable_irq(irq: u32) {
        amd64::interrupts::x86_disable_irq(irq);
    }

    unsafe fn end_of_interrupt(irq: u32) {
        amd64::interrupts::x86_send_eoi(irq);
    }

    fn interrupts_enabled() -> bool {
        amd64::interrupts::x86_interrupts_enabled()
    }

    unsafe fn disable_interrupts() -> u64 {
        amd64::interrupts::x86_disable_interrupts()
    }

    unsafe fn restore_interrupts(state: u64) {
        amd64::interrupts::x86_restore_interrupts(state);
    }

    unsafe fn send_ipi(target_cpu: u32, vector: u32) -> i32 {
        amd64::interrupts::x86_send_ipi(vector, target_cpu)
    }
}

// ============= ArchMMU Implementation =============

impl ArchMMU for Amd64Arch {
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
            amd64::asm::x86_tlb_global_invalidate();
        } else {
            // Invalidate specific address
            amd64::asm::x86_tlb_invalidate_page(va);
        }
    }

    unsafe fn flush_tlb_all() {
        amd64::asm::x86_tlb_global_invalidate();
    }

    unsafe fn is_valid_va(va: VAddr) -> bool {
        amd64::mmu::x86_is_vaddr_canonical_impl(va)
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

impl ArchCache for Amd64Arch {
    unsafe fn clean_dcache(addr: VAddr, len: usize) {
        amd64::cache::arch_clean_cache_range(addr, len);
    }

    unsafe fn invalidate_dcache(addr: VAddr, len: usize) {
        amd64::cache::arch_invalidate_cache_range(addr, len);
    }

    unsafe fn clean_invalidate_dcache(addr: VAddr, len: usize) {
        amd64::cache::arch_clean_invalidate_cache_range(addr, len);
    }

    unsafe fn sync_icache(addr: VAddr, len: usize) {
        amd64::cache::arch_sync_cache_range(addr, len);
    }

    fn dcache_line_size() -> usize {
        amd64::cache::arch_dcache_line_size()
    }

    fn icache_line_size() -> usize {
        amd64::cache::arch_icache_line_size()
    }
}

// ============= ArchCpuId Implementation =============

impl ArchCpuId for Amd64Arch {
    fn current_cpu() -> u32 {
        amd64::mp::x86_get_cpuid()
    }

    fn cpu_count() -> u32 {
        amd64::mp::x86_cpu_count()
    }

    fn get_features() -> u64 {
        unsafe { amd64::feature::x86_feature_get_all() }
    }
}

// ============= ArchMemoryBarrier Implementation =============

impl ArchMemoryBarrier for Amd64Arch {
    fn mb() {
        unsafe { core::arch::asm!("mfence", options(nostack)); }
    }

    fn rmb() {
        unsafe { core::arch::asm!("lfence", options(nostack)); }
    }

    fn wmb() {
        unsafe { core::arch::asm!("sfence", options(nostack)); }
    }

    fn acquire() {
        unsafe { core::arch::asm!("mfence", options(nostack)); }
    }

    fn release() {
        unsafe { core::arch::asm!("sfence", options(nostack)); }
    }
}

// ============= ArchHalt Implementation =============

impl ArchHalt for Amd64Arch {
    unsafe fn halt() {
        core::arch::asm!("hlt");
    }

    fn pause() {
        unsafe { core::arch::asm!("pause"); }
    }

    fn serialize() {
        unsafe {
            let _: u32;
            core::arch::asm!("cpuid", lateout("eax") _, lateout("ecx") _);
        }
    }
}

// ============= ArchUserAccess Implementation =============

impl ArchUserAccess for Amd64Arch {
    unsafe fn copy_from_user(dst: *mut u8, src: VAddr, len: usize) -> isize {
        extern "C" {
            fn _x86_copy_to_or_from_user(
                dst: *mut u8,
                src: *const u8,
                len: usize,
                fault_return: *mut *const u8,
            ) -> usize;
        }

        let mut fault_return: *const u8 = core::ptr::null();
        let result = _x86_copy_to_or_from_user(
            dst,
            src as *const u8,
            len,
            &mut fault_return,
        );

        if !fault_return.is_null() {
            return -1; // Fault occurred
        }

        result as isize
    }

    unsafe fn copy_to_user(dst: VAddr, src: *const u8, len: usize) -> isize {
        extern "C" {
            fn _x86_copy_to_or_from_user(
                dst: *mut u8,
                src: *const u8,
                len: usize,
                fault_return: *mut *const u8,
            ) -> usize;
        }

        let mut fault_return: *const u8 = core::ptr::null();
        let result = _x86_copy_to_or_from_user(
            dst as *mut u8,
            src,
            len,
            &mut fault_return,
        );

        if !fault_return.is_null() {
            return -1; // Fault occurred
        }

        result as isize
    }

    fn is_user_address(addr: VAddr) -> bool {
        amd64::is_user_address(addr)
    }

    unsafe fn validate_user_range(addr: VAddr, len: usize, _write: bool) -> bool {
        // Check for overflow
        if addr.wrapping_add(len) < addr {
            return false;
        }

        // Check if all addresses are in user space
        // User addresses are canonical with bit 47 = 0
        const CANONICAL_MASK: u64 = 0xFFFF800000000000;
        let addr_end = addr + len;

        (addr as u64 & CANONICAL_MASK) == 0 && (addr_end as u64 & CANONICAL_MASK) == 0
    }
}

// ============= ArchUserEntry Implementation =============

impl ArchUserEntry for Amd64Arch {
    unsafe fn enter_userspace(arg1: usize, arg2: usize, sp: usize, pc: usize, flags: u64) -> ! {
        amd64::x86_uspace_entry(arg1, arg2, sp, pc, flags)
    }

    unsafe fn return_to_userspace(iframe: *mut ()) -> ! {
        amd64::x86_uspace_exception_return(iframe)
    }
}

// ============= ArchDebug Implementation =============

impl ArchDebug for Amd64Arch {
    fn read_perf_counter() -> u64 {
        amd64::asm::rdtsc()
    }

    unsafe fn set_hw_breakpoint(addr: VAddr, kind: u32) -> i32 {
        // Use x86 debug registers (DR0-DR3, DR7)
        // TODO: Implement proper breakpoint support
        let _ = addr;
        let _ = kind;
        -1 // Not implemented yet
    }

    unsafe fn disable_hw_breakpoints() {
        amd64::debugger::x86_disable_debug_state();
    }
}

// ============= ArchFpu Implementation =============

impl ArchFpu for Amd64Arch {
    type FpuState = amd64::debugger::X86ThreadStateVectorRegs;

    unsafe fn init() {
        // FPU is enabled by default on AMD64
        // No special init needed
    }

    unsafe fn save(state: *mut Self::FpuState) {
        // Get the current thread - for now use null
        // In a real implementation, this would get the current thread
        amd64::debugger::x86_get_set_vector_regs(
            &mut crate::kernel::thread::Thread::dummy_thread(),
            state,
            crate::kernel::arch::amd64::debugger::RegAccess::Get,
        );
    }

    unsafe fn restore(state: *const Self::FpuState) {
        // Get the current thread - for now use null
        amd64::debugger::x86_get_set_vector_regs(
            &mut crate::kernel::thread::Thread::dummy_thread(),
            state as *mut _,
            crate::kernel::arch::amd64::debugger::RegAccess::Set,
        );
    }

    fn is_enabled() -> bool {
        true // FPU is always enabled on x86-64
    }

    unsafe fn enable() {
        // FPU is always enabled, nothing to do
    }

    unsafe fn disable() {
        // Can't really disable FPU on x86-64
        // Would require CR0 modifications
    }
}

// ============= Arch Marker Trait Implementation =============

impl Arch for Amd64Arch {}
