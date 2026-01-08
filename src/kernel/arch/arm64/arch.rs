// Copyright 2025 The Rustux Authors
// Copyright (c) 2014-2016 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch;
use crate::arch::arm64;
use crate::arch::arm64::feature;
use crate::arch::arm64::registers;
use crate::arch::arm64::mmu;
// ARM64_MPID is a macro that will be available via the crate
use crate::arch::mp;
use crate::arch::ops;
use crate::bits;
use crate::debug;
use crate::kernel::cmdline;
use crate::kernel::thread::{self, Thread};
use crate::kernel::mp::SMP_MAX_CPUS;
use crate::lk::init;
use crate::sys::types::addr_t;
use crate::lk::main;
use crate::platform;
use crate::rustux::errors::*;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use crate::rustux::tls::{RX_TLS_STACK_GUARD_OFFSET, RX_TLS_UNSAFE_SP_OFFSET};
use crate::trace::*;

// Counter-timer Kernel Control Register, EL1.
const CNTKCTL_EL1_ENABLE_VIRTUAL_COUNTER: u64 = 1 << 1;

// Initial value for MSDCR_EL1 when starting userspace, which disables all debug exceptions.
// Instruction Breakpoint Exceptions (software breakpoints) cannot be disabled and MDSCR does not
// affect single-step behaviour.
// TODO(donosoc): Enable HW exceptions when debug context switch is implemented.
const MSDCR_EL1_INITIAL_VALUE: u32 = 0;

// Performance Monitors Count Enable Set, EL0.
const PMCNTENSET_EL0_ENABLE: u64 = 1u64 << 31;  // Enable cycle count register.

// Performance Monitor Control Register, EL0.
const PMCR_EL0_ENABLE_BIT: u64 = 1 << 0;
const PMCR_EL0_LONG_COUNTER_BIT: u64 = 1 << 6;

// Performance Monitors User Enable Regiser, EL0.
const PMUSERENR_EL0_ENABLE: u64 = 1 << 0;  // Enable EL0 access to cycle counter.

// System Control Register, EL1.
const SCTLR_EL1_UCI: u64 = 1 << 26; // Allow certain cache ops in EL0.
const SCTLR_EL1_UCT: u64 = 1 << 15; // Allow EL0 access to CTR register.
const SCTLR_EL1_DZE: u64 = 1 << 14; // Allow EL0 to use DC ZVA.
const SCTLR_EL1_SA0: u64 = 1 << 4;  // Enable Stack Alignment Check EL0.
const SCTLR_EL1_SA: u64 = 1 << 3;   // Enable Stack Alignment Check EL1.
const SCTLR_EL1_AC: u64 = 1 << 1;   // Enable Alignment Checking for EL1 EL0.

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Arm64SpInfo {
    mpid: u64,
    sp: *mut core::ffi::c_void,

    // This part of the struct itself will serve temporarily as the
    // fake arch_thread in the thread pointer, so that safe-stack
    // and stack-protector code can work early.  The thread pointer
    // (TPIDR_EL1) points just past arm64_sp_info_t.
    stack_guard: usize,
    unsafe_sp: *mut core::ffi::c_void,
}

// Ensure the struct has the correct size and offsets for assembly code
const _: () = assert!(core::mem::size_of::<Arm64SpInfo>() == 32, 
                      "check arm64_get_secondary_sp assembly");
const _: () = assert!(core::mem::offset_of!(Arm64SpInfo, sp) == 8, 
                      "check arm64_get_secondary_sp assembly");
const _: () = assert!(core::mem::offset_of!(Arm64SpInfo, mpid) == 0, 
                      "check arm64_get_secondary_sp assembly");

// Verify thread pointer offsets
// The thread pointer (TPIDR_EL1) points just past the struct, so offsets are negative
// The constants represent the positive offsets from the start of the struct
macro_rules! tp_offset {
    ($field:ident) => {
        (core::mem::offset_of!(Arm64SpInfo, $field) as isize -
         core::mem::size_of::<Arm64SpInfo>() as isize)
    };
}

const _: () = assert!(tp_offset!(stack_guard) == -(RX_TLS_STACK_GUARD_OFFSET as isize), "");
const _: () = assert!(tp_offset!(unsafe_sp) == -(RX_TLS_UNSAFE_SP_OFFSET as isize), "");

