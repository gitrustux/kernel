// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Deferred Procedure Calls
//!
//! This module provides Deferred Procedure Call (DPC) support for the Rustux kernel.
//! DPCs allow work to be deferred to be executed later in a thread context.
//!
//! # Design
//!
//! - **Per-CPU DPC queues**: Each CPU has its own DPC queue
//! - **Dedicated thread**: Each CPU has a DPC worker thread
//! - **Deferred execution**: Work can be queued from interrupt context
//! - **FIFO ordering**: DPCs executed in order they were queued
//!
//! # Usage
//!
//! ```rust
//! let dpc = Dpc::new(|dpc| {
//!     // Do work
//! });
//!
//! // Queue for later execution
//! dpc.queue(true)?;
//! ```

#![no_std]

use crate::kernel::sync::{Event, EventFlags, Mutex};
use crate::kernel::thread::ThreadId;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// Import logging macros
use crate::{log_debug, log_info};

/// ============================================================================
/// DPC
/// ============================================================================

/// DPC callback function type
pub type DpcCallback = unsafe fn(dpc: &Dpc);

/// DPC priority level
pub const DPC_THREAD_PRIORITY: u8 = 32; // Higher than default

/// DPC structure
///
/// Represents a deferred procedure call that can be queued for later execution.
pub struct Dpc {
    /// Callback function to execute
    pub func: Mutex<Option<DpcCallback>>,

    /// Opaque argument passed to callback
    pub arg: u64,

    /// Node for linked list
    pub next: Mutex<Option<&'static Dpc>>,

    /// Whether this DPC is currently queued
    pub queued: AtomicBool,
}

unsafe impl Send for Dpc {}

impl Dpc {
    /// Create a new DPC
    pub const fn new() -> Self {
        Self {
            func: Mutex::new(None),
            arg: 0,
            next: Mutex::new(None),
            queued: AtomicBool::new(false),
        }
    }

    /// Create a DPC with a callback
    pub fn with_callback(callback: DpcCallback) -> Self {
        Self {
            func: Mutex::new(Some(callback)),
            arg: 0,
            next: Mutex::new(None),
            queued: AtomicBool::new(false),
        }
    }

    /// Set the callback function
    pub fn set_callback(&self, callback: DpcCallback) {
        *self.func.lock() = Some(callback);
    }

    /// Set the argument
    pub fn set_arg(&mut self, arg: u64) {
        self.arg = arg;
    }

    /// Queue the DPC for execution
    ///
    /// # Arguments
    ///
    /// * `reschedule` - Whether to trigger immediate rescheduling
    ///
    /// # Returns
    ///
    /// - `Ok(())` if queued successfully
    /// - `Err(RX_ERR_ALREADY_EXISTS)` if already queued
    pub fn queue(&self, reschedule: bool) -> Result {
        // Check if already queued
        if self.queued.load(Ordering::Acquire) {
            return Err(RX_ERR_ALREADY_EXISTS);
        }

        // Get current CPU's DPC queue
        let cpu_num = crate::kernel::percpu::current_cpu_num() as usize;

        // Add to queue
        unsafe {
            dpc_queue_cpu(self, cpu_num as u32, reschedule)?;
        }

        self.queued.store(true, Ordering::Release);
        Ok(())
    }

    /// Cancel the DPC if queued
    ///
    /// # Returns
    ///
    /// true if was queued and removed, false otherwise
    pub fn cancel(&self) -> bool {
        if !self.queued.load(Ordering::Acquire) {
            return false;
        }

        // Remove from queue
        unsafe {
            dpc_remove_from_queue(self);
        }

        self.queued.store(false, Ordering::Release);
        true
    }

    /// Execute the DPC callback
    ///
    /// Called by the DPC worker thread.
    pub fn execute(&self) {
        if let Some(func) = *self.func.lock() {
            unsafe {
                func(self);
            }
        }
    }

    /// Check if DPC is queued
    pub fn is_queued(&self) -> bool {
        self.queued.load(Ordering::Acquire)
    }
}

/// ============================================================================
/// Per-CPU DPC State
/// ============================================================================

/// Per-CPU DPC state
pub struct DpcState {
    /// DPC queue for this CPU
    pub queue: Mutex<DpcQueue>,

