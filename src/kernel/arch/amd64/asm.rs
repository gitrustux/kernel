// Copyright 2025 The Rustux Authors
// Copyright (c) 2015 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 low-level assembly operations
//!
//! This module provides functions that require inline assembly
//! for context switching, spinlocks, and other low-level operations.

#![feature(asm_const)]
#![feature(naked_functions)]

use crate::kernel::arch::amd64::mp;
use crate::rustux::types::*;
use core::arch::naked_asm;

/// Page size constant
const PAGE_SIZE: usize = 4096;

/// Context switch frame structure
///
/// This must match the layout expected by x86_64_context_switch
#[repr(C)]
pub struct X86ContextSwitchFrame {
    rbx: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}

/// Perform a context switch between threads
///
/// # Arguments
///
/// * `oldsp` - Pointer to save the old stack pointer
/// * `newsp` - New stack pointer to load
///
/// # Safety
///
/// Both pointers must be valid and the function must be called
/// with proper stack alignment
#[unsafe(naked)]
pub unsafe extern "C" fn x86_64_context_switch(oldsp: *mut u64, newsp: u64) {
    naked_asm!(
        // Save callee-saved registers
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Save old stack pointer and load new one
        "mov [rdi], rsp",
        "mov rsp, rsi",

        // Restore callee-saved registers from new stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",

        "ret"
    );
}

/// Acquire a spin lock
///
/// Uses the current CPU number + 1 as the lock value.
///
/// # Arguments
///
/// * `lock` - Pointer to the spin lock (u64)
///
/// # Safety
///
/// lock must point to valid memory
pub unsafe fn arch_spin_lock(lock: *mut u64) {
    // Get current CPU number and add 1
    let cpu_num = (mp::x86_get_cpuid() + 1) as u64;

    loop {
        // Try to acquire the lock using cmpxchg
        let prev = _cmpxchg64_rel(lock, cpu_num, 0);

        if prev == 0 {
            // We got the lock
            return;
        }

        // Lock is held, spin with pause
        loop {
            core::arch::asm!("pause");
            if lock.read_volatile() == 0 {
                break;
            }
        }
    }
}

/// Try to acquire a spin lock without spinning
///
/// # Arguments
///
/// * `lock` - Pointer to the spin lock (u64)
///
/// # Returns
///
/// true if the lock was acquired, false otherwise
///
/// # Safety
///
/// lock must point to valid memory
pub unsafe fn arch_spin_trylock(lock: *mut u64) -> bool {
    let cpu_num = (mp::x86_get_cpuid() + 1) as u64;
    let prev = _cmpxchg64_rel(lock, cpu_num, 0);

    prev == 0
}

/// Release a spin lock
///
/// # Arguments
///
/// * `lock` - Pointer to the spin lock (u64)
///
/// # Safety
///
/// lock must point to valid memory and must be held by the current CPU
pub unsafe fn arch_spin_unlock(lock: *mut u64) {
    lock.write_volatile(0);
}

/// Zero a page using rep stosq
///
/// # Arguments
///
/// * `addr` - Starting address of the page to zero
///
/// # Safety
///
/// addr must point to a valid page-aligned memory region
pub unsafe fn arch_zero_page(addr: *mut u8) {
    let count = PAGE_SIZE / 8;

    core::arch::asm!(
        "xor rax, rax",
        "cld",
        "rep stosq",
        in("rdi") addr,
        in("rcx") count,
        in("rax") 0u64,
        options(nostack)
    );
}

/// Load the startup IDT
///
/// # Safety
///
/// Must only be called during early boot
pub unsafe fn load_startup_idt() {
    extern "C" {
        #[link_name = "_idt_startup"]
        static IDT_STARTUP: [u8; 16 * 256];
    }

    let idt_ptr: u64;
    let limit: u16;

    core::arch::asm!(
        "lea rax, [{idt}]",
        "mov [rsp - 16], {limit}",
        "mov [rsp - 14], rax",
        "lidt [rsp - 16]",
        idt = sym IDT_STARTUP,
        limit = const (16 * 256) - 1,
        out("rax") idt_ptr,
        options(nostack)
    );
}

/// Spin lock type
pub type spin_lock_t = u64;

/// Read the TSC (Time Stamp Counter)
///
/// # Returns
///
/// The current TSC value
#[inline]
pub fn rdtsc() -> u64 {
    unsafe {
        let (low, high): (u32, u32);
        core::arch::asm!("rdtsc", lateout("eax") low, lateout("edx") high, options(nomem, nostack));
        ((high as u64) << 32) | (low as u64)
    }
}

/// Read the TSC with precise timestamping
///
/// # Returns
///
/// The current TSC value
#[inline]
pub fn rdtscp() -> u64 {
    unsafe {
        let (low, high): (u32, u32);
        core::arch::asm!("rdtscp", lateout("eax") low, lateout("edx") high, options(nomem, nostack));
        ((high as u64) << 32) | (low as u64)
    }
}

/// PAUSE instruction - hint to CPU that we're spinning
#[inline]
pub fn cpu_pause() {
    unsafe { core::arch::asm!("pause", options(nostack)) };
}

/// Serializing instruction - ensures all prior instructions complete
#[inline]
pub fn serialize() {
    unsafe {
        let _: u32;
        core::arch::asm!("cpuid", lateout("eax") _, lateout("ecx") _, options(nostack));
    }
}

/// Memory fence - compiler barrier
#[inline]
pub fn compiler_barrier() {
    unsafe { core::arch::asm!("", options(nostack, nomem)) };
}

