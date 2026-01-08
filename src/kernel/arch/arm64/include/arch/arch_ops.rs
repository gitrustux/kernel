// Copyright 2025 The Rustux Authors
// Copyright (c) 2008-2014 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64;
use crate::arch::arm64::feature;
use crate::arch::arm64::interrupts;
use crate::arch::arm64::mp;
use crate::reg::*;
use crate::rustux::compiler::*;
use crate::rustux::types::VAddr;
use crate::rustux::types::PAddr as paddr_t;

pub const ENABLE_CYCLE_COUNTER: u32 = 1;

#[inline(always)]
pub fn arch_spinloop_pause() {
    unsafe { core::arch::asm!("yield") }
}

#[inline(always)]
pub fn mb() {
    unsafe { core::arch::asm!("dsb sy") }
}

#[inline(always)]
pub fn smp_mb() {
    unsafe { core::arch::asm!("dmb sy") }
}

#[inline(always)]
pub fn arch_cycle_count() -> u64 {
    let count: u64;
    unsafe {
        core::arch::asm!("mrs {}, pmccntr_el0", out(reg) count);
    }
    count
}

#[inline(always)]
pub fn arch_cpu_features() -> u32 {
    unsafe { feature::arm64_features }
}

#[inline(always)]
pub fn arch_dcache_line_size() -> u32 {
    unsafe { feature::arm64_dcache_size }
}

#[inline(always)]
pub fn arch_icache_line_size() -> u32 {
    unsafe { feature::arm64_icache_size }
}

/// Log architecture-specific data for process creation.
/// This can only be called after the process has been created and before
/// it is running. Alas we can't use rx_koid_t here as the arch layer is at a
/// lower level than rustux.
#[inline(always)]
pub fn arch_trace_process_create(_pid: u64, _tt_phys: paddr_t) {
    // nothing to do
}

/// Get the current CPU number
#[inline(always)]
pub fn arch_curr_cpu_num() -> u32 {
    unsafe { mp::arch_curr_cpu_num() }
}

/// Enable IRQ interrupts
#[inline(always)]
pub fn arch_enable_ints() {
    interrupts::arch_enable_ints();
}

/// Disable IRQ interrupts
#[inline(always)]
pub fn arch_disable_ints() {
    interrupts::arch_disable_ints();
}

/// Check if IRQ interrupts are disabled
#[inline(always)]
pub fn arch_ints_disabled() -> bool {
    interrupts::arch_ints_disabled()
}

/// Save current interrupt state
#[inline(always)]
pub fn arch_save_ints() -> u64 {
    interrupts::arch_save_ints()
}

/// Restore interrupt state
#[inline(always)]
pub fn arch_restore_ints(state: u64) {
    interrupts::arch_restore_ints(state);
}

/// Check if an address is in user space
#[inline(always)]
pub fn arch_is_user_address(addr: VAddr) -> bool {
    // ARM64 user space is typically 0x0000_0000_0000_0000 to 0x0000_ffff_ffff_ffff
    // Kernel space starts at 0xffff_0000_0000_0000
    addr < 0x1_0000_0000_0000
}

/// Clean cache range (data cache clean by address)
#[inline(always)]
pub fn arch_clean_cache_range(_addr: VAddr, _len: usize) {
    // TODO: Implement cache clean
    unsafe {
        core::arch::asm!("dsb sy");
    }
}

/// Disable interrupts and return state
#[inline(always)]
pub fn arch_interrupt_save(_flags: u64) -> u64 {
    unsafe {
        let daif: u64;
        core::arch::asm!("mrs {}, daif", out(reg) daif, options(nomem, nostack));
        // Disable IRQs (set bit 7)
        core::arch::asm!("msr daifset, #2");
        daif
    }
}

/// Restore interrupt state
#[inline(always)]
pub fn arch_interrupt_restore(state: u64, _flags: u64) {
    unsafe {
        core::arch::asm!("msr daif, {}", in(reg) state);
    }
}