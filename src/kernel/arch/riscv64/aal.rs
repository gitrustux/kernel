// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V Architecture Abstraction Layer (AAL) implementation
//!
//! This module implements the AAL traits for RISC-V (RV64GC),
//! providing the architecture-specific implementations.


use crate::kernel::arch::arch_traits::*;
use crate::arch::riscv64;
use crate::arch::riscv64::registers;
use crate::rustux::types::*;

/// Marker type for RISC-V architecture
pub enum Riscv64Arch {}

// ============= ArchStartup Implementation =============

impl ArchStartup for Riscv64Arch {
    unsafe fn early_init() {
        riscv64::arch_early_init();
    }

    unsafe fn init_mmu() {
        riscv64::mmu::enable_paging();
    }

    unsafe fn init_exceptions() {
        // Exception vectors are installed in start.S
    }

    unsafe fn late_init() {
        riscv64::arch_late_init();
    }
}

// ============= ArchThreadContext Implementation =============

impl ArchThreadContext for Riscv64Arch {
    type Context = riscv64::RiscvIframe;

    unsafe fn init_thread(
        thread: &mut crate::kernel::thread::Thread,
        entry_point: VAddr,
        arg: usize,
        stack_top: VAddr,
    ) {
        riscv64::thread::riscv_thread_init(thread, entry_point, arg, stack_top);
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
        riscv64::thread::riscv_context_switch(old_thread, new_thread);
    }

    unsafe fn current_sp() -> usize {
        let sp: usize;
        core::arch::asm!("mv {0}, sp", out(reg) sp);
        sp
    }

    unsafe fn set_sp(sp: usize) {
        core::arch::asm!("mv sp, {0}", in(reg) sp);
    }
}

// ============= ArchTimer Implementation =============

impl ArchTimer for Riscv64Arch {
    fn now_monotonic() -> u64 {
        unsafe {
            let time: u64;
            core::arch::asm!("rdtime {0}", out(reg) time);
            time
        }
    }

    fn set_timer(deadline: u64) {
        unsafe {
            registers::write_csr(registers::csr::STIMECMP, deadline);
        }
    }

    fn cancel_timer() {
        unsafe {
            // Set timer to max value (never fire)
            registers::write_csr(registers::csr::STIMECMP, u64::MAX);
        }
    }

    fn get_frequency() -> u64 {
        // TODO: Get timebase frequency from device tree
        // Default to 10 MHz for now
        10_000_000
    }
}

// ============= ArchInterrupts Implementation =============

impl ArchInterrupts for Riscv64Arch {
    unsafe fn enable_irq(irq: u32) {
        // Use PLIC (Platform-Local Interrupt Controller)
        // Get current hart (CPU) number
        let hart = riscv64::mp::riscv_get_cpu_num() as u32;
        riscv64::plic::plic_enable_irq(hart, irq);
    }

    unsafe fn disable_irq(irq: u32) {
        // Get current hart (CPU) number
        let hart = riscv64::mp::riscv_get_cpu_num() as u32;
        riscv64::plic::plic_disable_irq(hart, irq);
    }

    unsafe fn end_of_interrupt(irq: u32) {
        // Get current hart (CPU) number
        let hart = riscv64::mp::riscv_get_cpu_num() as u32;
        riscv64::plic::plic_complete(hart, irq);
    }

    fn interrupts_enabled() -> bool {
        unsafe {
            let sstatus: u64;
            core::arch::asm!("csrr {0}, sstatus", out(reg) sstatus);
            (sstatus & (1 << 1)) != 0 // SIE bit
        }
    }

    unsafe fn disable_interrupts() -> u64 {
        let sstatus: u64;
        core::arch::asm!("csrr {0}, sstatus; csrc sstatus, {1}",
                         out(reg) sstatus, in(reg) 2u64); // Clear SIE
        sstatus
    }

    unsafe fn restore_interrupts(state: u64) {
        if (state & (1 << 1)) != 0 {
            // Re-enable interrupts if they were enabled
            registers::set_csr(registers::csr::SSTATUS, 1 << 1);
        }
    }

    unsafe fn send_ipi(target_cpu: u32, vector: u32) -> i32 {
        riscv64::mp::riscv_send_ipi(target_cpu as usize);
        0 // OK
    }
}

// ============= ArchMMU Implementation =============

