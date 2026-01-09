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

// Page-related constants
const PAGE_SIZE_MMU: usize = 4096;
const PAGE_MASK_MMU: usize = 4095; // 0xFFF

// Page table indices calculation helpers
#[inline]
fn pml4_index(va: VAddr) -> usize {
    (va >> 39) & 0x1FF
}

#[inline]
fn pdp_index(va: VAddr) -> usize {
    (va >> 30) & 0x1FF
}

#[inline]
fn pd_index(va: VAddr) -> usize {
    (va >> 21) & 0x1FF
}

#[inline]
fn pt_index(va: VAddr) -> usize {
    (va >> 12) & 0x1FF
}

// Convert high-level flags to PTE flags
fn flags_to_pte_flags(flags: u64) -> u64 {
    use crate::kernel::arch::amd64::page_tables::mmu_flags::*;

    let mut pte_flags = X86_MMU_PG_P; // Present

    // Write permission
    if flags & 0x2 != 0 {
        pte_flags |= X86_MMU_PG_W;
    }

    // User permission
    if flags & 0x4 != 0 {
        pte_flags |= X86_MMU_PG_U;
    }

    // Global flag for kernel mappings
    if flags & 0x1 == 0 {
        // Kernel mapping
        pte_flags |= X86_MMU_PG_G;
    }

    // No-execute flag
    if flags & (1 << 63) != 0 {
        pte_flags |= X86_MMU_PG_NX;
    }

    pte_flags
}

