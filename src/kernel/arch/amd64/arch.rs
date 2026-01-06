// Copyright 2025 The Rustux Authors
// Copyright (c) 2009 Corey Tabaka
// Copyright (c) 2015 Intel Corporation
// Copyright (c) 2016 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 architecture-specific initialization and core functions

#![no_std]

use core::sync::atomic::{AtomicI32, Ordering};
use core::ptr;

use crate::kernel::arch::amd64;
use crate::kernel::arch::amd64::apic;
use crate::kernel::arch::amd64::descriptor;
use crate::kernel::arch::amd64::feature;
use crate::kernel::arch::amd64::mmu;
use crate::kernel::arch::amd64::mp;
use crate::kernel::arch::amd64::registers::*;
use crate::kernel::arch::amd64::tsc;
use crate::kernel::arch::amd64::uspace_entry;
use crate::kernel::debug;
use crate::println;
use crate::kernel::thread::Thread;
// use crate::lk::init;  // TODO: Implement lk module
// use crate::platform;  // TODO: Implement platform module

/// Save a pointer to the bootdata (ZBI), if present
pub static mut ZBI_BASE: *mut u8 = ptr::null_mut();

/// Early architecture initialization
///
/// This is called very early in the boot process, before the VM subsystem
/// is fully initialized.
pub fn arch_early_init() {
    mmu::x86_mmu_early_init();
}

/// Main architecture initialization
///
/// Called after the VM subsystem is up. Prints processor info and
/// initializes core architecture features.
pub fn arch_init() {
    let model = feature::x86_get_model();
    println!(
        "Processor Model Info: type {:#x} family {:#x} model {:#x} stepping {:#x}",
        model.processor_type,
        model.family,
        model.model,
        model.stepping
    );
    println!(
        "\tdisplay_family {:#x} display_model {:#x}",
        model.display_family,
        model.display_model
    );

    feature::x86_feature_debug();

    mmu::x86_mmu_init();

    descriptor::gdt_setup();
    descriptor::idt_setup_readonly();

    // x86_processor_trace_init() - TODO: Implement processor trace support
}

/// Enter userspace at the given entry point
///
/// # Safety
/// Caller must ensure all pointers are valid and the system is in a proper state
pub unsafe fn arch_enter_uspace(entry_point: usize, sp: usize, arg1: usize, arg2: usize) -> ! {
    println!("entry {:#x} user stack {:#x}", entry_point, sp);
    let kernel_sp = (*mp::x86_get_percpu()).default_tss.rsp0;
    println!("kernel stack {:#x}", kernel_sp);

    arch_disable_ints();

    // Default user space flags:
    // IOPL 0
    // Interrupts enabled
    const X86_FLAGS_IOPL_SHIFT: u64 = 12;
    const X86_FLAGS_IF: u64 = 1 << 9;
    let flags = (0 << X86_FLAGS_IOPL_SHIFT) | X86_FLAGS_IF;

    // Check that we're probably still pointed at the kernel gs
    let gs_base = read_msr(X86_MSR_IA32_GS_BASE);
    assert!(mmu::is_kernel_address(gs_base as usize), "GS base not in kernel space");

    // Check that the kernel stack is set properly
    assert!(
        mmu::is_kernel_address(kernel_sp as usize),
        "Kernel stack not in kernel space"
    );

    // Set up user's fs and gs base
    write_msr(X86_MSR_IA32_FS_BASE, 0);

    // Set the KERNEL_GS_BASE msr here, because we're going to swapgs below
    write_msr(X86_MSR_IA32_KERNEL_GS_BASE, 0);

    uspace_entry::x86_uspace_entry(arg1, arg2, sp, entry_point, flags);

    unreachable!();
}

/// Suspend the system
pub fn arch_suspend() {
    assert!(arch_ints_disabled());
    apic::apic_io_save();
    tsc::x86_tsc_store_adjustment();
}

