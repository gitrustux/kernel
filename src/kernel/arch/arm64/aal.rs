// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 Architecture Abstraction Layer (AAL) implementation
//!
//! This module implements the AAL traits for ARM64, providing
//! the architecture-specific implementations of the common interfaces.


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
        // Call the MMU mapping function through address space
        use crate::arch::aspace::ArchAspace;

        // Get kernel address space
        let kernel_aspace = arm64::mmu::arm64_get_kernel_aspace();

        // Align to page size (4KB)
        const PAGE_SIZE: usize = 4096;
        let aligned_va = va & !(PAGE_SIZE - 1);
        let aligned_pa = pa & !(PAGE_SIZE as u64 - 1);
        let aligned_len = (len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        let num_pages = aligned_len / PAGE_SIZE;

        for i in 0..num_pages {
            let current_va = aligned_va + (i * PAGE_SIZE);
            let current_pa = (aligned_pa as u64 + (i * PAGE_SIZE) as u64) as PAddr;

            // Map page using address space
            match arm64::mmu::arm64_map_page(kernel_aspace, current_va, current_pa, flags) {
                0 => {}, // OK
                _ => return -1, // Error
            }
        }

        0 // OK
    }

    unsafe fn unmap(va: VAddr, len: usize) {
        use crate::arch::aspace::ArchAspace;

        // Get kernel address space
        let kernel_aspace = arm64::mmu::arm64_get_kernel_aspace();

        // Align to page size (4KB)
        const PAGE_SIZE: usize = 4096;
        let aligned_va = va & !(PAGE_SIZE - 1);
        let aligned_len = (len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        let num_pages = aligned_len / PAGE_SIZE;

        for i in 0..num_pages {
            let current_va = aligned_va + (i * PAGE_SIZE);
            // Unmap page
            let _ = arm64::mmu::arm64_unmap_page(kernel_aspace, current_va);
        }
    }

    unsafe fn protect(va: VAddr, len: usize, flags: u64) -> i32 {
        use crate::arch::aspace::ArchAspace;

        // Get kernel address space
        let kernel_aspace = arm64::mmu::arm64_get_kernel_aspace();

        // Align to page size (4KB)
        const PAGE_SIZE: usize = 4096;
        let aligned_va = va & !(PAGE_SIZE - 1);
        let aligned_len = (len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        let num_pages = aligned_len / PAGE_SIZE;

        for i in 0..num_pages {
            let current_va = aligned_va + (i * PAGE_SIZE);
            // Update protection - requires unmap and remap
            match arm64::mmu::arm64_protect_page(kernel_aspace, current_va, flags) {
                0 => {}, // OK
                _ => return -1, // Error
            }
        }

        0 // OK
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
        // Walk page tables to translate VA to PA
        use crate::arch::aspace::ArchAspace;

        let kernel_aspace = arm64::mmu::arm64_get_kernel_aspace();
        match arm64::mmu::arm64_translate(kernel_aspace, va) {
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

impl ArchCache for Arm64Arch {
    unsafe fn clean_dcache(addr: VAddr, len: usize) {
        // Clean data cache to point of coherency
        // ARM64 uses dc cvac instruction for cache clean
        let cache_line_size = arm64::include::arch::arch_ops::arch_dcache_line_size() as usize;
        let mut current = addr;
        let end = addr + len;

        while current < end {
            core::arch::asm!(
                "dc cvac, {0}",
                in(reg) current,
                options(nostack)
            );
            current += cache_line_size;
        }

        // Ensure the clean completes
        core::arch::asm!("dsb sy", options(nostack));
    }

    unsafe fn invalidate_dcache(addr: VAddr, len: usize) {
        // Invalidate data cache
        // ARM64 uses dc ivac instruction for cache invalidate
        let cache_line_size = arm64::include::arch::arch_ops::arch_dcache_line_size() as usize;
        let mut current = addr;
        let end = addr + len;

        while current < end {
            core::arch::asm!(
                "dc ivac, {0}",
                in(reg) current,
                options(nostack)
            );
            current += cache_line_size;
        }

        // Ensure the invalidate completes
        core::arch::asm!("dsb sy", options(nostack));
    }

    unsafe fn clean_invalidate_dcache(addr: VAddr, len: usize) {
        // Clean and invalidate data cache
        // ARM64 uses dc civac instruction for cache clean+invalidate
        let cache_line_size = arm64::include::arch::arch_ops::arch_dcache_line_size() as usize;
        let mut current = addr;
        let end = addr + len;

        while current < end {
            core::arch::asm!(
                "dc civac, {0}",
                in(reg) current,
                options(nostack)
            );
            current += cache_line_size;
        }

        // Ensure the operation completes
        core::arch::asm!("dsb sy", options(nostack));
    }

    unsafe fn sync_icache(addr: VAddr, len: usize) {
        // Synchronize instruction cache
        // Clean data cache first, then invalidate instruction cache
        Self::clean_dcache(addr, len);

        let cache_line_size = arm64::include::arch::arch_ops::arch_icache_line_size() as usize;
        let mut current = addr;
        let end = addr + len;

        while current < end {
            core::arch::asm!(
                "ic ivau, {0}",
                in(reg) current,
                options(nostack)
            );
            current += cache_line_size;
        }

        // Ensure the synchronization completes
        core::arch::asm!("dsb sy", options(nostack));
        core::arch::asm!("isb", options(nostack));
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
    unsafe fn enter_userspace(arg1: usize, arg2: usize, sp: usize, pc: usize, _flags: u64) -> ! {
        // ARM64 uses eret to return to user space
        // Note: arch_enter_uspace signature is (pc, sp, arg1, arg2)
        arm64::arch_enter_uspace(pc as u64, sp as u64, arg1 as u64, arg2 as u64)
    }

    unsafe fn return_to_userspace(_iframe: *mut ()) -> ! {
        // ARM64 arch_uspace_exception_return takes no arguments
        arm64::arch_uspace_exception_return()
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
        // ARM64 hardware breakpoints use DBGBCRn_EL1 and DBGBVRn_EL1 registers
        // There are typically up to 16 breakpoint registers (DBGBCR0_EL1 to DBGBCR15_EL1)

        // Find a free breakpoint slot
        const MAX_BREAKPOINTS: usize = 16;
        static mut BREAKPOINT_IN_USE: [bool; MAX_BREAKPOINTS] = [false; MAX_BREAKPOINTS];

        let slot = match BREAKPOINT_IN_USE.iter().position(|&used| !used) {
            Some(slot) => slot,
            None => return -1, // No free slots
        };

        // Configure breakpoint control register (DBGBCRn_EL1)
        // Bits:
        //   [0] - E (Enable) = 1
        //   [1:2] - SSC (Security State Control)
        //   [3:4] - PMC (Privilege Mode Control)
        //   [5:8] - BST (Byte Select Target)
        //   [9] - HMC (Higher mode control)
        //   [12] - BAS (Byte Address Select) for unaligned
        //   [13:14] - LBN (Linked Breakpoint Number)
        //   [15:16] - TYPE (0 = unlinked address, 1 = linked address, etc.)
        //   [20:21] - MATCH (1 = AArch32, 0 = AArch64)
        let mut dbgbcr: u64 = 0;
        dbgbcr |= 1 << 0;  // Enable breakpoint

        // Set breakpoint type based on kind
        // kind 0 = execute, 1 = load, 2 = store
        match kind {
            0 => dbgbcr |= (0b0000) << 20, // Execute (AArch64)
            1 => dbgbcr |= (0b0010) << 20, // Load
            2 => dbgbcr |= (0b0011) << 20, // Store
            _ => return -1, // Invalid kind
        }

        dbgbcr |= (0b11) << 3;  // EL1 and EL0

        // Write breakpoint control register
        match slot {
            0 => core::arch::asm!("msr dbgbcr0_el1, {}", in(reg) dbgbcr, options(nostack)),
            1 => core::arch::asm!("msr dbgbcr1_el1, {}", in(reg) dbgbcr, options(nostack)),
            2 => core::arch::asm!("msr dbgbcr2_el1, {}", in(reg) dbgbcr, options(nostack)),
            3 => core::arch::asm!("msr dbgbcr3_el1, {}", in(reg) dbgbcr, options(nostack)),
            4 => core::arch::asm!("msr dbgbcr4_el1, {}", in(reg) dbgbcr, options(nostack)),
            5 => core::arch::asm!("msr dbgbcr5_el1, {}", in(reg) dbgbcr, options(nostack)),
            6 => core::arch::asm!("msr dbgbcr6_el1, {}", in(reg) dbgbcr, options(nostack)),
            7 => core::arch::asm!("msr dbgbcr7_el1, {}", in(reg) dbgbcr, options(nostack)),
            8 => core::arch::asm!("msr dbgbcr8_el1, {}", in(reg) dbgbcr, options(nostack)),
            9 => core::arch::asm!("msr dbgbcr9_el1, {}", in(reg) dbgbcr, options(nostack)),
            10 => core::arch::asm!("msr dbgbcr10_el1, {}", in(reg) dbgbcr, options(nostack)),
            11 => core::arch::asm!("msr dbgbcr11_el1, {}", in(reg) dbgbcr, options(nostack)),
            12 => core::arch::asm!("msr dbgbcr12_el1, {}", in(reg) dbgbcr, options(nostack)),
            13 => core::arch::asm!("msr dbgbcr13_el1, {}", in(reg) dbgbcr, options(nostack)),
            14 => core::arch::asm!("msr dbgbcr14_el1, {}", in(reg) dbgbcr, options(nostack)),
            15 => core::arch::asm!("msr dbgbcr15_el1, {}", in(reg) dbgbcr, options(nostack)),
            _ => return -1,
        }

        // Write breakpoint value register (DBGBVRn_EL1) with the address
        match slot {
            0 => core::arch::asm!("msr dbgbvr0_el1, {}", in(reg) addr as u64, options(nostack)),
            1 => core::arch::asm!("msr dbgbvr1_el1, {}", in(reg) addr as u64, options(nostack)),
            2 => core::arch::asm!("msr dbgbvr2_el1, {}", in(reg) addr as u64, options(nostack)),
            3 => core::arch::asm!("msr dbgbvr3_el1, {}", in(reg) addr as u64, options(nostack)),
            4 => core::arch::asm!("msr dbgbvr4_el1, {}", in(reg) addr as u64, options(nostack)),
            5 => core::arch::asm!("msr dbgbvr5_el1, {}", in(reg) addr as u64, options(nostack)),
            6 => core::arch::asm!("msr dbgbvr6_el1, {}", in(reg) addr as u64, options(nostack)),
            7 => core::arch::asm!("msr dbgbvr7_el1, {}", in(reg) addr as u64, options(nostack)),
            8 => core::arch::asm!("msr dbgbvr8_el1, {}", in(reg) addr as u64, options(nostack)),
            9 => core::arch::asm!("msr dbgbvr9_el1, {}", in(reg) addr as u64, options(nostack)),
            10 => core::arch::asm!("msr dbgbvr10_el1, {}", in(reg) addr as u64, options(nostack)),
            11 => core::arch::asm!("msr dbgbvr11_el1, {}", in(reg) addr as u64, options(nostack)),
            12 => core::arch::asm!("msr dbgbvr12_el1, {}", in(reg) addr as u64, options(nostack)),
            13 => core::arch::asm!("msr dbgbvr13_el1, {}", in(reg) addr as u64, options(nostack)),
            14 => core::arch::asm!("msr dbgbvr14_el1, {}", in(reg) addr as u64, options(nostack)),
            15 => core::arch::asm!("msr dbgbvr15_el1, {}", in(reg) addr as u64, options(nostack)),
            _ => return -1,
        }

        // Enable debug exceptions in MDSCR_EL1
        let mut mdscr: u64;
        core::arch::asm!("mrs {}, mdscr_el1", out(reg) mdscr);
        mdscr |= 1 << 12;  // Enable breakpoints
        core::arch::asm!("msr mdscr_el1, {}", in(reg) mdscr, options(nostack));

        BREAKPOINT_IN_USE[slot] = true;
        0 // Success
    }

    unsafe fn disable_hw_breakpoints() {
        // Clear all hardware breakpoints
        const MAX_BREAKPOINTS: usize = 16;

        for i in 0..MAX_BREAKPOINTS {
            let zero: u64 = 0;
            match i {
                0 => core::arch::asm!("msr dbgbcr0_el1, {}", in(reg) zero, options(nostack)),
                1 => core::arch::asm!("msr dbgbcr1_el1, {}", in(reg) zero, options(nostack)),
                2 => core::arch::asm!("msr dbgbcr2_el1, {}", in(reg) zero, options(nostack)),
                3 => core::arch::asm!("msr dbgbcr3_el1, {}", in(reg) zero, options(nostack)),
                4 => core::arch::asm!("msr dbgbcr4_el1, {}", in(reg) zero, options(nostack)),
                5 => core::arch::asm!("msr dbgbcr5_el1, {}", in(reg) zero, options(nostack)),
                6 => core::arch::asm!("msr dbgbcr6_el1, {}", in(reg) zero, options(nostack)),
                7 => core::arch::asm!("msr dbgbcr7_el1, {}", in(reg) zero, options(nostack)),
                8 => core::arch::asm!("msr dbgbcr8_el1, {}", in(reg) zero, options(nostack)),
                9 => core::arch::asm!("msr dbgbcr9_el1, {}", in(reg) zero, options(nostack)),
                10 => core::arch::asm!("msr dbgbcr10_el1, {}", in(reg) zero, options(nostack)),
                11 => core::arch::asm!("msr dbgbcr11_el1, {}", in(reg) zero, options(nostack)),
                12 => core::arch::asm!("msr dbgbcr12_el1, {}", in(reg) zero, options(nostack)),
                13 => core::arch::asm!("msr dbgbcr13_el1, {}", in(reg) zero, options(nostack)),
                14 => core::arch::asm!("msr dbgbcr14_el1, {}", in(reg) zero, options(nostack)),
                15 => core::arch::asm!("msr dbgbcr15_el1, {}", in(reg) zero, options(nostack)),
                _ => {}
            }
        }

        // Disable debug exceptions in MDSCR_EL1
        let mut mdscr: u64;
        core::arch::asm!("mrs {}, mdscr_el1", out(reg) mdscr);
        mdscr &= !(1 << 12);  // Disable breakpoints
        core::arch::asm!("msr mdscr_el1, {}", in(reg) mdscr, options(nostack));
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