impl ArchMMU for Riscv64Arch {
    unsafe fn map(pa: PAddr, va: VAddr, len: usize, flags: u64) -> i32 {
        use crate::arch::riscv64::page_table::{kernel_as, flags::pte_flags};

        let aspace = kernel_as();
        let mut offset = 0;
        let page_count = (len + 4095) / 4096;

        for i in 0..page_count {
            let current_va = va + offset;
            let current_pa = pa + offset;
            let page_flags = if (flags & ArchMMUFlags::WRITE) != 0 {
                pte_flags::READ | pte_flags::WRITE | pte_flags::VALID
            } else if (flags & ArchMMUFlags::EXECUTE) != 0 {
                pte_flags::READ | pte_flags::EXECUTE | pte_flags::VALID
            } else {
                pte_flags::READ | pte_flags::VALID
            };

            match aspace.map_page(current_va, current_pa, page_flags, true) {
                Ok(()) => {},
                Err(_) => return -1, // Error
            }

            offset += 4096;
        }

        0 // OK
    }

    unsafe fn unmap(va: VAddr, len: usize) {
        use crate::arch::riscv64::page_table::kernel_as;

        let aspace = kernel_as();
        let page_count = (len + 4095) / 4096;

        for i in 0..page_count {
            let _ = aspace.unmap_page(va + (i * 4096));
        }
    }

    unsafe fn protect(va: VAddr, len: usize, flags: u64) -> i32 {
        use crate::arch::riscv64::page_table::{kernel_as, flags::pte_flags};

        let aspace = kernel_as();
        let page_count = (len + 4095) / 4096;

        let page_flags = if (flags & ArchMMUFlags::WRITE) != 0 {
            pte_flags::READ | pte_flags::WRITE | pte_flags::VALID
        } else if (flags & ArchMMUFlags::EXECUTE) != 0 {
            pte_flags::READ | pte_flags::EXECUTE | pte_flags::VALID
        } else {
            pte_flags::READ | pte_flags::VALID
        };

        for i in 0..page_count {
            let current_va = va + (i * 4096);
            // To change protection, we need to unmap and remap
            // For now, just return OK
            let _ = current_va;
            let _ = page_flags;
        }

        0 // OK
    }

    unsafe fn flush_tlb(va: VAddr, len: usize) {
        if len == 0 {
            riscv64::mmu::tlb_flush();
        } else {
            riscv64::mmu::tlb_flush_page(va);
        }
    }

    unsafe fn flush_tlb_all() {
        riscv64::mmu::tlb_flush();
    }

    unsafe fn is_valid_va(va: VAddr) -> bool {
        // RISC-V Sv39/Sv48 canonical address check
        // User space: bit 63 = 0, bits 62-48 must equal bit 47
        // Kernel space: bit 63 = 1, bits 62-48 must equal bit 47
        riscv64::mmu::is_valid_canonical_va(va)
    }

    unsafe fn virt_to_phys(va: VAddr) -> PAddr {
        use crate::arch::riscv64::page_table::kernel_as;

        let aspace = kernel_as();
        match aspace.translate(va) {
            Some(pa) => pa,
            None => {
                // Fall back to direct mapping for physical memory
                // This is a simplification for kernel mappings
                va as PAddr
            }
        }
    }

    unsafe fn phys_to_virt(pa: PAddr) -> VAddr {
        // TODO: Use proper physical mapping
        // For now, just return physical address
        pa as VAddr
    }
}

// ============= ArchCache Implementation =============

impl ArchCache for Riscv64Arch {
    unsafe fn clean_dcache(_addr: VAddr, _len: usize) {
        // RISC-V has no data cache clean instruction
        // Use fence for memory ordering
        core::arch::asm!("fence ow, ow");
    }

    unsafe fn invalidate_dcache(_addr: VAddr, _len: usize) {
        // RISC-V has no dcache invalidate instruction
        // Use fence for memory ordering
        core::arch::asm!("fence or, or");
    }

    unsafe fn clean_invalidate_dcache(_addr: VAddr, _len: usize) {
        // RISC-V has no combined clean/invalidate
        // Use fence for memory ordering
        core::arch::asm!("fence ow, ow");
    }

    unsafe fn sync_icache(addr: VAddr, len: usize) {
        // Use fence.i to synchronize instruction cache
        core::arch::asm!("fence.i");
    }

    fn dcache_line_size() -> usize {
        64 // Default cache line size for RISC-V
    }

    fn icache_line_size() -> usize {
        64 // Default cache line size for RISC-V
    }
}

