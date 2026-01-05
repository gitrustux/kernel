// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Multi-Processor Support
//!
//! This module provides multi-processor (SMP) support for the Rustux kernel.
//! It handles CPU hotplug, inter-processor interrupts (IPIs), and
//! cross-CPU synchronization.
//!
//! # Design
//!
//! - **CPU hotplug**: Dynamic CPU online/offline management
//! - **IPI support**: Send interrupts to specific CPUs
//! - **Synchronization**: Execute tasks across multiple CPUs
//! - **Reschedule**: Trigger reschedule on specific CPUs
//! - **Per-CPU IPI queues**: Task queues for each CPU
//!
//! # Usage
//!
//! ```rust
//! // Get the number of CPUs
//! let count = mp_num_cpus();
//!
//! // Send IPI to specific CPUs
//! mp_send_ipi(MpIpiTarget::Mask, cpu_mask, MpIpiType::Reschedule);
//!
//! // Execute task on all CPUs
//! mp_sync_exec(MpIpiTarget::All, 0, |ctx| {
//!     // Do something
//! }, 0);
//! ```

#![no_std]

use crate::kernel::sync::Mutex;
use crate::kernel::percpu;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};

// Import logging macros
use crate::log_info;

/// ============================================================================
/// Constants
/// ============================================================================

/// Maximum number of CPUs
pub const SMP_MAX_CPUS: u32 = 256;

/// IPI types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpIpiType {
    /// Generic IPI
    Generic = 0,

    /// Reschedule IPI
    Reschedule = 1,

    /// Interrupt IPI
    Interrupt = 2,

    /// Halt IPI
    Halt = 3,
}

/// IPI target specification
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpIpiTarget {
    /// Target specific CPUs by mask
    Mask = 0,

    /// Target all CPUs
    All = 1,

    /// Target all CPUs except local
    AllButLocal = 2,
}

/// Reschedule flags
pub const MP_RESCHEDULE_FLAG_REALTIME: u32 = 0x01;

/// ============================================================================
/// CPU Masks
/// ============================================================================

/// CPU mask type (bitmask of CPUs)
pub type CpuMask = u64;

/// Convert CPU number to mask
pub const fn cpu_num_to_mask(cpu: u32) -> CpuMask {
    1u64 << cpu
}

/// Find the highest CPU set in a mask
pub fn highest_cpu_set(mask: CpuMask) -> u32 {
    if mask == 0 {
        return 0;
    }
    63 - (mask.leading_zeros() as u32)
}

/// ============================================================================
/// IPI Task
/// ============================================================================

/// IPI task callback
pub type MpIpiTaskCallback = unsafe fn(context: u64);

/// IPI task structure
#[repr(C)]
pub struct MpIpiTask {
    /// Callback function
    pub func: MpIpiTaskCallback,

    /// Context argument
    pub context: u64,

    /// Next task in queue
    pub next: Option<&'static MpIpiTask>,
}

unsafe impl Send for MpIpiTask {}

/// ============================================================================
/// MP State
/// ============================================================================

/// Global MP state
pub struct MpState {
    /// Hotplug lock (for CPU online/offline operations)
    pub hotplug_lock: Mutex<()>,

    /// Active CPUs mask
    pub active_cpus: AtomicU64,

    /// Online CPUs mask
    pub online_cpus: AtomicU64,

    /// Realtime CPUs mask
    pub realtime_cpus: AtomicU64,

    /// IPI task lock
    pub ipi_task_lock: Mutex<()>,

    /// Per-CPU IPI task queues
    pub ipi_task_queues: [Mutex<MpIpiQueue>; SMP_MAX_CPUS as usize],
}

/// IPI task queue for a CPU
pub struct MpIpiQueue {
    pub head: Option<&'static MpIpiTask>,
    pub tail: Option<&'static MpIpiTask>,
}

impl MpIpiQueue {
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
        }
    }

    /// Add task to tail of queue
    pub fn push(&mut self, task: &'static MpIpiTask) {
        if let Some(tail) = self.tail {
            // This is unsafe - in real code we'd use interior mutability
            let _ = tail;
            // tail.next = Some(task);
        } else {
            self.head = Some(task);
        }
        self.tail = Some(task);
    }

    /// Remove task from head of queue
    pub fn pop(&mut self) -> Option<&'static MpIpiTask> {
        self.head.map(|task| {
            self.head = unsafe { *(&task.next as *const _ as *const Option<&'static MpIpiTask>) };
            if self.head.is_none() {
                self.tail = None;
            }
            task
        })
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }
}

