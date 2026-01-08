// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! AMD64 architecture-specific functionality
//!
//! This module re-exports all the AMD64-specific modules.


use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::mem::MaybeUninit;
use core::arch::asm;

// Import types from parent module
use crate::kernel::arch::amd64::registers::X86GeneralRegs;

// Re-export all modules from the amd64 directory
pub mod acpi;
pub mod apic;
pub mod asm;
pub mod bootstrap16;
pub mod cpu_topology;
pub mod descriptor;
pub mod feature;
pub mod general_regs;
pub mod idt;
pub mod interrupts;
pub mod ioport;
pub mod mmu_mem_types;
pub mod mmu;
pub mod mp;
pub mod perf_mon;
pub mod proc_trace;
pub mod pvclock;
pub mod registers;
pub mod timer_freq;
pub mod tsc;
pub mod user_copy;
pub mod vmx_state;
pub mod x86intrin;

/// Represents the x86_64 interrupt frame structure.
#[repr(C)]
pub struct X86Iframe {
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rcx: u64,
    rax: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    vector: u64,
    err_code: u64,
    ip: u64,
    cs: u64,
    flags: u64,
    user_sp: u64,
    user_ss: u64,
}

pub type X86IframeT = X86Iframe;

/// Architecture-specific exception context.
pub struct ArchExceptionContext {
    is_page_fault: bool,
    frame: *const X86Iframe,
    cr2: u64,
}

/// Represents the context switch frame for x86_64.
#[repr(C)]
pub struct X86ContextSwitchFrame {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    rip: u64,
}

/// Function prototypes
extern "C" {
    fn x86_exception_handler(frame: *mut X86Iframe);
    fn platform_irq(frame: *mut X86Iframe);

    fn x86_64_context_switch(oldsp: *mut *const u64, newsp: *const u64);
    fn x86_uspace_entry(arg1: usize, arg2: usize, sp: usize, pc: usize, rflags: u64) -> !;
    fn x86_syscall();
    fn x86_syscall_process_pending_signals(gregs: *mut X86GeneralRegs);
    fn x86_init_smp(apic_ids: *const u32, num_cpus: u32);
    fn x86_bringup_aps(apic_ids: *const u32, count: u32) -> i32; // zx_status_t
}

/// I/O Bitmap
pub const IO_BITMAP_BITS: usize = 65536;
pub const IO_BITMAP_BYTES: usize = IO_BITMAP_BITS / 8;
pub const IO_BITMAP_LONGS: usize = IO_BITMAP_BITS / core::mem::size_of::<usize>();

/// x86-64 Task State Segment (TSS)
#[repr(C)]
#[derive(Debug)]
pub struct Tss64 {
    rsvd0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    rsvd1: u32,
    rsvd2: u32,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    rsvd3: u32,
    rsvd4: u32,
    rsvd5: u16,
    iomap_base: u16,
    tss_bitmap: [u8; IO_BITMAP_BYTES + 1],
}

#[inline]
pub unsafe fn x86_clts() {
    asm!("clts");
}

#[inline]
pub unsafe fn x86_hlt() {
    asm!("hlt");
}

#[inline]
pub unsafe fn x86_sti() {
    asm!("sti");
}

#[inline]
pub unsafe fn x86_cli() {
    asm!("cli");
}

#[inline]
pub unsafe fn x86_ltr(sel: u16) {
    asm!("ltr {}", in(reg) sel);
}

#[inline]
pub unsafe fn x86_lidt(base: usize) {
    asm!("lidt ({})", in(reg) base);
}

#[inline]
pub unsafe fn x86_lgdt(base: usize) {
    asm!("lgdt ({})", in(reg) base);
}

// I/O operations
#[inline]
pub unsafe fn inp(port: u16) -> u8 {
    let mut rv: u8 = MaybeUninit::uninit().assume_init();
    asm!("inb dx, al", in("dx") port, out("al") rv);
    rv
}

#[inline]
pub unsafe fn inpw(port: u16) -> u16 {
    let mut rv: u16 = MaybeUninit::uninit().assume_init();
    asm!("inw dx, ax", in("dx") port, out("ax") rv);
    rv
}

#[inline]
pub unsafe fn inpd(port: u16) -> u32 {
    let mut rv: u32 = MaybeUninit::uninit().assume_init();
    asm!("inl dx, eax", in("dx") port, out("eax") rv);
    rv
}

#[inline]
pub unsafe fn outp(port: u16, data: u8) {
    asm!("outb al, dx", in("al") data, in("dx") port);
}

#[inline]
pub unsafe fn outpw(port: u16, data: u16) {
    asm!("outw ax, dx", in("ax") data, in("dx") port);
}

#[inline]
pub unsafe fn outpd(port: u16, data: u32) {
    asm!("outl eax, dx", in("eax") data, in("dx") port);
}

#[inline]
pub unsafe fn rdtsc() -> u64 {
    let mut lo: u32;
    let mut hi: u32;
    asm!("rdtsc", out("eax") lo, out("edx") hi);
    ((hi as u64) << 32) | (lo as u64)
}

// CPUID operations
#[inline]
pub unsafe fn cpuid(sel: u32) -> (u32, u32, u32, u32) {
    let mut a: u32 = sel;
    let mut b: u32 = 0;
    let mut c: u32 = 0;
    let mut d: u32 = 0;
    asm!("cpuid",
         inout("eax") a,
         inout("ecx") c,
         inout("edx") d,
         lateout("esi") b
    );
    (a, b, c, d)
}

// Get CR register values
#[inline]
pub unsafe fn x86_get_cr0() -> u64 {
    let cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0);
    cr0
}

#[inline]
pub unsafe fn x86_set_cr0(in_val: u64) {
    asm!("mov cr0, {}", in(reg) in_val);
}

// Other functions
#[inline]
pub unsafe fn x86_is_paging_enabled() -> bool {
    x86_get_cr0() & (1 << 31) != 0
}