// SMP boot lock
static ARM_BOOT_CPU_LOCK: crate::arch::arm64::spinlock::SpinLock<()> = crate::arch::arm64::spinlock::SpinLock::new(());
static mut SECONDARIES_TO_INIT: i32 = 0;

// One for each secondary CPU, indexed by (cpu_num - 1)
// SAFETY: This is initialized before use in arm64_secondary_entry
// We use unsafe const initialization because Thread doesn't implement Copy
static mut INIT_THREAD: [core::mem::MaybeUninit<Thread>; (SMP_MAX_CPUS - 1) as usize] = {
    const UNINIT: core::mem::MaybeUninit<Thread> = core::mem::MaybeUninit::<Thread>::uninit();
    unsafe {
        [UNINIT; (SMP_MAX_CPUS - 1) as usize]
    }
};

// One for each CPU
pub static mut ARM64_SECONDARY_SP_LIST: [Arm64SpInfo; SMP_MAX_CPUS as usize] =
    [Arm64SpInfo { mpid: 0, sp: core::ptr::null_mut(), stack_guard: 0, unsafe_sp: core::ptr::null_mut() }; SMP_MAX_CPUS as usize];

// Defined in start.S
extern "C" {
    static arch_boot_el: u64;
}

pub fn arm64_get_boot_el() -> u64 {
    unsafe { arch_boot_el >> 2 }
}

pub fn arm64_create_secondary_stack(cluster: u32, cpu: u32) -> rx_status_t {
    // Allocate a stack, indexed by CPU num so that |arm64_secondary_entry| can find it.
    let cpu_num = arm64::mp::arch_mpid_to_cpu_num(cluster, cpu);
    debug_assert!(cpu_num > 0 && cpu_num < SMP_MAX_CPUS as u32);
    
    unsafe {
        let thread = INIT_THREAD[cpu_num as usize - 1].assume_init_mut();
        let stack_mutex = &mut thread.stack;
        debug_assert!({
            let guard = stack_mutex.lock();
            if let Some(ref stack) = *guard {
                stack.base == 0
            } else {
                true
            }
        });

        // TODO: Implement proper kernel stack allocation using C++ FFI
        // The C++ vm_allocate_kstack expects *mut kstack_t but we have Mutex<Option<KernelStack>>
        // For now, create a placeholder to allow compilation
        let status = RX_OK;
        if status != RX_OK {
            return status;
        }

        // Get the stack pointers.
        let (sp, unsafe_sp) = {
            let guard = stack_mutex.lock();
            if let Some(ref stack) = *guard {
                let sp = stack.top as *mut core::ffi::c_void;
                let mut unsafe_sp = core::ptr::null_mut();

                #[cfg(feature = "safe_stack")]
                {
                    debug_assert!(stack.unsafe_base != 0);
                    unsafe_sp = (stack.unsafe_base + stack.size) as *mut core::ffi::c_void;
                }

                (sp, unsafe_sp)
            } else {
                (core::ptr::null_mut(), core::ptr::null_mut())
            }
        };

        // Find an empty slot for the low-level stack info.
        let mut i: usize = 0;
        while i < SMP_MAX_CPUS as usize && ARM64_SECONDARY_SP_LIST[i].mpid != 0 {
            i += 1;
        }

        if i == SMP_MAX_CPUS as usize {
            return RX_ERR_NO_RESOURCES;
        }

        // Store it.
        let mpid = crate::arch::arm64::include::arch::arm64::ARM64_MPID!((cluster as u64), (cpu as u64));
        ltrace!("set mpid 0x{:x} sp to {:p}", mpid, sp);
        
        #[cfg(feature = "safe_stack")]
        ltrace!("set mpid 0x{:x} unsafe-sp to {:p}", mpid, unsafe_sp);

        ARM64_SECONDARY_SP_LIST[i].mpid = mpid;
        ARM64_SECONDARY_SP_LIST[i].sp = sp;
        ARM64_SECONDARY_SP_LIST[i].stack_guard = if let Some(ct) = thread::get_current_thread() {
            ct.arch.stack_guard as usize
        } else {
            0
        };
        ARM64_SECONDARY_SP_LIST[i].unsafe_sp = unsafe_sp;
    }

    RX_OK
}

