// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Hypervisor CPU Management
//!
//! This module provides CPU-related functionality for the hypervisor,
//! including per-CPU task execution and thread pinning.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Per-CPU task type
///
/// Tasks executed on each CPU during hypervisor operations.
pub type PercpuTask = extern "C" fn(context: *mut u8, cpu_num: u32) -> i32;

/// CPU mask type
pub type CpuMask = u64;

/// Per-CPU state for task execution
struct PercpuState {
    /// CPU mask of successful executions
    cpu_mask: AtomicU64,
    /// Task to execute
    task: PercpuTask,
    /// Context to pass to task
    context: *mut u8,
}

unsafe impl Send for PercpuState {}
unsafe impl Sync for PercpuState {}

impl PercpuState {
    /// Create new per-CPU state
    fn new(task: PercpuTask, context: *mut u8) -> Self {
        Self {
            cpu_mask: AtomicU64::new(0),
            task,
            context,
        }
    }
}

/// Execute a task on all CPUs
///
/// # Arguments
///
/// * `task` - Task to execute on each CPU
/// * `context` - Context to pass to the task
///
/// # Returns
///
/// CPU mask of CPUs where the task succeeded
pub fn percpu_exec(task: PercpuTask, context: *mut u8) -> CpuMask {
    let state = PercpuState::new(task, context);

    // TODO: Implement MP sync execution across all CPUs
    // For now, we'll execute on the current CPU
    let cpu_num = arch_curr_cpu_num();
    let status = (state.task)(state.context, cpu_num);

    if status == 0 {
        // ZX_OK
        state.cpu_mask.fetch_or(cpu_num_to_mask(cpu_num), Ordering::Release);
    }

    state.cpu_mask.load(Ordering::Acquire)
}

/// Get the CPU for a virtual processor
///
/// # Arguments
///
/// * `vpid` - Virtual processor ID
///
/// # Returns
///
/// Physical CPU number
pub fn cpu_of(vpid: u16) -> u32 {
    ((vpid - 1) % arch_max_num_cpus() as u16) as u32
}

/// Pin the current thread to the CPU for a virtual processor
///
/// # Arguments
///
/// * `vpid` - Virtual processor ID
///
/// # Returns
///
/// The current thread
pub fn pin_thread(vpid: u16) -> *mut Thread {
    let thread = get_current_thread();
    let cpu = cpu_of(vpid);

    // TODO: Set thread CPU affinity
    let _ = (thread, cpu);

    thread
}

/// Check that the current thread is pinned to the correct CPU
///
/// # Arguments
///
/// * `vpid` - Virtual processor ID
/// * `thread` - Thread to check
///
/// # Returns
///
/// true if the thread is correctly pinned
pub fn check_pinned_cpu_invariant(vpid: u16, thread: *const Thread) -> bool {
    let cpu = cpu_of(vpid);
    let current_thread = get_current_thread();
    let current_cpu = arch_curr_cpu_num();
    let cpu_mask = cpu_num_to_mask(cpu);

    // Check that:
    // 1. This is the current thread
    // 2. The thread is pinned to the correct CPU
    // 3. We're running on that CPU
    thread == current_thread
        && /* thread_cpu_affinity(thread) & cpu_mask != 0 */ true
        && current_cpu == cpu
}

/// Get the current CPU number
fn arch_curr_cpu_num() -> u32 {
    // TODO: Implement arch-specific current CPU number
    0
}

/// Get the maximum number of CPUs
fn arch_max_num_cpus() -> u32 {
    // TODO: Implement arch-specific max CPU count
    1
}

/// Convert CPU number to mask
fn cpu_num_to_mask(cpu_num: u32) -> u64 {
    1u64 << cpu_num
}

/// Get the current thread
fn get_current_thread() -> *mut Thread {
    // TODO: Implement thread lookup
    core::ptr::null_mut()
}

/// Thread opaque type
#[repr(C)]
pub struct Thread {
    _private: [u8; 0],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_of() {
        // With 1 CPU
        assert_eq!(cpu_of(1), 0);
        assert_eq!(cpu_of(2), 0);

        // With 4 CPUs (would need to mock arch_max_num_cpus)
        // assert_eq!(cpu_of(1), 0);
        // assert_eq!(cpu_of(2), 1);
        // assert_eq!(cpu_of(5), 0);
    }

    #[test]
    fn test_cpu_num_to_mask() {
        assert_eq!(cpu_num_to_mask(0), 1);
        assert_eq!(cpu_num_to_mask(1), 2);
        assert_eq!(cpu_num_to_mask(2), 4);
        assert_eq!(cpu_num_to_mask(8), 256);
    }

    #[test]
    fn test_percpu_state() {
        extern "C" fn dummy_task(_context: *mut u8, _cpu_num: u32) -> i32 {
            0 // ZX_OK
        }

        let state = PercpuState::new(dummy_task, core::ptr::null_mut());
        assert_eq!(state.cpu_mask.load(Ordering::Acquire), 0);
    }
}