// ============= ArchCpuId Implementation =============

impl ArchCpuId for Riscv64Arch {
    fn current_cpu() -> u32 {
        riscv64::mp::riscv_get_cpu_num()
    }

    fn cpu_count() -> u32 {
        riscv64::mp::riscv_num_online_cpus()
    }

    fn get_features() -> u64 {
        riscv64::feature::riscv_get_features()
    }
}

// ============= ArchMemoryBarrier Implementation =============

impl ArchMemoryBarrier for Riscv64Arch {
    fn mb() {
        unsafe { core::arch::asm!("fence", options(nostack, memory)); }
    }

    fn rmb() {
        unsafe { core::arch::asm!("fence ir, ir", options(nostack, memory)); }
    }

    fn wmb() {
        unsafe { core::arch::asm!("fence ow, ow", options(nostack, memory)); }
    }

    fn acquire() {
        unsafe { core::arch::asm!("fence r, rw", options(nostack, memory)); }
    }

    fn release() {
        unsafe { core::arch::asm!("fence rw, w", options(nostack, memory)); }
    }
}

// ============= ArchHalt Implementation =============

impl ArchHalt for Riscv64Arch {
    unsafe fn halt() {
        core::arch::asm!("wfi");
    }

    fn pause() {
        unsafe { core::arch::asm!("pause"); }
    }

    fn serialize() {
        unsafe {
            let mut _: u32;
            core::arch::asm!("fence", options(nostack));
        }
    }
}

// ============= ArchUserAccess Implementation =============

impl ArchUserAccess for Riscv64Arch {
    unsafe fn copy_from_user(dst: *mut u8, src: VAddr, len: usize) -> isize {
        riscv64::user_copy_c::riscv_copy_from_user(dst, src, len)
    }

    unsafe fn copy_to_user(dst: VAddr, src: *const u8, len: usize) -> isize {
        riscv64::user_copy_c::riscv_copy_to_user(dst, src, len)
    }

    fn is_user_address(addr: VAddr) -> bool {
        riscv64::user_copy_c::riscv_is_user_address(addr)
    }

    unsafe fn validate_user_range(addr: VAddr, len: usize, _write: bool) -> bool {
        // Check for overflow
        if addr.wrapping_add(len) < addr {
            return false;
        }

        // Validate the range
        riscv64::user_copy_c::riscv_user_access_verify(addr, len, false)
    }
}

// ============= ArchUserEntry Implementation =============

impl ArchUserEntry for Riscv64Arch {
    unsafe fn enter_userspace(arg1: usize, arg2: usize, sp: usize, pc: usize, flags: u64) -> ! {
        riscv64::uspace_entry_simple(sp, pc, arg1)
    }

    unsafe fn return_to_userspace(iframe: *mut ()) -> ! {
        riscv64::uspace_entry_exception_return(iframe)
    }
}

// ============= ArchDebug Implementation =============

impl ArchDebug for Riscv64Arch {
    fn read_perf_counter() -> u64 {
        unsafe {
            let mcycle: u64;
            core::arch::asm!("csrr {0}, mcycle", out(reg) mcycle);
            mcycle
        }
    }

    unsafe fn set_hw_breakpoint(addr: VAddr, kind: u32) -> i32 {
        // RISC-V debug breakpoints use triggers
        // TODO: Implement proper breakpoint support
        let _ = addr;
        let _ = kind;
        -1 // Not implemented yet
    }

    unsafe fn disable_hw_breakpoints() {
        // Clear tselect triggers
        // TODO: Implement proper breakpoint support
    }
}

// ============= ArchFpu Implementation =============

impl ArchFpu for Riscv64Arch {
    type FpuState = riscv64::fpu::FpuState;

    unsafe fn init() {
        riscv64::fpu::riscv_fpu_init();
    }

    unsafe fn save(state: *mut Self::FpuState) {
        riscv64::fpu::riscv_fpu_save(state);
    }

    unsafe fn restore(state: *const Self::FpuState) {
        riscv64::fpu::riscv_fpu_restore(state);
    }

    fn is_enabled() -> bool {
        riscv64::fpu::riscv_fpu_enabled()
    }

    unsafe fn enable() {
        riscv64::fpu::riscv_fpu_init();
    }

    unsafe fn disable() {
        riscv64::fpu::riscv_fpu_disable();
    }
}

// ============= Arch Marker Trait Implementation =============

impl Arch for Riscv64Arch {}