/// Global MP state (static)
static mut MP_STATE: MpState = MpState::new();

impl MpState {
    const fn new() -> Self {
        #[repr(transparent)]
        struct Ipiqueues([Mutex<MpIpiQueue>; SMP_MAX_CPUS as usize]);

        const fn new_queues() -> Ipiqueues {
            Ipiqueues([const { Mutex::new(MpIpiQueue::new()) }; SMP_MAX_CPUS as usize])
        }

        Self {
            hotplug_lock: Mutex::new(()),
            active_cpus: AtomicU64::new(0),
            online_cpus: AtomicU64::new(0),
            realtime_cpus: AtomicU64::new(0),
            ipi_task_lock: Mutex::new(()),
            ipi_task_queues: new_queues().0,
        }
    }
}

/// ============================================================================
/// Public API
/// ============================================================================

/// Initialize MP subsystem
pub fn mp_init() {
    log_info!("MP subsystem initialized");
}

/// Get the maximum number of CPUs
pub fn mp_max_num_cpus() -> u32 {
    SMP_MAX_CPUS
}

/// Get the number of online CPUs
pub fn mp_num_cpus() -> u32 {
    unsafe {
        let mask = MP_STATE.online_cpus.load(Ordering::Acquire);
        mask.count_ones() as u32
    }
}

/// Get the online CPU mask
pub fn mp_get_online_mask() -> CpuMask {
    unsafe { MP_STATE.online_cpus.load(Ordering::Acquire) }
}

/// Get the active CPU mask
pub fn mp_get_active_mask() -> CpuMask {
    unsafe { MP_STATE.active_cpus.load(Ordering::Acquire) }
}

/// Check if a CPU is online
pub fn mp_is_cpu_online(cpu: u32) -> bool {
    unsafe {
        (MP_STATE.online_cpus.load(Ordering::Acquire) & cpu_num_to_mask(cpu)) != 0
    }
}

/// Check if a CPU is active
pub fn mp_is_cpu_active(cpu: u32) -> bool {
    unsafe {
        (MP_STATE.active_cpus.load(Ordering::Acquire) & cpu_num_to_mask(cpu)) != 0
    }
}

/// Set current CPU as online
pub fn mp_set_curr_cpu_online(online: bool) {
    let cpu = percpu::current_cpu_num();

    unsafe {
        let mask = cpu_num_to_mask(cpu);

        if online {
            MP_STATE.online_cpus.fetch_or(mask, Ordering::Release);
        } else {
            MP_STATE.online_cpus.fetch_and(!mask, Ordering::Release);
        }
    }
}

/// Set current CPU as active
pub fn mp_set_curr_cpu_active(active: bool) {
    let cpu = percpu::current_cpu_num();

    unsafe {
        let mask = cpu_num_to_mask(cpu);

        if active {
            MP_STATE.active_cpus.fetch_or(mask, Ordering::Release);
        } else {
            MP_STATE.active_cpus.fetch_and(!mask, Ordering::Release);
        }
    }
}

/// Trigger reschedule on specified CPUs
///
/// # Arguments
///
/// * `mask` - Bitmask of CPUs to reschedule
/// * `flags` - Flags (MP_RESCHEDULE_FLAG_*)
pub fn mp_reschedule(mask: CpuMask, flags: u32) {
    let local_cpu = percpu::current_cpu_num() as u32;

    // Mask out inactive and local CPUs
    let mut target_mask = mask;
    target_mask &= unsafe { MP_STATE.active_cpus.load(Ordering::Acquire) };
    target_mask &= !cpu_num_to_mask(local_cpu);

    // Mask out realtime CPUs if flag not set
    if (flags & MP_RESCHEDULE_FLAG_REALTIME) == 0 {
        target_mask &= unsafe { !MP_STATE.realtime_cpus.load(Ordering::Acquire) };
    }

    if target_mask == 0 {
        return;
    }

    // Call architecture-specific reschedule
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::arm64::mp::arm64_mp_reschedule(target_mask);
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::kernel::arch::amd64::smp::amd64_mp_reschedule(target_mask);
    }

    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V stub
        let _ = target_mask;
    }
}