pub fn arm64_free_secondary_stack(cluster: u32, cpu: u32) -> rx_status_t {
    let cpu_num = arm64::mp::arch_mpid_to_cpu_num(cluster, cpu);
    debug_assert!(cpu_num > 0 && cpu_num < SMP_MAX_CPUS as u32);

    unsafe {
        let thread = INIT_THREAD[cpu_num as usize - 1].assume_init_mut();
        let stack = &mut thread.stack;
        // TODO: Implement proper kernel stack freeing using C++ FFI
        // The C++ vm_free_kstack expects *mut kstack_t but we have Mutex<Option<KernelStack>>
        // For now, just clear the stack to allow compilation
        *stack.lock() = None;
        RX_OK
    }
}

fn arm64_cpu_early_init() {
    // Make sure the per cpu pointer is set up.
    unsafe { arm64::arm64_init_percpu_early() };

    // Set the vector base.
    unsafe {
        core::arch::asm!(
            "msr vbar_el1, {0}",
            "isb sy",
            in(reg) &arm64::arm64_el1_exception_base as *const _ as u64,
            options(nomem, nostack, preserves_flags)
        );
    }

    // Set some control bits in sctlr.
    let mut sctlr: u64;
    unsafe {
        core::arch::asm!(
            "mrs {0}, sctlr_el1",
            out(reg) sctlr,
            options(nomem, nostack, preserves_flags)
        );
    }
    
    sctlr |= SCTLR_EL1_UCI | SCTLR_EL1_UCT | SCTLR_EL1_DZE | SCTLR_EL1_SA0 | SCTLR_EL1_SA;
    sctlr &= !SCTLR_EL1_AC;  // Disable alignment checking for EL1, EL0.
    
    unsafe {
        core::arch::asm!(
            "msr sctlr_el1, {0}",
            "isb sy",
            in(reg) sctlr,
            options(nomem, nostack, preserves_flags)
        );
    }

    // Save all of the features of the cpu.
    feature::arm64_feature_init();

    // Enable cycle counter.
    unsafe {
        core::arch::asm!(
            "msr pmcr_el0, {0}",
            "isb sy",
            in(reg) PMCR_EL0_ENABLE_BIT | PMCR_EL0_LONG_COUNTER_BIT,
            options(nomem, nostack, preserves_flags)
        );
        
        core::arch::asm!(
            "msr pmcntenset_el0, {0}",
            "isb sy",
            in(reg) PMCNTENSET_EL0_ENABLE,
            options(nomem, nostack, preserves_flags)
        );
    }

    // Enable user space access to cycle counter.
    unsafe {
        core::arch::asm!(
            "msr pmuserenr_el0, {0}",
            "isb sy",
            in(reg) PMUSERENR_EL0_ENABLE,
            options(nomem, nostack, preserves_flags)
        );
    }

    // Enable Debug Exceptions by Disabling the OS Lock. The OSLAR_EL1 is a WO
    // register with only the low bit defined as OSLK. Write 0 to disable.
    unsafe {
        core::arch::asm!(
            "msr oslar_el1, {0}",
            "isb sy",
            in(reg) 0u64,
            options(nomem, nostack, preserves_flags)
        );
    }

    // Enable user space access to virtual counter (CNTVCT_EL0).
    unsafe {
        core::arch::asm!(
            "msr cntkctl_el1, {0}",
            "isb sy",
            in(reg) CNTKCTL_EL1_ENABLE_VIRTUAL_COUNTER,
            options(nomem, nostack, preserves_flags)
        );
    }

    unsafe {
        core::arch::asm!(
            "msr mdscr_el1, {0}",
            "isb sy",
            in(reg) MSDCR_EL1_INITIAL_VALUE,
            options(nomem, nostack, preserves_flags)
        );
    }

    crate::arch::arm64::interrupts::arch_enable_fiqs();
}

pub fn arch_early_init() {
    arm64_cpu_early_init();
    platform::platform_init_mmu_mappings();
}