/// Read a 64-bit MSR (Model Specific Register)
///
/// # Arguments
///
/// * `msr` - MSR index to read
///
/// # Returns
///
/// The value of the MSR
///
/// # Safety
///
/// msr must be a valid MSR for the current CPU
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let (low, high): (u32, u32);
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        lateout("eax") low,
        lateout("edx") high,
        options(nomem, nostack)
    );
    ((high as u64) << 32) | (low as u64)
}

/// Write a 64-bit MSR (Model Specific Register)
///
/// # Arguments
///
/// * `msr` - MSR index to write
/// * `value` - Value to write
///
/// # Safety
///
/// msr must be a valid writable MSR for the current CPU
#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nostack)
    );
}

/// Read CR2 control register (Page Fault Linear Address)
///
/// # Returns
///
/// The value of CR2
#[inline]
pub unsafe fn x86_get_cr2() -> u64 {
    let cr2: u64;
    core::arch::asm!("mov {0}, cr2", out(reg) cr2, options(nomem, nostack));
    cr2
}

/// Read CR3 control register (Page Directory Base Register)
///
/// # Returns
///
/// The value of CR3
#[inline]
pub unsafe fn x86_get_cr3() -> u64 {
    let cr3: u64;
    core::arch::asm!("mov {0}, cr3", out(reg) cr3, options(nomem, nostack));
    cr3
}

/// Write CR3 control register
///
/// # Arguments
///
/// * `value` - Value to write to CR3
#[inline]
pub unsafe fn x86_set_cr3(value: u64) {
    core::arch::asm!("mov cr3, {0}", in(reg) value, options(nostack));
}

/// Read CR4 control register
///
/// # Returns
///
/// The value of CR4
#[inline]
pub unsafe fn x86_get_cr4() -> u64 {
    let cr4: u64;
    core::arch::asm!("mov {0}, cr4", out(reg) cr4, options(nomem, nostack));
    cr4
}

/// Write CR4 control register
///
/// # Arguments
///
/// * `value` - Value to write to CR4
#[inline]
pub unsafe fn x86_set_cr4(value: u64) {
    core::arch::asm!("mov cr4, {0}", in(reg) value, options(nostack));
}

/// Read CR8 control register (Task Priority Register)
///
/// # Returns
///
/// The value of CR8
#[inline]
pub unsafe fn x86_get_cr8() -> u64 {
    let cr8: u64;
    core::arch::asm!("mov {0}, cr8", out(reg) cr8, options(nomem, nostack));
    cr8
}

/// Write CR8 control register
///
/// # Arguments
///
/// * `value` - Value to write to CR8 (0-15, TPR)
#[inline]
pub unsafe fn x86_set_cr8(value: u64) {
    core::arch::asm!("mov cr8, {0}", in(reg) value, options(nostack));
}

/// x86 CR4 flags
pub const X86_CR4_PGE: u64 = 1 << 7;  // Page Global Enable
pub const X86_CR4_PSE: u64 = 1 << 4;  // Page Size Extension
pub const X86_CR4_SMEP: u64 = 1 << 20; // Supervisor Mode Execution Protection
pub const X86_CR4_SMAP: u64 = 1 << 21; // Supervisor Mode Access Prevention

/// x86 RFLAGS
pub const X86_FLAGS_IF: u64 = 1 << 9;   // Interrupt Enable
pub const X86_FLAGS_AC: u64 = 1 << 18;  // Alignment Check

/// INVLPG instruction - invalidate a TLB entry
///
/// # Arguments
///
/// * `addr` - Virtual address to invalidate
#[inline]
pub unsafe fn invlpg(addr: VAddr) {
    core::arch::asm!("invlpg [{0}]", in(reg) addr, options(nostack));
}

/// Invalidate TLB entry for a specific page
///
/// # Arguments
///
/// * `addr` - Virtual address to invalidate
#[inline]
pub unsafe fn x86_tlb_invalidate_page(addr: VAddr) {
    invlpg(addr);
}

/// Global TLB invalidation
///
/// Invalidates all TLB entries except global pages
#[inline]
pub unsafe fn x86_tlb_global_invalidate() {
    // Reload CR3 to invalidate non-global TLB entries
    let cr3 = x86_get_cr3();
    x86_set_cr3(cr3);
}

/// Compare and exchange 64-bit with release semantics
///
/// This is a custom intrinsic that implements cmpxchg with release ordering.
///
/// # Arguments
///
/// * `ptr` - Pointer to the value to compare and swap
/// * `new` - New value to write if comparison succeeds
/// * `old` - Expected value to compare against
///
/// # Returns
///
/// The previous value at *ptr
///
/// # Safety
///
/// ptr must be aligned and point to valid memory
#[inline]
pub unsafe fn _cmpxchg64_rel(ptr: *mut u64, new: u64, old: u64) -> u64 {
    let prev: u64;
    core::arch::asm!(
        "lock cmpxchg [{ptr}], {new}",
        ptr = in(reg) ptr,
        new = in(reg) new,
        in("eax") old,
        lateout("rax") prev,
        options(nostack),
    );
    prev
}

/// Read debug status register (DR6)
///
/// # Returns
///
/// The value of DR6 debug status register
#[inline]
pub unsafe fn x86_read_debug_status() -> u64 {
    let dr6: u64;
    core::arch::asm!("mov {0}, dr6", out(reg) dr6, options(nomem, nostack));
    dr6
}