/// Resume the system
pub fn arch_resume() {
    assert!(arch_ints_disabled());

    unsafe { mp::x86_init_percpu(0); }
    mmu::x86_mmu_percpu_init();
    mmu::x86_pat_sync(mp::cpu_num_to_mask(0));

    apic::apic_local_init();

    // Ensure the CPU that resumed was assigned the correct percpu object
    let percpu = unsafe { mp::x86_get_percpu() };
    assert!(apic::apic_local_id() == unsafe { (*percpu).apic_id }, "APIC ID mismatch");

    apic::apic_io_restore();
}

/// Finish secondary CPU entry
///
/// # Safety
/// Caller must ensure thread and cpu_num are valid
unsafe fn finish_secondary_entry(
    aps_still_booting: &AtomicI32,
    thread: &mut Thread,
    cpu_num: u32,
) -> ! {
    // Signal that this CPU is initialized. It is important that after this
    // operation, we do not touch any resources associated with bootstrap
    // besides our thread_t and stack, since this is the checkpoint the
    // bootstrap process uses to identify completion.
    let old_val = aps_still_booting.fetch_and(!(1 << cpu_num), Ordering::AcqRel);
    if old_val == 0 {
        // If the value is already zero, then booting this CPU timed out.
        halt_and_loop();
    }

    // Defer configuring memory settings until after the atomic_and above.
    // This ensures that we were in no-fill cache mode for the duration of early
    // AP init.
    let cr0 = x86_get_cr0();
    assert!(cr0 & X86_CR0_CD != 0, "Cache disabled bit not set");
    mmu::x86_mmu_percpu_init();

    // Load the appropriate PAT/MTRRs. This must happen after init_percpu, so
    // that this CPU is considered online.
    mmu::x86_pat_sync(1 << cpu_num);

    // Run early secondary cpu init routines up to the threading level
    // TODO: Implement lk_init_level
    // init::lk_init_level(
    //     init::LK_INIT_FLAG_SECONDARY_CPUS,
    //     init::LK_INIT_LEVEL_EARLIEST,
    //     init::LK_INIT_LEVEL_THREADING - 1,
    // );

    // TODO: thread_secondary_cpu_init_early(thread);
    // The thread stacks and struct are from a single allocation, free it
    // when we exit into the scheduler.
    // thread.flags |= THREAD_FLAG_FREE_STRUCT;

    // TODO: lk_secondary_cpu_entry();

    // If lk_secondary_cpu_entry returns, halt the core
    halt_and_loop();
}

#[inline]
fn halt_and_loop() -> ! {
    loop {
        unsafe {
            x86_hlt();
        }
    }
}

/// Secondary CPU entry point
///
/// This is called from assembly, before any other Rust code.
/// The %gs.base is not set up yet, so we must be careful not to
/// generate code that uses %gs before it's initialized.
///
/// # Safety
/// Called only from assembly during secondary CPU bringup
#[no_mangle]
pub unsafe extern "C" fn x86_secondary_entry(
    aps_still_booting: *const AtomicI32,
    thread: *mut Thread,
) -> ! {
    // Would prefer this to be in init_percpu, but there is a dependency on a
    // page mapping existing, and the BP calls that before the VM subsystem is
    // initialized.
    apic::apic_local_init();

    let local_apic_id = apic::apic_local_id();
    let cpu_num = mp::x86_apic_id_to_cpu_num(local_apic_id);
    if cpu_num < 0 {
        // If we could not find our CPU number, do not proceed further
        halt_and_loop();
    }

    let cpu_num = cpu_num as u32;

    assert!(cpu_num > 0, "Secondary CPU num must be > 0");

    // Set %gs.base to our percpu struct. This has to be done before
    // calling x86_init_percpu, which initializes most of that struct, so
    // that x86_init_percpu can use safe-stack and/or stack-protector code.
    // TODO: Implement percpu array for multiple CPUs
    // For now, we just use x86_get_percpu() which gets the current CPU's percpu via GS
    let _percpu = unsafe { mp::x86_get_percpu() };
    // write_msr(X86_MSR_IA32_GS_BASE, percpu as *const _ as u64);

    // Copy the stack-guard value from the boot CPU's percpu
    let bp_percpu = unsafe { mp::x86_get_percpu() };
    // (*percpu).stack_guard = (*bp_percpu).stack_guard;

    // TODO: Set up safe stack if enabled
    // #[cfg(feature = "safe_stack")]
    // {
    //     let unsafe_sp = (*thread).stack.unsafe_base + (*thread).stack.size;
    //     x86_write_gs_offset64(
    //         ZX_TLS_UNSAFE_SP_OFFSET,
    //         unsafe_sp & !0xf,  // Round down to 16-byte alignment
    //     );
    // }

    mp::x86_init_percpu(cpu_num);

    // Now do the rest of the work, in a function that is free to use %gs in its code.
    finish_secondary_entry(&*aps_still_booting, &mut *thread, cpu_num);
}