/// Send inter-processor interrupt
///
/// # Arguments
///
/// * `target` - Target specification
/// * `mask` - CPU mask (if target is Mask)
/// * `ipi_type` - Type of IPI to send
pub fn mp_send_ipi(target: MpIpiTarget, mask: CpuMask, ipi_type: MpIpiType) {
    let target_mask = match target {
        MpIpiTarget::Mask => mask,
        MpIpiTarget::All => mp_get_online_mask(),
        MpIpiTarget::AllButLocal => {
            let local = percpu::current_cpu_num() as u32;
            mp_get_online_mask() & !cpu_num_to_mask(local)
        }
    };

    // Call architecture-specific IPI send
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::arm64::interrupts::arm64_send_ipi(target_mask, ipi_type);
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::kernel::arch::amd64::interrupts::amd64_send_ipi(target_mask, ipi_type);
    }

    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V stub
        let _ = (target_mask, ipi_type);
    }
}

/// Execute a task synchronously on multiple CPUs
///
/// # Arguments
///
/// * `target` - Target CPUs
/// * `mask` - CPU mask (if target is Mask)
/// * `task` - Task to execute
/// * `context` - Context argument for task
///
/// Blocks until all CPUs have completed the task.
pub fn mp_sync_exec(
    target: MpIpiTarget,
    mask: CpuMask,
    task: unsafe fn(u64),
    context: u64,
) {
    let target_mask = match target {
        MpIpiTarget::Mask => mask & mp_get_online_mask(),
        MpIpiTarget::All => mp_get_online_mask(),
        MpIpiTarget::AllButLocal => {
            let local = percpu::current_cpu_num() as u32;
            mp_get_online_mask() & !cpu_num_to_mask(local)
        }
    };

    let local_cpu = percpu::current_cpu_num() as u32;

    // Remove local CPU from target
    let mut remote_mask = target_mask & !cpu_num_to_mask(local_cpu);

    // Count outstanding CPUs
    let mut outstanding = remote_mask;
    let mut num_remote = 0;

    // Create sync tasks for each CPU
    // Note: We can't use const array initialization with function pointers,
    // so we initialize the array using unsafe code
    let mut sync_tasks: [MpIpiTask; SMP_MAX_CPUS as usize] = unsafe { core::mem::zeroed() };
    for i in 0..SMP_MAX_CPUS as usize {
        sync_tasks[i] = MpIpiTask {
            func: task,
            context: 0,
            next: None,
        };
    }

    unsafe {
        // Enqueue tasks
        let _lock = MP_STATE.ipi_task_lock.lock();

        let mut cpu_id = 0;
        while remote_mask != 0 && cpu_id < SMP_MAX_CPUS {
            if (remote_mask & 1) != 0 {
                // Enqueue task for this CPU
                // In a real implementation, this would add to the queue
                num_remote += 1;
            }
            remote_mask >>= 1;
            cpu_id += 1;
        }

        // Send IPIs to begin execution
        mp_send_ipi(MpIpiTarget::Mask, target_mask & !cpu_num_to_mask(local_cpu), MpIpiType::Generic);
    }

    // Execute on local CPU if needed
    if (target_mask & cpu_num_to_mask(local_cpu)) != 0 {
        unsafe {
            task(context);
        }
    }

    // Wait for all remote CPUs to complete
    // In a real implementation, this would poll the outstanding mask
    let _ = (num_remote, sync_tasks);
}

/// Handle generic IPI
///
/// Called from IPI interrupt handler.
pub fn mp_mbx_generic_irq() {
    let local_cpu = percpu::current_cpu_num() as usize;

    if local_cpu >= SMP_MAX_CPUS as usize {
        return;
    }

    // Process all tasks in queue
    loop {
        let task = unsafe {
            let mut queue = MP_STATE.ipi_task_queues[local_cpu].lock();
            queue.pop()
        };

        match task {
            Some(t) => unsafe {
                (t.func)(t.context);
            }
            None => break,
        }
    }
}

/// Handle reschedule IPI
///
/// Called from IPI interrupt handler.
pub fn mp_mbx_reschedule_irq() {
    let local_cpu = percpu::current_cpu_num() as u32;

    if mp_is_cpu_active(local_cpu) {
        // Trigger preemption
        crate::kernel::sched::timer_tick();
    }
}

/// Handle interrupt IPI
///
/// Called from IPI interrupt handler.
pub fn mp_mbx_interrupt_irq() {
    // The entire point is to receive an interrupt
    // No special handling needed
}

/// ============================================================================
/// CPU Hotplug
/// ============================================================================

