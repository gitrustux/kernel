// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Symmetric Multi-Processing (SMP) support
//!
//! This module handles bringing up additional CPU cores (APs)
//! and managing SMP initialization.


use crate::kernel::arch::amd64;
use crate::kernel::arch::amd64::apic;
use crate::kernel::arch::amd64::apic::apic_send_ipi;
use crate::kernel::arch::amd64::bootstrap16;
use crate::kernel::arch::amd64::mp;
use crate::kernel::debug;
use crate::print;
use crate::println;
use crate::kernel::thread::Thread;
use crate::rustux::types::*;
use core::sync::atomic::{AtomicI32, Ordering};
use core::ptr;

/// IPI delivery modes
const DELIVERY_MODE_INIT: u8 = 5;
const DELIVERY_MODE_STARTUP: u8 = 6;

/// Initialize SMP for the given number of CPUs
///
/// # Arguments
///
/// * `apic_ids` - Array of APIC IDs for the CPUs
/// * `num_cpus` - Number of CPUs to initialize
pub fn x86_init_smp(apic_ids: &[u32], num_cpus: u32) {
    assert!(num_cpus <= u8::MAX as u32);

    let status = unsafe { x86_allocate_ap_structures(apic_ids.as_ptr(), num_cpus as u8) };
    if status != 0 {
        println!("Failed to allocate structures for APs");
        return;
    }

    unsafe { lk_init_secondary_cpus(num_cpus - 1) };
}

/// Bring up the AP (Application Processor) cores
///
/// # Arguments
///
/// * `apic_ids` - Array of APIC IDs for the CPUs to bring up
/// * `count` - Number of CPUs to bring up
///
/// # Returns
///
/// Status code: 0 for success, negative for error
pub fn x86_bringup_aps(apic_ids: &[u32], count: u32) -> i32 {
    // If being asked to bring up 0 cpus, move on
    if count == 0 {
        return 0; // ZX_OK
    }

    // Sanity check the given ids
    let mut aps_still_booting: AtomicI32 = AtomicI32::new(0);

    for i in 0..count as usize {
        let cpu = unsafe { x86_apic_id_to_cpu_num(apic_ids[i]) };
        if cpu <= 0 {
            return -3; // ZX_ERR_INVALID_ARGS
        }

        // Check if CPU is already online
        let cpu = cpu as u32;
        if unsafe { mp_is_cpu_online(cpu) } {
            return -9; // ZX_ERR_BAD_STATE
        }

        aps_still_booting.fetch_or(1 << cpu, Ordering::SeqCst);
    }

    // Acquire bootstrap16 memory for AP startup code
    let (bootstrap_aspace, bootstrap_data, bootstrap_instr_ptr) =
        match unsafe { x86_bootstrap16_acquire(x86_secondary_cpu_long_mode_entry as usize) } {
            Some(data) => data,
            None => return -1, // ZX_ERR_NO_MEMORY
        };

    unsafe {
        (*bootstrap_data).cpu_id_counter = 0;
        (*bootstrap_data).cpu_waiting_mask = &aps_still_booting as *const _ as *mut _;

        // Zero the kstack list
        for i in 0..count as usize {
            (*bootstrap_data).per_cpu[i].kstack_base = ptr::null_mut();
            (*bootstrap_data).per_cpu[i].thread = ptr::null_mut();
        }

        // Allocate kstacks and threads for all processors
        for i in 0..count as usize {
            let thread_ptr = unsafe { allocate_thread() };
            if thread_ptr.is_null() {
                // Cleanup and return error
                cleanup_bootstrap_threads(bootstrap_data, i);
                x86_bootstrap16_release(bootstrap_data);
                return -2; // ZX_ERR_NO_MEMORY
            }

            let stack_base = unsafe { vm_allocate_kstack(thread_ptr) };
            (*bootstrap_data).per_cpu[i].kstack_base = stack_base;
            (*bootstrap_data).per_cpu[i].thread = thread_ptr;
        }
    }

    // Memory fence to ensure all writes are visible to APs
    core::sync::atomic::fence(Ordering::SeqCst);

    print!("booting apic ids: ");
    for i in 0..count as usize {
        print!("{:#x} ", apic_ids[i]);
        unsafe { apic_send_ipi(0, apic_ids[i], DELIVERY_MODE_INIT) };
    }
    println!("");

    // Wait 10 ms and then send the startup signals
    unsafe { thread_sleep_relative(10) };

    // Calculate startup vector
    assert!(bootstrap_instr_ptr < 1024 * 1024);
    assert!(bootstrap_instr_ptr & 0xFFF == 0);
    let vec = (bootstrap_instr_ptr >> 12) as u8;

    // Try up to two times per CPU, as Intel recommends
    for _try in 0..2 {
        for i in 0..count as usize {
            let apic_id = apic_ids[i];

            // This will cause the APs to begin executing at bootstrap_instr_ptr
            unsafe { apic_send_ipi(vec as u32, apic_id, DELIVERY_MODE_STARTUP) };
        }

        if aps_still_booting.load(Ordering::SeqCst) == 0 {
            break;
        }

        // Wait 1ms for cores to boot
        unsafe { thread_sleep_relative(1) };
    }

    // Wait up to 1 second for cores to boot
    for _tries_left in 0..200 {
        if aps_still_booting.load(Ordering::SeqCst) == 0 {
            break;
        }
        unsafe { thread_sleep_relative(5) };
    }

    // Check for failed APs
    let failed_aps = aps_still_booting.swap(0, Ordering::SeqCst) as u32;
    if failed_aps != 0 {
        println!("Failed to boot CPUs: mask {:#x}", failed_aps);

        for i in 0..count as usize {
            let cpu = unsafe { x86_apic_id_to_cpu_num(apic_ids[i]) } as u32;
            let mask = 1 << cpu;

            if (failed_aps & mask) == 0 {
                continue;
            }

            // Shut the failed AP down
            unsafe { apic_send_ipi(0, apic_ids[i], DELIVERY_MODE_INIT) };

            // It shouldn't have been in the scheduler
            assert!(!unsafe { mp_is_cpu_active(cpu) });

            // Make sure CPU is not marked online
            unsafe {
                let online_cpus = mp_get_online_cpus();
                *online_cpus &= !mask;
            }

            // Free the failed AP's thread and stack
            unsafe {
                free_thread_and_stack((*bootstrap_data).per_cpu[i].thread);
                (*bootstrap_data).per_cpu[i].thread = ptr::null_mut();
            }
        }

        unsafe {
            x86_bootstrap16_release(bootstrap_data);
        }

        return -4; // ZX_ERR_TIMED_OUT
    }

    // Success - cleanup temporary structures
    unsafe {
        x86_bootstrap16_release(bootstrap_data);
    }

    0 // ZX_OK
}

