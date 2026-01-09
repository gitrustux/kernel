// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V Multi-Processor (MP) support
//!
//! This module handles bringing up additional harts (CPU cores)
//! and managing MP initialization for RISC-V systems.


use crate::arch::riscv64::registers;
use crate::arch::riscv64::registers::csr;
use crate::debug;
use crate::rustux::types::*;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Maximum number of harts (CPU cores) supported
const MAX_HARTS: usize = 16;

/// Online CPU bitmask
static mut ONLINE_CPUS: u32 = 0;
/// Active CPU bitmask (CPUs that have entered the scheduler)
static mut ACTIVE_CPUS: u32 = 0;

/// Per-hart data structure
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PerHartData {
    /// Hart ID
    pub hart_id: usize,
    /// CPU number (0-based index)
    pub cpu_num: u32,
    /// Kernel stack pointer
    pub kernel_sp: usize,
    /// Whether this hart is online
    pub online: bool,
    /// Whether this hart is active (in scheduler)
    pub active: bool,
}

/// Per-hart data array
static mut PER_HART_DATA: [PerHartData; MAX_HARTS] = [PerHartData {
    hart_id: 0,
    cpu_num: 0,
    kernel_sp: 0,
    online: false,
    active: false,
}; MAX_HARTS];

/// Secondary hart entry point type
pub type SecondaryHartEntry = unsafe extern "C" fn(hart_id: usize, arg: usize);

/// Secondary hart entry point function pointer
static mut SECONDARY_HART_ENTRY: Option<SecondaryHartEntry> = None;
/// Argument to pass to secondary harts
static mut SECONDARY_HART_ARG: usize = 0;

/// Harts that are still booting
pub static HARTS_BOOTING: AtomicU32 = AtomicU32::new(0);

/// Convert hart ID to CPU number
///
/// # Arguments
///
/// * `hart_id` - The hart ID from mhartid CSR
///
/// # Returns
///
/// CPU number (0-based index) or negative if invalid
pub fn riscv_hart_id_to_cpu_num(hart_id: usize) -> i32 {
    // Simple linear mapping: hart_id -> cpu_num
    // In a real system, this would be more complex to handle
    // non-contiguous hart IDs
    if hart_id >= MAX_HARTS {
        return -1; // Invalid hart ID
    }

    hart_id as i32
}

/// Convert CPU number to hart ID
///
/// # Arguments
///
/// * `cpu_num` - CPU number (0-based index)
///
/// # Returns
///
/// Hart ID or negative if invalid
pub fn riscv_cpu_num_to_hart_id(cpu_num: u32) -> i32 {
    // Simple linear mapping: cpu_num -> hart_id
    if cpu_num as usize >= MAX_HARTS {
        return -1; // Invalid CPU number
    }

    cpu_num as i32
}

/// Get the current hart ID
///
/// # Returns
///
/// The current hart ID from the mhartid CSR
#[inline]
pub fn riscv_get_hart_id() -> usize {
    unsafe { registers::read_csr(csr::MHARTID) as usize }
}

/// Get the current CPU number
///
/// # Returns
///
/// The current CPU number (0-based index)
#[inline]
pub fn riscv_get_cpu_num() -> u32 {
    let hart_id = riscv_get_hart_id();
    match riscv_hart_id_to_cpu_num(hart_id) {
        cpu_num if cpu_num >= 0 => cpu_num as u32,
        _ => 0, // Fallback to CPU 0
    }
}

/// Check if a CPU is online
///
/// # Arguments
///
/// * `cpu` - CPU number to check
///
/// # Returns
///
/// true if the CPU is online
pub fn riscv_is_cpu_online(cpu: u32) -> bool {
    unsafe { (ONLINE_CPUS & (1 << cpu)) != 0 }
}

/// Check if a CPU is active (in the scheduler)
///
/// # Arguments
///
/// * `cpu` - CPU number to check
///
/// # Returns
///
/// true if the CPU is active
pub fn riscv_is_cpu_active(cpu: u32) -> bool {
    unsafe { (ACTIVE_CPUS & (1 << cpu)) != 0 }
}

/// Mark a CPU as online
///
/// # Arguments
///
/// * `cpu` - CPU number to mark online
///
/// # Safety
///
/// Must be called with proper synchronization
pub unsafe fn riscv_mark_cpu_online(cpu: u32) {
    ONLINE_CPUS |= 1 << cpu;
    PER_HART_DATA[cpu as usize].online = true;
}

/// Mark a CPU as active
///
/// # Arguments
///
/// * `cpu` - CPU number to mark active
///
/// # Safety
///
/// Must be called with proper synchronization
pub unsafe fn riscv_mark_cpu_active(cpu: u32) {
    ACTIVE_CPUS |= 1 << cpu;
    PER_HART_DATA[cpu as usize].active = true;
}