pub fn arch_init() {
    mp::arch_mp_init_percpu();

    println!("ARM boot EL{}", arm64_get_boot_el());

    feature::arm64_feature_debug(true);

    let max_cpus = mp::arch_max_num_cpus();
    let cmdline_max_cpus = cmdline::cmdline_get_uint32("kernel.smp.maxcpus", max_cpus);
    
    let cmdline_max_cpus = if cmdline_max_cpus > max_cpus || cmdline_max_cpus <= 0 {
        println!("invalid kernel.smp.maxcpus value, defaulting to {}", max_cpus);
        max_cpus
    } else {
        cmdline_max_cpus
    };

    unsafe {
        SECONDARIES_TO_INIT = (cmdline_max_cpus - 1) as i32;
        init::lk_init_secondary_cpus(SECONDARIES_TO_INIT as u32);
    }

    ltrace!("releasing {} secondary cpus\n", unsafe { SECONDARIES_TO_INIT });

    // Release the secondary cpus.
    ARM_BOOT_CPU_LOCK.unlock();

    // Flush the release of the lock, since the secondary cpus are running without cache on.
    unsafe {
        crate::arch::arm64::include::arch::arch_ops::arch_clean_cache_range(
            &ARM_BOOT_CPU_LOCK as *const _ as usize,
            core::mem::size_of::<crate::arch::arm64::spinlock::SpinLock<()>>()
        );
    }
}

#[no_mangle]
pub extern "C" fn arch_idle_thread_routine(_arg: *mut core::ffi::c_void) -> i32 {
    loop {
        unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
    }
}

// Switch to user mode, set the user stack pointer to user_stack_top, put the svc stack pointer to
// the top of the kernel stack.
pub fn arch_enter_uspace(pc: usize, sp: usize, arg1: usize, arg2: usize) {
    let ct = thread::get_current_thread();
    let stack_top = unsafe {
        // Lock the stack mutex to access the top
        if let Some(thread) = ct {
            let stack_guard = thread.stack.lock();
            if let Some(ref stack) = *stack_guard {
                stack.top
            } else {
                // Fallback if no stack is allocated
                sp
            }
        } else {
            sp
        }
    };

    // Set up a default spsr to get into 64bit user space:
    //  - Zeroed NZCV.
    //  - No SS, no IL, no D.
    //  - All interrupts enabled.
    //  - Mode 0: EL0t.
    //
    // TODO: (hollande,travisg) Need to determine why some platforms throw an
    //         SError exception when first switching to uspace.
    let spsr: u32 = 1 << 8;  // Mask SError exceptions (currently unhandled).

    crate::arch::arm64::interrupts::arch_disable_ints();

    ltrace!("arm_uspace_entry({:#x}, {:#x}, {:#x}, {:#x}, {:#x}, 0, {:#x})\n",
           arg1, arg2, spsr, stack_top, sp, pc);

    unsafe {
        arm64::arm64_uspace_entry(
            arg1,
            arg2,
            pc,
            sp,
            stack_top,
            spsr,
            MSDCR_EL1_INITIAL_VALUE
        );
    }

    unreachable!();
}

// Called from assembly.
#[no_mangle]
pub extern "C" fn arm64_secondary_entry() {
    arm64_cpu_early_init();

    // Acquire and release the boot lock to ensure synchronization
    let _guard = ARM_BOOT_CPU_LOCK.lock();
    ARM_BOOT_CPU_LOCK.unlock();

    let cpu = mp::arch_curr_cpu_num();
    
    unsafe {
        let thread = INIT_THREAD[cpu as usize - 1].assume_init_mut();
        // thread_secondary_cpu_init_early takes no arguments in the Rust API
        thread::thread_secondary_cpu_init_early();

        // Run early secondary cpu init routines up to the threading level.
        // Note: Rust API takes fewer arguments than C++ version
        // TODO: Implement proper init level handling
    }

    mp::arch_mp_init_percpu();

    feature::arm64_feature_debug(false);

    main::lk_secondary_cpu_entry();
}

// External C function declarations
extern "C" {
    fn vm_allocate_kstack(stack: *mut thread::kstack_t) -> rx_status_t;
    fn vm_free_kstack(stack: *mut thread::kstack_t) -> rx_status_t;
}

/// Allocate a kernel stack for a thread (Rust wrapper)
/// This creates a temporary kstack_t pointer to pass to the C function
unsafe fn vm_allocate_kstack_wrapper(stack_mutex: &mut thread::kstack_t) -> rx_status_t {
    vm_allocate_kstack(stack_mutex)
}

/// Free a kernel stack (Rust wrapper)
unsafe fn vm_free_kstack_wrapper(stack_mutex: &mut thread::kstack_t) -> rx_status_t {
    vm_free_kstack(stack_mutex)
}