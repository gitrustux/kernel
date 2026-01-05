// Copyright 2025 The Rustux Authors
// Copyright (c) 2008-2014 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64;
use crate::arch::arm64::feature;
use crate::arch::arm64::interrupt;
use crate::arch::arm64::mp;
use crate::reg::*;
use crate::rustux::compiler::*;

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