/// Initialize secondary CPUs
///
/// # Arguments
///
/// * `count` - Number of secondary CPUs to initialize
unsafe fn lk_init_secondary_cpus(count: u32) {
    // TODO: Implement lk_init_secondary_cpus
    // This should initialize the thread structures for secondary CPUs
}

/// Clean up bootstrap threads on error
///
/// # Safety
///
/// bootstrap_data and count must be valid
unsafe fn cleanup_bootstrap_threads(bootstrap_data: *mut BootstrapData, count: usize) {
    for i in 0..count {
        let thread = (*bootstrap_data).per_cpu[i].thread;
        if !thread.is_null() {
            free_thread_and_stack(thread);
        }
    }
}

/// Allocate a thread structure
unsafe fn allocate_thread() -> *mut Thread {
    use alloc::alloc::{alloc_zeroed, Layout};

    // Create a layout for the Thread structure
    let layout = Layout::new::<Thread>();
    if layout.size() == 0 {
        return core::ptr::null_mut();
    }

    let ptr = alloc_zeroed(layout);
    if ptr.is_null() {
        return core::ptr::null_mut();
    }

    ptr as *mut Thread
}

/// Allocate a kernel stack for a thread
unsafe fn vm_allocate_kstack(thread: *mut Thread) -> *mut u8 {
    use alloc::alloc::{alloc, Layout};

    // Kernel stack size (8 pages = 32KB for kernel stack)
    const KERNEL_STACK_SIZE: usize = 8 * 4096;

    // Allocate kernel stack with proper alignment
    let layout = Layout::from_size_align(KERNEL_STACK_SIZE, 16).unwrap();
    let stack_ptr = alloc(layout);

    if stack_ptr.is_null() {
        return 0 as *mut u8;
    }

    // Store stack pointer in thread if thread is valid
    if !thread.is_null() {
        // TODO: Store stack information in thread structure
        // (*thread).stack_base = stack_ptr;
        // (*thread).stack_size = KERNEL_STACK_SIZE;
    }

    // Return the top of the stack (stacks grow downward)
    stack_ptr.add(KERNEL_STACK_SIZE) as *mut u8
}