impl ArchMMU for Amd64Arch {
    unsafe fn map(pa: PAddr, va: VAddr, len: usize, flags: u64) -> i32 {
        use crate::kernel::arch::amd64::page_tables::mmu_flags::*;

        // Align addresses to page boundaries
        let aligned_pa = pa & !(PAGE_MASK_MMU as PAddr);
        let aligned_va = va & !PAGE_MASK_MMU;

        // Calculate number of pages
        let num_pages = (len + PAGE_MASK_MMU) / PAGE_SIZE_MMU;

        // Get current CR3 (page table base)
        let cr3 = amd64::mmu::read_cr3();

        // Convert high-level flags to PTE flags
        let pte_flags = flags_to_pte_flags(flags);

        // For each page to map
        for i in 0..num_pages {
            let current_va = aligned_va + (i * PAGE_SIZE_MMU);
            let current_pa = (aligned_pa as u64 + (i * PAGE_SIZE_MMU) as u64) as PAddr;

            // Walk the page tables
            let pml4 = cr3 as *mut u64;

            // PML4 entry
            let pml4_idx = pml4_index(current_va);
            let pml4_entry = *pml4.add(pml4_idx);

            if pml4_entry & X86_MMU_PG_P == 0 {
                // PML4 entry not present - need to allocate PDP table
                // For now, fail with error
                return -1; // RX_ERR_NO_MEMORY
            }

            // Get PDP table
            let pdp = ((pml4_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pdp_idx = pdp_index(current_va);
            let pdp_entry = *pdp.add(pdp_idx);

            let pd: *mut u64;
            if pdp_entry & X86_MMU_PG_P == 0 {
                // PDP entry not present - need to allocate PD table
                // For now, fail with error
                return -1;
            }

            // Check if this is a 1GB huge page
            if pdp_entry & X86_MMU_PG_PS != 0 {
                // Already a huge page, skip for now
                continue;
            }

            pd = ((pdp_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pd_idx = pd_index(current_va);
            let pd_entry = *pd.add(pd_idx);

            let pt: *mut u64;
            if pd_entry & X86_MMU_PG_P == 0 {
                // PD entry not present - need to allocate PT table
                // For now, fail with error
                return -1;
            }

            // Check if this is a 2MB large page
            if pd_entry & X86_MMU_PG_PS != 0 {
                // Already a large page, skip for now
                continue;
            }

            pt = ((pd_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pt_idx = pt_index(current_va);

            // Set the PTE
            let new_pte = (current_pa & X86_PG_FRAME as PAddr) as u64 | pte_flags;
            *pt.add(pt_idx) = new_pte;
        }

        // Flush TLB for the mapped range
        amd64::mmu::x86_tlb_invalidate_page(aligned_va);

        0 // OK
    }

    unsafe fn unmap(va: VAddr, len: usize) {
        use crate::kernel::arch::amd64::page_tables::mmu_flags::*;

        // Align to page boundary
        let aligned_va = va & !PAGE_MASK_MMU;
        let num_pages = (len + PAGE_MASK_MMU) / PAGE_SIZE_MMU;

        // Get current CR3 (page table base)
        let cr3 = amd64::mmu::read_cr3();

        // For each page, clear the present bit
        for i in 0..num_pages {
            let vaddr = aligned_va + (i * PAGE_SIZE_MMU);

            // Walk the page tables
            let pml4 = cr3 as *mut u64;

            // PML4 entry
            let pml4_idx = pml4_index(vaddr);
            let pml4_entry = *pml4.add(pml4_idx);

            if pml4_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Get PDP table
            let pdp = ((pml4_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pdp_idx = pdp_index(vaddr);
            let pdp_entry = *pdp.add(pdp_idx);

            if pdp_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Get PD table
            let pd = ((pdp_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pd_idx = pd_index(vaddr);
            let pd_entry = *pd.add(pd_idx);

            if pd_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Check if this is a large page
            if pd_entry & X86_MMU_PG_PS != 0 {
                // 2MB large page - unmap the whole thing
                *pd.add(pd_idx) = 0;
                continue;
            }

            // Get PT table
            let pt = ((pd_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pt_idx = pt_index(vaddr);
            let pt_entry = *pt.add(pt_idx);

            if pt_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Clear the PTE
            *pt.add(pt_idx) = 0;
        }

        // Flush TLB to ensure changes take effect
        amd64::mmu::x86_tlb_invalidate_page(aligned_va);
    }

    unsafe fn protect(va: VAddr, len: usize, flags: u64) -> i32 {
        use crate::kernel::arch::amd64::page_tables::mmu_flags::*;

        // Align to page boundary
        let aligned_va = va & !PAGE_MASK_MMU;
        let num_pages = (len + PAGE_MASK_MMU) / PAGE_SIZE_MMU;

        // Convert high-level flags to PTE flags
        let pte_flags = flags_to_pte_flags(flags);

        // Get current CR3 (page table base)
        let cr3 = amd64::mmu::read_cr3();

        // For each page, update the protection flags
        for i in 0..num_pages {
            let vaddr = aligned_va + (i * PAGE_SIZE_MMU);

            // Walk the page tables
            let pml4 = cr3 as *mut u64;

            // PML4 entry
            let pml4_idx = pml4_index(vaddr);
            let pml4_entry = *pml4.add(pml4_idx);

            if pml4_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Get PDP table
            let pdp = ((pml4_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pdp_idx = pdp_index(vaddr);
            let pdp_entry = *pdp.add(pdp_idx);

            if pdp_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Get PD table
            let pd = ((pdp_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pd_idx = pd_index(vaddr);
            let pd_entry = *pd.add(pd_idx);

            if pd_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Check if this is a large page
            if pd_entry & X86_MMU_PG_PS != 0 {
                // 2MB large page - update protection
                let new_entry = (pd_entry & X86_LARGE_PAGE_FRAME) | pte_flags | X86_MMU_PG_PS;
                *pd.add(pd_idx) = new_entry;
                continue;
            }

            // Get PT table
            let pt = ((pd_entry & X86_PG_FRAME) as VAddr) as *mut u64;
            let pt_idx = pt_index(vaddr);
            let pt_entry = *pt.add(pt_idx);

            if pt_entry & X86_MMU_PG_P == 0 {
                continue; // Not mapped
            }

            // Update the PTE flags (preserve physical address)
            let new_pte = (pt_entry & X86_PG_FRAME) | pte_flags;
            *pt.add(pt_idx) = new_pte;
        }

        // Flush TLB to ensure changes take effect
        amd64::mmu::x86_tlb_invalidate_page(aligned_va);

        0 // OK
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

// Debug register indices
const DR0: usize = 0;
const DR1: usize = 1;
const DR2: usize = 2;
const DR3: usize = 3;
const DR6: usize = 6;
const DR7: usize = 7;

// Breakpoint kinds
const BP_KIND_EXECUTE: u32 = 0;
const BP_KIND_WRITE: u32 = 1;
const BP_KIND_IO_READ_WRITE: u32 = 2;
const BP_KIND_READ_WRITE: u32 = 3;

// Maximum number of hardware breakpoints (x86-64 supports 4)
const MAX_HW_BREAKPOINTS: usize = 4;

// Track which breakpoint slots are in use
static mut HW_BREAKPOINT_IN_USE: [bool; MAX_HW_BREAKPOINTS] = [false; MAX_HW_BREAKPOINTS];

impl ArchDebug for Amd64Arch {
    fn read_perf_counter() -> u64 {
        amd64::asm::rdtsc()
    }

    unsafe fn set_hw_breakpoint(addr: VAddr, kind: u32) -> i32 {
        // Find an available breakpoint slot
        let slot = match HW_BREAKPOINT_IN_USE.iter().position(|&used| !used) {
            Some(slot) => slot,
            None => return -1, // No available breakpoint slots
        };

        // Configure breakpoint in DR7
        // DR7 layout:
        // - Bits 0-1: L0/G0 (local/global enable for DR0)
        // - Bits 2-3: L1/G1 (local/global enable for DR1)
        // - Bits 4-5: L2/G2 (local/global enable for DR2)
        // - Bits 6-7: L3/G3 (local/global enable for DR3)
        // - Bits 16-17: R/W0 (read/write for DR0)
        // - Bits 18-19: R/W1 (read/write for DR1)
        // - Bits 20-21: R/W2 (read/write for DR2)
        // - Bits 22-23: R/W3 (read/write for DR3)
        // - Bits 24-25: LEN0 (length for DR0)
        // - Bits 26-27: LEN1 (length for DR1)
        // - Bits 28-29: LEN2 (length for DR2)
        // - Bits 30-31: LEN3 (length for DR3)

        let mut dr7: u64;
        core::arch::asm!("mov %%dr7, {}", out(reg) dr7, options(nomem));

        // Set breakpoint address in DR0-DR3
        let dr_reg = match slot {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            _ => return -1,
        };

        // Write breakpoint address
        match dr_reg {
            0 => core::arch::asm!("mov {}, %%dr0", in(reg) addr as u64, options(nostack)),
            1 => core::arch::asm!("mov {}, %%dr1", in(reg) addr as u64, options(nostack)),
            2 => core::arch::asm!("mov {}, %%dr2", in(reg) addr as u64, options(nostack)),
            3 => core::arch::asm!("mov {}, %%dr3", in(reg) addr as u64, options(nostack)),
            _ => unreachable!(),
        }

        // Set breakpoint type (R/W field)
        let rw_bits = match kind {
            BP_KIND_EXECUTE => 0b00,  // Execute
            BP_KIND_WRITE => 0b01,    // Write
            BP_KIND_IO_READ_WRITE => 0b10,  // I/O read/write
            BP_KIND_READ_WRITE => 0b11,    // Read/write
            _ => 0b00,  // Default to execute
        };

        // Set length to 1 byte (bits 00 = 1 byte)
        // Other options: 01 = 2 bytes, 10 = 8 bytes, 11 = 4 bytes
        let len_bits = 0b00;

        // Enable breakpoint (set local enable bit)
        dr7 |= 1 << (slot * 2);  // Set L0-L3
        dr7 &= !(0b11 << (16 + slot * 4));  // Clear R/W field
        dr7 |= (rw_bits as u64) << (16 + slot * 4);  // Set R/W field
        dr7 &= !(0b11 << (24 + slot * 4));  // Clear LEN field
        dr7 |= (len_bits as u64) << (24 + slot * 4);  // Set LEN field

        // Write DR7 to enable the breakpoint
        core::arch::asm!("mov {}, %%dr7", in(reg) dr7, options(nostack));

        // Mark slot as in use
        HW_BREAKPOINT_IN_USE[slot] = true;

        0 // Success
    }

    unsafe fn disable_hw_breakpoints() {
        // Clear all breakpoints by writing 0 to DR7
        core::arch::asm!("mov {}, %%dr7", in(reg) 0u64, options(nostack));

        // Mark all slots as available
        for slot in HW_BREAKPOINT_IN_USE.iter_mut() {
            *slot = false;
        }
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