/// Register architecture-specific CPU commands with the console
///
/// This provides debug/test commands for CPU operations like feature display,
/// hotplug, and unplug.
pub fn register_cpu_commands() {
    // TODO: Implement command registration when console subsystem is ready
    // Commands:
    // - cpu features: Display CPU features
    // - cpu unplug <cpu_id>: Unplug a CPU
    // - cpu hotplug <cpu_id>: Hotplug a CPU
}

// MSR and CR register constants
pub const X86_MSR_IA32_GS_BASE: u32 = 0xC000_0101;
pub const X86_MSR_IA32_FS_BASE: u32 = 0xC000_0100;
pub const X86_MSR_IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;
pub const X86_CR0_CD: u64 = 1 << 30; // Cache disable
pub const X86_CR0_NW: u64 = 1 << 29; // Not-write-through

/// Disable interrupts
#[inline]
pub fn arch_disable_ints() -> u64 {
    unsafe { x86_cli() };
    0 // Return interrupt state (to be implemented)
}

/// Enable interrupts
#[inline]
pub fn arch_enable_ints() {
    unsafe { x86_sti() };
}

/// Check if interrupts are disabled
#[inline]
pub fn arch_ints_disabled() -> bool {
    // Read RFLAGS and check IF bit
    let rflags: u64;
    unsafe {
        core::arch::asm!("pushfq; pop {}", out(reg) rflags, options(nostack, nomem));
    }
    rflags & (1 << 9) == 0
}

// Export for use by other modules
pub use crate::kernel::arch::amd64::X86Iframe;
// pub use crate::kernel::arch::amd64::Tss64;  // TODO: Implement Tss64

/// Late architecture initialization
///
/// Called after threading and scheduler are initialized
pub fn arch_late_init() {
    // TODO: Implement late initialization
    // This typically includes:
    // - APIC timer setup
    // - Performance counter setup
    // - Additional per-CPU initialization
}

/// Initialize architecture-specific thread state
///
/// # Arguments
///
/// * `thread` - Thread to initialize
///
/// # Safety
///
/// thread must point to valid memory
pub unsafe fn arch_thread_initialize(thread: *mut Thread) {
    // TODO: Initialize arch-specific thread state
    let _ = thread;
}

/// Context switch to a new thread
///
/// # Arguments
///
/// * `old_thread` - Previous thread pointer
/// * `new_thread` - New thread to switch to
///
/// # Safety
///
/// Both pointers must be valid and this must only be called from proper context
pub unsafe fn arch_context_switch(old_thread: *mut Thread, new_thread: *mut Thread) {
    // TODO: Implement context switch
    let _ = old_thread;
    let _ = new_thread;
}

/// Check if an address is in user space
///
/// # Arguments
///
/// * `addr` - Virtual address to check
///
/// # Returns
///
/// true if the address is in user space
pub fn is_user_address(addr: usize) -> bool {
    // User space addresses are in the lower half (canonical)
    addr < 0x0000_8000_0000_0000
}

/// Platform IRQ handler (stub)
///
/// TODO: Implement proper platform IRQ handling
pub fn platform_irq(frame: *mut amd64::X86Iframe) {
    // Stub implementation - platform-specific IRQ handling
    // This would typically call into platform-specific code
    let _ = frame; // Suppress unused warning
}

/// Return from exception to user space
///
/// # Safety
///
/// Must only be called from exception context
pub unsafe fn x86_uspace_exception_return(_iframe: *mut ()) -> ! {
    // TODO: Implement proper exception return
    loop {
        core::hint::spin_loop();
    }
}