/// Free a thread and its kernel stack
unsafe fn free_thread_and_stack(thread: *mut Thread) {
    use alloc::alloc::{dealloc, Layout};

    if thread.is_null() {
        return;
    }

    // TODO: Free kernel stack if we stored it in the thread
    // if !(*thread).stack_base.is_null() {
    //     let layout = Layout::from_size_align((*thread).stack_size, 16).unwrap();
    //     dealloc((*thread).stack_base as *mut u8, layout);
    // }

    // Free the thread structure
    let layout = Layout::new::<Thread>();
    dealloc(thread as *mut u8, layout);
}

/// Sleep for the specified number of milliseconds
unsafe fn thread_sleep_relative(ms: u32) {
    // TODO: Implement proper sleep
    // For now, spin wait
    let mut i = 0;
    for _ in 0..ms * 1000 {
        i = core::ptr::read_volatile(&i);
        unsafe { core::arch::asm!("pause") };
    }
}

/// Get pointer to online CPUs bitmask
unsafe fn mp_get_online_cpus() -> *mut u32 {
    extern "C" {
        #[link_name = "mp.online_cpus"]
        static mut MP_ONLINE_CPUS: u32;
    }
    &mut MP_ONLINE_CPUS
}

// External functions and types
extern "C" {
    /// Entry point for secondary CPU startup
    fn x86_secondary_cpu_long_mode_entry();

    /// Allocate AP structures for the given APIC IDs
    fn x86_allocate_ap_structures(apic_ids: *const u32, num_cpus: u8) -> i32;

    /// Convert APIC ID to CPU number
    fn x86_apic_id_to_cpu_num(apic_id: u32) -> i32;

    /// Check if CPU is online
    fn mp_is_cpu_online(cpu: u32) -> bool;

    /// Check if CPU is active (running in scheduler)
    fn mp_is_cpu_active(cpu: u32) -> bool;

    /// Acquire bootstrap16 memory region for AP startup
    fn x86_bootstrap16_acquire(entry: usize) -> Option<(*mut VmAspace, *mut BootstrapData, usize)>;

    /// Release bootstrap16 memory region
    fn x86_bootstrap16_release(data: *mut BootstrapData);
}

/// Bootstrap data for AP startup
#[repr(C)]
pub struct BootstrapData {
    pub cpu_id_counter: u32,
    pub cpu_waiting_mask: *mut AtomicI32,
    pub per_cpu: [PerCpuBootstrap; 64], // Maximum 64 CPUs
}

/// Per-CPU bootstrap data
#[repr(C)]
pub struct PerCpuBootstrap {
    pub kstack_base: *mut u8,
    pub thread: *mut Thread,
}

/// VM address space (placeholder)
#[repr(C)]
pub struct VmAspace;

/// Reschedule CPUs based on target mask
///
/// # Arguments
///
/// * `target_mask` - Bitmask of CPUs to reschedule
pub fn amd64_mp_reschedule(target_mask: u64) {
    // TODO: Implement proper CPU rescheduling
    // For now, this is a stub
    let _ = target_mask;
}

/// Prepare CPU for idle state
///
/// # Arguments
///
/// * `idle` - Whether the CPU should enter idle state
pub fn amd64_prepare_cpu_idle(idle: bool) {
    if idle {
        unsafe {
            // Enable interrupts before halting
            core::arch::asm!("sti");

            // Halt the CPU until an interrupt arrives
            // This is the proper idle state for x86-64 CPUs
            core::arch::asm!("hlt");
        }
    } else {
        // CPU is leaving idle state
        // Disable interrupts if needed
        unsafe {
            core::arch::asm!("cli");
        }
    }
}
