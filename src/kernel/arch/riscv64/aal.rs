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
use crate::arch::riscv64::registers;
use crate::rustux::types::*;

// Import RISC-V submodules
use crate::arch::riscv64::mmu;
use crate::arch::riscv64::mp;
use crate::arch::riscv64::plic;
use crate::arch::riscv64::feature;
use crate::arch::riscv64::user_copy_c;
use crate::arch::riscv64::fpu;
use crate::arch::riscv64::uspace_entry;

/// Marker type for RISC-V architecture
pub enum Riscv64Arch {}

// ============= ArchStartup Implementation =============

impl ArchStartup for Riscv64Arch {
    unsafe fn early_init() {
        // TODO: Implement RISC-V early initialization
        println!("RISC-V: Early init");
    }

    unsafe fn init_mmu() {
        // TODO: Implement paging enable
        println!("RISC-V: Init MMU");
    }

    unsafe fn init_exceptions() {
        // Exception vectors are installed in start.S
    }

    unsafe fn late_init() {
        // TODO: Implement RISC-V late initialization
        println!("RISC-V: Late init");
    }
}

// ============= ArchThreadContext Implementation =============

impl ArchThreadContext for Riscv64Arch {
    type Context = crate::arch::riscv64::exceptions_c::RiscvIframe;

    unsafe fn init_thread(
        thread: &mut crate::kernel::thread::Thread,
        entry_point: VAddr,
        arg: usize,
        stack_top: VAddr,
    ) {
        // TODO: Implement RISC-V thread initialization
        let _ = (thread, entry_point, arg, stack_top);
        println!("RISC-V: Thread init");
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
        // TODO: Implement RISC-V context switch
        let _ = (old_thread, new_thread);
        println!("RISC-V: Context switch");
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
        let hart = mp::riscv_get_cpu_num() as u32;
        plic::plic_enable_irq(hart, irq);
    }

    unsafe fn disable_irq(irq: u32) {
        // Get current hart (CPU) number
        let hart = mp::riscv_get_cpu_num() as u32;
        plic::plic_disable_irq(hart, irq);
    }

    unsafe fn end_of_interrupt(irq: u32) {
        // Get current hart (CPU) number
        let hart = mp::riscv_get_cpu_num() as u32;
        plic::plic_complete(hart, irq);
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
        mp::riscv_send_ipi(target_cpu as usize);
        0 // OK
    }
}

// ============= ArchMMU Implementation =============

impl ArchMMU for Riscv64Arch {
    unsafe fn map(pa: PAddr, va: VAddr, len: usize, flags: u64) -> i32 {
        use crate::arch::riscv64::mmu::pte_flags;
        use crate::arch::riscv64::page_table::kernel_as;

        let aspace = kernel_as();
        let mut offset = 0;
        let page_count = (len + 4095) / 4096;

        for i in 0..page_count {
            let current_va = va + offset;
            let current_pa = pa + offset as u64;
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
        use crate::arch::riscv64::mmu::pte_flags;
        use crate::arch::riscv64::page_table::kernel_as;

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
            mmu::tlb_flush();
        } else {
            mmu::tlb_flush_page(va);
        }
    }

    unsafe fn flush_tlb_all() {
        mmu::tlb_flush();
    }

    unsafe fn is_valid_va(va: VAddr) -> bool {
        // RISC-V Sv39/Sv48 canonical address check
        // User space: bit 63 = 0, bits 62-48 must equal bit 47
        // Kernel space: bit 63 = 1, bits 62-48 must equal bit 47
        mmu::is_valid_canonical_va(va)
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
        mp::riscv_get_cpu_num()
    }

    fn cpu_count() -> u32 {
        mp::riscv_num_online_cpus()
    }

    fn get_features() -> u64 {
        feature::riscv_get_features()
    }
}

// ============= ArchMemoryBarrier Implementation =============

impl ArchMemoryBarrier for Riscv64Arch {
    fn mb() {
        unsafe { core::arch::asm!("fence", options(nostack)); }
    }

    fn rmb() {
        unsafe { core::arch::asm!("fence ir, ir", options(nostack)); }
    }

    fn wmb() {
        unsafe { core::arch::asm!("fence ow, ow", options(nostack)); }
    }

    fn acquire() {
        unsafe { core::arch::asm!("fence r, rw", options(nostack)); }
    }

    fn release() {
        unsafe { core::arch::asm!("fence rw, w", options(nostack));
        }
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
            core::arch::asm!("fence", options(nostack));
        }
    }
}

// ============= ArchUserAccess Implementation =============

impl ArchUserAccess for Riscv64Arch {
    unsafe fn copy_from_user(dst: *mut u8, src: VAddr, len: usize) -> isize {
        user_copy_c::riscv_copy_from_user(dst, src, len)
    }

    unsafe fn copy_to_user(dst: VAddr, src: *const u8, len: usize) -> isize {
        user_copy_c::riscv_copy_to_user(dst, src, len)
    }

    fn is_user_address(addr: VAddr) -> bool {
        user_copy_c::riscv_is_user_address(addr)
    }

    unsafe fn validate_user_range(addr: VAddr, len: usize, _write: bool) -> bool {
        // Check for overflow
        if addr.wrapping_add(len) < addr {
            return false;
        }

        // Validate the range
        user_copy_c::riscv_user_access_verify(addr, len, false)
    }
}

// ============= ArchUserEntry Implementation =============

impl ArchUserEntry for Riscv64Arch {
    unsafe fn enter_userspace(arg1: usize, arg2: usize, sp: usize, pc: usize, flags: u64) -> ! {
        uspace_entry::riscv_uspace_entry_simple(sp, pc, arg1)
    }

    unsafe fn return_to_userspace(iframe: *mut ()) -> ! {
        uspace_entry::riscv_uspace_exception_return(iframe)
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
    type FpuState = fpu::FpuState;

    unsafe fn init() {
        fpu::riscv_fpu_init();
    }

    unsafe fn save(state: *mut Self::FpuState) {
        fpu::riscv_fpu_save(state);
    }

    unsafe fn restore(state: *const Self::FpuState) {
        fpu::riscv_fpu_restore(state);
    }

    fn is_enabled() -> bool {
        fpu::riscv_fpu_enabled()
    }

    unsafe fn enable() {
        fpu::riscv_fpu_init();
    }

    unsafe fn disable() {
        fpu::riscv_fpu_disable();
    }
}

// ============= Arch Marker Trait Implementation =============

impl Arch for Riscv64Arch {}