    /// Event to signal when DPCs are queued
    pub event: Event,

    /// DPC thread ID for this CPU
    pub thread_id: Mutex<Option<ThreadId>>,

    /// Whether DPC thread should stop
    pub stop: AtomicBool,

    /// Whether this DPC state is initialized
    pub initialized: AtomicBool,
}

/// DPC queue (linked list)
pub struct DpcQueue {
    pub head: Option<&'static Dpc>,
    pub tail: Option<&'static Dpc>,
    pub count: usize,
}

impl DpcQueue {
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            count: 0,
        }
    }

    /// Add a DPC to the tail of the queue
    pub fn push(&mut self, dpc: &'static Dpc) {
        if let Some(tail) = self.tail {
            *tail.next.lock() = Some(dpc);
        } else {
            self.head = Some(dpc);
        }
        self.tail = Some(dpc);
        self.count += 1;
    }

    /// Remove a DPC from the head of the queue
    pub fn pop(&mut self) -> Option<&'static Dpc> {
        self.head.map(|dpc| {
            self.head = *dpc.next.lock();
            if self.head.is_none() {
                self.tail = None;
            }
            self.count -= 1;
            dpc
        })
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.count
    }
}

/// ============================================================================
/// Global DPC State
/// ============================================================================

/// Per-CPU DPC state array
static mut DPC_STATES: [DpcState; 256] = [const { DpcState::new() }; 256];

/// DPC lock
static DPC_LOCK: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

impl DpcState {
    const fn new() -> Self {
        Self {
            queue: Mutex::new(DpcQueue::new()),
            event: Event::new(false, EventFlags::empty()),
            thread_id: Mutex::new(None),
            stop: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
        }
    }

    /// Initialize DPC state for a CPU
    pub fn init(&mut self, cpu_num: u32) {
        // Initialize event
        self.event.init(false, EventFlags::empty());

        // Create DPC worker thread
        let thread_id = create_dpc_thread(cpu_num);
        *self.thread_id.lock() = Some(thread_id);

        self.initialized.store(true, Ordering::Release);

        log_info!("DPC initialized for CPU {}", cpu_num);
    }

    /// Shutdown DPC for a CPU
    pub fn shutdown(&self) {
        if !self.initialized.load(Ordering::Acquire) {
            return;
        }

        // Signal stop
        self.stop.store(true, Ordering::Release);

        // Signal the event to wake the thread
        self.event.signal();

        // Wait for thread to terminate
        if let Some(thread_id) = *self.thread_id.lock() {
            // TODO: Wait for thread termination
            log_debug!("Waiting for DPC thread {} to terminate", thread_id);
        }

        self.initialized.store(false, Ordering::Release);

        log_info!("DPC shutdown for CPU");
    }

    /// Process DPCs from the queue
    ///
    /// Called by the DPC worker thread.
    pub fn process(&self) -> bool {
        // Pop and execute DPCs
        loop {
            let dpc = {
                let mut queue = self.queue.lock();
                queue.pop()
            };

            match dpc {
                Some(dpc) => {
                    dpc.queued.store(false, Ordering::Release);
                    dpc.execute();
                }
                None => {
                    // Queue is empty, unsignal the event
                    self.event.unsignal();
                    break;
                }
            }
        }

        // Check if we should stop
        self.stop.load(Ordering::Acquire)
    }
}

/// ============================================================================
/// Public API
/// ============================================================================

/// Initialize DPC for the current CPU
pub fn dpc_init_for_cpu() {
    unsafe {
        let cpu_num = crate::kernel::percpu::current_cpu_num() as usize;

        // Check if already initialized
        if DPC_STATES[cpu_num].initialized.load(Ordering::Acquire) {
            return;
        }

        (*DPC_STATES.as_mut_ptr().add(cpu_num)).init(cpu_num as u32);
    }
}

/// Initialize DPC subsystem
pub fn dpc_init() {
    log_info!("DPC subsystem initialized");
    dpc_init_for_cpu();
}