/// Get the online CPUs bitmask
///
/// # Returns
///
/// Bitmask of online CPUs
pub fn riscv_get_online_cpus() -> u32 {
    unsafe { ONLINE_CPUS }
}

/// Get the per-hart data for a specific CPU
///
/// # Arguments
///
/// * `cpu` - CPU number
///
/// # Returns
///
/// Reference to the per-hart data
///
/// # Safety
///
/// cpu must be valid
pub unsafe fn riscv_get_per_hart_data(cpu: u32) -> &'static mut PerHartData {
    &mut PER_HART_DATA[cpu as usize]
}

/// Get the per-hart data for the current CPU
///
/// # Returns
///
/// Reference to the current CPU's per-hart data
///
/// # Safety
///
/// Must be called from a valid CPU context
pub unsafe fn riscv_get_current_per_hart_data() -> &'static mut PerHartData {
    let cpu = riscv_get_cpu_num();
    riscv_get_per_hart_data(cpu)
}

/// Initialize MP support for the given number of harts
///
/// # Arguments
///
/// * `hart_ids` - Array of hart IDs for the system
/// * `num_harts` - Number of harts to initialize
pub fn riscv_init_mp(hart_ids: &[usize], num_harts: usize) {
    println!("RISC-V MP init: {} harts", num_harts);

    // Validate that we have space for all harts
    assert!(num_harts <= MAX_HARTS);

    // Initialize per-hart data
    for i in 0..num_harts {
        unsafe {
            let cpu_num = riscv_hart_id_to_cpu_num(hart_ids[i]);
            if cpu_num >= 0 {
                let cpu_num = cpu_num as usize;
                PER_HART_DATA[cpu_num] = PerHartData {
                    hart_id: hart_ids[i],
                    cpu_num: cpu_num as u32,
                    kernel_sp: 0,
                    online: false,
                    active: false,
                };
            }
        }
    }

    // Mark boot hart (CPU 0) as online
    unsafe {
        riscv_mark_cpu_online(0);
    }
}

/// Send inter-processor interrupt (IPI) to a target hart
///
/// RISC-V uses MSIP (Machine Software Interrupt Pending) registers
/// in the CLINT (Core-Local Interrupt Controller) for IPIs.
///
/// # Arguments
///
/// * `target_hart_id` - Hart ID to send IPI to
///
/// # Returns
///
/// 0 on success, negative on error
pub fn riscv_send_ipi(target_hart_id: usize) -> i32 {
    // MSIP registers are memory-mapped
    // For SiFive U54/U74, MSIP base is typically at 0x0200_0000
    // Each hart has a 4-byte MSIP register
    const MSIP_BASE: usize = 0x0200_0000;

    // Write to the MSIP register for the target hart
    // Writing 1 triggers the software interrupt
    let msip_addr = MSIP_BASE + (target_hart_id * 4);

    unsafe {
        let msip_ptr = msip_addr as *mut u32;
        msip_ptr.write_volatile(1);
    }

    0 // OK
}

/// Wait for harts to finish booting
///
/// # Arguments
///
/// * `expected_mask` - Bitmask of harts that should be booting
///
/// # Returns
///
/// 0 if all harts booted successfully, timeout otherwise
pub fn riscv_wait_for_harts(expected_mask: u32) -> i32 {
    // Wait up to 1 second (200 * 5ms)
    for _ in 0..200 {
        let booting = HARTS_BOOTING.load(Ordering::SeqCst);
        if booting == 0 {
            return 0; // All harts booted
        }

        // Sleep 5ms
        // TODO: Implement proper timer-based sleep
        for _ in 0..100_000 {
            unsafe { core::arch::asm!("nop") };
        }
    }

    // Timeout
    let failed_mask = HARTS_BOOTING.load(Ordering::SeqCst);
    println!("Harts failed to boot: mask {:#x}", failed_mask);
    -1 // Timeout
}

