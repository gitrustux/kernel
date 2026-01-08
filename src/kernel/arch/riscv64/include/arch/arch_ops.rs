// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit architecture operations

use crate::arch::riscv64;
use crate::arch::riscv64::feature;
use crate::arch::riscv64::interrupt;
use crate::arch::riscv64::mp;
use crate::reg::*;
use crate::rustux::compiler::*;

pub const ENABLE_CYCLE_COUNTER: u32 = 1;

/// Spin loop hint for RISC-V
#[inline(always)]
pub fn arch_spinloop_pause() {
    unsafe { core::arch::asm!("pause") }
}

/// Full memory barrier
#[inline(always)]
pub fn mb() {
    unsafe { core::arch::asm!("fence iorw, iorw") }
}

/// SMP memory barrier
#[inline(always)]
pub fn smp_mb() {
    unsafe { core::arch::asm!("fence iorw, iorw") }
}

/// Read the cycle counter (rdcycle)
#[inline(always)]
pub fn arch_cycle_count() -> u64 {
    let count: u64;
    unsafe {
        core::arch::asm!("rdcycle {}", out(reg) count);
    }
    count
}

/// Read the instruction counter (rdinstret)
#[inline(always)]
pub fn arch_instret_count() -> u64 {
    let count: u64;
    unsafe {
        core::arch::asm!("rdinstret {}", out(reg) count);
    }
    count
}

/// Get CPU features
#[inline(always)]
pub fn arch_cpu_features() -> u32 {
    unsafe { feature::riscv_features }
}

/// Get data cache line size
#[inline(always)]
pub fn arch_dcache_line_size() -> u32 {
    unsafe { feature::riscv_dcache_size }
}

/// Get instruction cache line size
#[inline(always)]
pub fn arch_icache_line_size() -> u32 {
    unsafe { feature::riscv_icache_size }
}

/// Log architecture-specific data for process creation.
/// This can only be called after the process has been created and before
/// it is running.
#[inline(always)]
pub fn arch_trace_process_create(_pid: u64, _satp: usize) {
    // nothing to do for RISC-V
}

/// Disable interrupts and return previous state
#[inline(always)]
pub fn arch_disable_ints() -> u64 {
    let flags: u64;
    unsafe {
        core::arch::asm!(
            "csrrci {flags}, mstatus, {mask}",
            "csrrci {tmp}, mstatus, 0",  // Ensure zero-extended
            flags = out(reg) _,
            tmp = out(reg) _,
            mask = const 0x8, // MIE bit
        );
    }
    0 // TODO: Proper implementation
}

/// Restore interrupt state
#[inline(always)]
pub fn arch_restore_ints(_flags: u64) {
    // TODO: Implement
}

/// Flush instruction cache
#[inline(always)]
pub fn arch_flush_insn_cache(vaddr: *mut u8, len: usize) {
    let mut addr = vaddr as usize;
    let end = addr + len;

    while addr < end {
        unsafe {
            core::arch::asm!("fence.i", options(nostack));
        }
        addr += arch_icache_line_size() as usize;
    }

    unsafe {
        core::arch::asm!("fence.i", options(nostack));
    }
}