/// Queue a DPC on a specific CPU
///
/// # Safety
///
/// Must be called with appropriate locking.
pub unsafe fn dpc_queue_cpu(dpc: &Dpc, cpu_id: u32, reschedule: bool) -> Result {
    if cpu_id >= 256 {
        return Err(RX_ERR_INVALID_ARGS);
    }

    let state = &DPC_STATES[cpu_id as usize];

    // Add to queue
    {
        let mut queue = state.queue.lock();
        queue.push(unsafe { &*(dpc as *const Dpc) });
    }

    // Signal the event
    if reschedule {
        state.event.signal_and_reschedule();
    } else {
        state.event.signal();
    }

    Ok(())
}

/// Remove a DPC from its queue
///
/// # Safety
///
/// Must be called with appropriate locking.
pub unsafe fn dpc_remove_from_queue(dpc: &Dpc) {
    // In a real implementation, this would search all CPU queues
    // and remove the DPC. For now, this is a stub.
    let _ = dpc;
}

/// Shutdown DPC for a specific CPU
pub fn dpc_shutdown_cpu(cpu_id: u32) {
    if cpu_id >= 256 {
        return;
    }

    unsafe {
        DPC_STATES[cpu_id as usize].shutdown();
    }
}

/// Transition DPCs from one CPU to another
///
/// Used when a CPU is going offline.
pub fn dpc_transition_off_cpu(src_cpu: u32, dst_cpu: u32) {
    if src_cpu >= 256 || dst_cpu >= 256 {
        return;
    }

    unsafe {
        let src_state = &DPC_STATES[src_cpu as usize];
        let dst_state = &DPC_STATES[dst_cpu as usize];

        // Move all DPCs from src to dst
        let mut src_queue = src_state.queue.lock();
        let mut dst_queue = dst_state.queue.lock();

        while let Some(dpc) = src_queue.pop() {
            dst_queue.push(dpc);
        }

        // Reset source state
        src_state.stop.store(false, Ordering::Release);
        src_state.initialized.store(false, Ordering::Release);
    }
}

/// ============================================================================
/// DPC Worker Thread
/// ============================================================================

/// DPC worker thread entry point
extern "C" fn dpc_worker_thread(_arg: u64) -> ! {
    let cpu_num = crate::kernel::percpu::current_cpu_num() as usize;

    log_debug!("DPC worker thread starting on CPU {}", cpu_num);

    loop {
        let state = unsafe { &DPC_STATES[cpu_num] };

        // Wait for work
        let _ = state.event.wait();

        // Process DPCs
        if state.process() {
            // Stop requested
            log_debug!("DPC worker thread on CPU {} stopping", cpu_num);
            break;
        }
    }

    // Thread exit - use the exit function from current thread
    // For now, just loop forever since we can't properly exit
    loop {
        unsafe { crate::kernel::arch::amd64::registers::x86_hlt() };
    }
}

/// Create DPC worker thread
fn create_dpc_thread(cpu_num: u32) -> ThreadId {
    // In a real implementation, this would create a new thread
    // with the dpc_worker_thread entry point
    log_debug!("Creating DPC thread for CPU {}", cpu_num);

    // Stub: return a dummy TID
    1000 + cpu_num as u64
}

// ============================================================================
// Module Initialization
// ============================================================================

/// Initialize the DPC subsystem
pub fn init() {
    // This is called from the main init sequence
    dpc_init();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpc_new() {
        let dpc = Dpc::new();
        assert!(!dpc.is_queued());
        assert!(dpc.func.lock().is_none());
    }

    #[test]
    fn test_dpc_with_callback() {
        let dpc = Dpc::with_callback(|_dpc| {
            // Callback
        });
        assert!(dpc.func.lock().is_some());
    }

    #[test]
    fn test_dpc_state_new() {
        let state = DpcState::new();
        assert!(!state.initialized.load(Ordering::Acquire));
        assert!(!state.stop.load(Ordering::Acquire));
    }

    #[test]
    fn test_dpc_queue() {
        let queue = DpcQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        let dpc1 = Dpc::new();
        let dpc2 = Dpc::new();

        unsafe {
            // This is unsafe because we're creating static references
            let static_dpc1 = &dpc1 as *const Dpc as &'static Dpc;
            let static_dpc2 = &dpc2 as *const Dpc as &'static Dpc;

            queue.push(static_dpc1);
            queue.push(static_dpc2);
        }

        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());
    }
}