/// Secondary hart entry point
///
/// This is called by each secondary hart when it starts.
///
/// # Arguments
///
/// * `hart_id` - The hart ID
///
/// # Safety
///
/// Must only be called from secondary hart startup code
#[no_mangle]
pub unsafe extern "C" fn riscv_secondary_hart_entry(hart_id: usize) -> ! {
    let cpu_num = riscv_hart_id_to_cpu_num(hart_id);

    if cpu_num < 0 {
        println!("Invalid hart ID: {}", hart_id);
        loop {
            core::arch::asm!("wfi");
        }
    }

    let cpu_num = cpu_num as u32;

    // Mark this hart as online
    riscv_mark_cpu_online(cpu_num);

    // Clear this hart from the booting mask
    HARTS_BOOTING.fetch_and(!(1 << cpu_num), Ordering::SeqCst);

    println!("Hart {} (CPU {}) online", hart_id, cpu_num);

    // Call the secondary hart entry function if set
    if let Some(entry) = SECONDARY_HART_ENTRY {
        entry(hart_id, SECONDARY_HART_ARG);
    } else {
        // No entry point, just halt
        println!("No secondary hart entry set, halting");
        loop {
            core::arch::asm!("wfi");
        }
    }

    // Should never return
    loop {
        core::arch::asm!("wfi");
    }
}

/// Bring up secondary harts
///
/// # Arguments
///
/// * `hart_ids` - Array of hart IDs to bring up
/// * `count` - Number of harts
/// * `entry` - Entry point function
/// * `arg` - Argument to pass to entry point
///
/// # Returns
///
/// 0 on success, negative on error
pub fn riscv_bringup_harts(
    hart_ids: &[usize],
    count: usize,
    entry: SecondaryHartEntry,
    arg: usize,
) -> i32 {
    if count == 0 {
        return 0; // Nothing to do
    }

    // Validate hart IDs
    for i in 0..count {
        let cpu_num = riscv_hart_id_to_cpu_num(hart_ids[i]);
        if cpu_num < 0 {
            println!("Invalid hart ID: {}", hart_ids[i]);
            return -1; // Invalid args
        }

        let cpu_num = cpu_num as u32;
        if riscv_is_cpu_online(cpu_num) {
            println!("CPU {} already online", cpu_num);
            return -2; // Bad state
        }
    }

    // Set the entry point and argument
    unsafe {
        SECONDARY_HART_ENTRY = Some(entry);
        SECONDARY_HART_ARG = arg;
    }

    // Set up the booting mask
    let mut booting_mask: u32 = 0;
    for i in 0..count {
        let cpu_num = riscv_hart_id_to_cpu_num(hart_ids[i]) as u32;
        booting_mask |= 1 << cpu_num;
    }
    HARTS_BOOTING.store(booting_mask, Ordering::SeqCst);

    // Send IPIs to wake up secondary harts
    print!("Waking harts:");
    for i in 0..count {
        print!(" {} ", hart_ids[i]);
        riscv_send_ipi(hart_ids[i]);
    }
    println!();

    // Wait for harts to come up
    riscv_wait_for_harts(booting_mask)
}

/// Get number of online CPUs
///
/// # Returns
///
/// Number of online CPUs
pub fn riscv_num_online_cpus() -> u32 {
    let mask = riscv_get_online_cpus();
    mask.count_ones()
}

/// Get the number of harts from device tree
///
/// # Returns
///
/// Number of harts in the system
///
/// # Safety
///
/// Must be called during early boot when device tree is available
pub unsafe fn riscv_get_num_harts_from_dt() -> usize {
    // Try to get device tree pointer from a1 register (passed by bootloader)
    // In RISC-V boot protocol, a1 contains the device tree pointer

    // Read mhartid to get current hart ID
    let mhartid: usize;
    core::arch::asm!("csrr {}, mhartid", out(reg) mhartid);

    // For now, assume single hart system
    // In a full implementation, we would:
    // 1. Parse the device tree blob (DTB) passed in a1
    // 2. Find the "/cpus" node
    // 3. Count all "cpu" child nodes
    // 4. Check each cpu node's "status" property (must be "okay")
    // 5. Return the count of enabled CPUs

    // Common QEMU virt machine configurations:
    // - Default: 1-4 harts
    // - Can be configured with -smp option

    // For bare-metal or QEMU, check if we can detect multiple harts
    // by checking the maximum hart ID we've seen

    // For now, start with the current hart + assume some defaults
    // This is a simplified implementation that will be improved
    // when proper device tree parsing is added

    // Return at least 1 (current hart)
    mhartid + 1
}

/// MP initialization state
#[repr(C)]
#[derive(Copy, Clone)]
pub enum MpInitState {
    NotStarted = 0,
    InProgress = 1,
    Complete = 2,
    Failed = 3,
}

/// Current MP initialization state
static mut MP_INIT_STATE: MpInitState = MpInitState::NotStarted;

/// Get MP initialization state
pub fn riscv_mp_init_state() -> MpInitState {
    unsafe { MP_INIT_STATE }
}

/// Set MP initialization state
///
/// # Safety
///
/// Should only be called by MP initialization code
pub unsafe fn riscv_set_mp_init_state(state: MpInitState) {
    MP_INIT_STATE = state;
}