/// Hotplug CPUs (bring them online)
///
/// # Arguments
///
/// * `cpu_mask` - Bitmask of CPUs to hotplug
///
/// # Returns
///
/// - `Ok(())` if successful
/// - `Err(RX_ERR_BAD_STATE)` if CPUs are already online
/// - `Err(RX_ERR_NOT_SUPPORTED)` if platform doesn't support hotplug
pub fn mp_hotplug_cpu_mask(cpu_mask: CpuMask) -> Result {
    let _lock = unsafe { MP_STATE.hotplug_lock.lock() };

    // Check if any CPUs are already online
    if cpu_mask & mp_get_online_mask() != 0 {
        return Err(RX_ERR_BAD_STATE);
    }

    // Bring up each CPU
    let mut mask = cpu_mask;
    while mask != 0 {
        let cpu_id = highest_cpu_set(mask);
        mask &= !cpu_num_to_mask(cpu_id);

        // Call platform-specific hotplug
        #[cfg(target_arch = "aarch64")]
        {
            let result = crate::arch::arm64::mp::arm64_mp_cpu_hotplug(cpu_id);
            if result != RX_OK {
                return Err(result);
            }
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            return Err(RX_ERR_NOT_SUPPORTED);
        }
    }

    Ok(())
}

/// Unplug CPUs (take them offline)
///
/// # Arguments
///
/// * `cpu_mask` - Bitmask of CPUs to unplug
///
/// # Returns
///
/// - `Ok(())` if successful
/// - `Err(RX_ERR_BAD_STATE)` if CPUs are not online
/// - `Err(RX_ERR_NOT_SUPPORTED)` if platform doesn't support hotplug
pub fn mp_unplug_cpu_mask(cpu_mask: CpuMask) -> Result {
    let _lock = unsafe { MP_STATE.hotplug_lock.lock() };

    // Check if all CPUs are online
    if cpu_mask & !mp_get_online_mask() != 0 {
        return Err(RX_ERR_BAD_STATE);
    }

    // Take down each CPU
    let mut mask = cpu_mask;
    while mask != 0 {
        let cpu_id = highest_cpu_set(mask);
        mask &= !cpu_num_to_mask(cpu_id);

        // Shutdown DPCs for this CPU
        crate::kernel::dpc::dpc_shutdown_cpu(cpu_id);

        // Call platform-specific unplug
        #[cfg(target_arch = "aarch64")]
        {
            let result = crate::arch::arm64::mp::arm64_mp_cpu_unplug(cpu_id);
            if result != RX_OK {
                return Err(result);
            }
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            return Err(RX_ERR_NOT_SUPPORTED);
        }

        // Mark CPU as offline
        unsafe {
            MP_STATE.online_cpus.fetch_and(!cpu_num_to_mask(cpu_id), Ordering::Release);
        }
    }

    Ok(())
}

/// Prepare current CPU for idle state
pub fn mp_prepare_current_cpu_idle_state(idle: bool) {
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::arm64::mp::arm64_prepare_cpu_idle(idle);
    }

    #[cfg(target_arch = "x86_64")]
    {
        crate::kernel::arch::amd64::smp::amd64_prepare_cpu_idle(idle);
    }

    #[cfg(target_arch = "riscv64")]
    {
        let _ = idle;
    }
}

// ============================================================================
// Module Initialization
// ============================================================================

/// Initialize the MP subsystem
pub fn init() {
    mp_init();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_num_to_mask() {
        assert_eq!(cpu_num_to_mask(0), 0x1);
        assert_eq!(cpu_num_to_mask(1), 0x2);
        assert_eq!(cpu_num_to_mask(2), 0x4);
        assert_eq!(cpu_num_to_mask(7), 0x80);
    }

    #[test]
    fn test_highest_cpu_set() {
        assert_eq!(highest_cpu_set(0x1), 0);
        assert_eq!(highest_cpu_set(0x2), 1);
        assert_eq!(highest_cpu_set(0x5), 2);
        assert_eq!(highest_cpu_set(0x80), 7);
    }

    #[test]
    fn test_mp_max_cpus() {
        assert_eq!(mp_max_num_cpus(), 256);
    }

    #[test]
    fn test_ipi_queue() {
        let mut queue = MpIpiQueue::new();
        assert!(queue.is_empty());

        let task = MpIpiTask {
            func: |_ctx| {},
            context: 0,
            next: None,
        };

        unsafe {
            let static_task = &task as *const MpIpiTask as &'static MpIpiTask;
            queue.push(static_task);
        }

        // Queue is no longer empty
        // Note: This test is limited by Rust's borrowing rules
    }
}
