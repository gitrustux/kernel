// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Thread Scheduler
//!
//! This module implements the thread scheduler for the Rustux kernel.
//! It uses a priority-based round-robin scheduling algorithm.
//!
//! # Design
//!
//! - **Priority-based**: Multiple priority levels (0-255)
//! - **Round-robin**: Threads at same priority scheduled in FIFO order
//! - **Preemptive**: Timer tick triggers context switch
//! - **Per-CPU**: Each CPU has its own run queue
//!
//! # Thread States
//!
//! ```text
//! New -> Ready -> Running -> Blocked -> Ready -> Running
//!                 |           |                      |
//!                 v           v                      v
//!               Dying -------> Dead <-----------------
//! ```
//!
//! # Usage
//!
//! ```rust
//! // Add thread to run queue
//! scheduler.ready(thread);
//!
//! // Schedule next thread
//! let next = scheduler.schedule();
//!
//! // Yield current thread
//! scheduler.yield_current();
//! ```


use crate::kernel::thread::{Thread, ThreadId, ThreadState, BlockReason, PRIORITY_DEFAULT};
use crate::rustux::types::*;
use crate::kernel::vm::Result;
use alloc::collections::VecDeque;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// Import logging macros
use crate::{log_debug, log_info, log_trace};

/// ============================================================================
/// Scheduler Configuration
/// ============================================================================

/// Number of priority levels
pub const N_PRIORITIES: usize = 32;

/// Default time slice (in nanoseconds)
pub const DEFAULT_TIME_SLICE: u64 = 10_000_000; // 10ms

/// Minimum time slice
pub const MIN_TIME_SLICE: u64 = 1_000_000; // 1ms

/// Maximum time slice
pub const MAX_TIME_SLICE: u64 = 100_000_000; // 100ms

/// ============================================================================
/// Per-CPU Scheduler State
/// ============================================================================

/// Per-CPU run queue
///
/// This is the core scheduler data structure for a single CPU.
pub struct RunQueue {
    /// Run queues for each priority level
    queues: [VecDeque<ThreadId>; N_PRIORITIES],

    /// Currently running thread on this CPU
    current: Option<ThreadId>,

    /// Preemption flag (set by timer interrupt)
    preempt_pending: AtomicBool,

    /// Time slice for current thread
    current_time_slice: u64,

    /// Last schedule time
    last_schedule_time: u64,

    /// Statistics
    stats: SchedulerStats,
}

impl RunQueue {
    /// Create a new run queue
    pub const fn new() -> Self {
        // Note: We can't initialize VecDeque in const context
        // This is a simplified version - real init happens in new()
        Self {
            queues: [
                // N_PRIORITIES empty VecDeques
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
                VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new(),
            ],
            current: None,
            preempt_pending: AtomicBool::new(false),
            current_time_slice: DEFAULT_TIME_SLICE,
            last_schedule_time: 0,
            stats: SchedulerStats::new(),
        }
    }

    /// Add a thread to the run queue
    pub fn enqueue(&mut self, tid: ThreadId, priority: u8) {
        let idx = self.priority_to_index(priority);
        self.queues[idx].push_back(tid);
        self.stats.ready_count += 1;

        log_trace!(
            "Enqueued thread: tid={} priority={} queue={}",
            tid,
            priority,
            idx
        );
    }

    /// Remove a thread from the run queue
    pub fn dequeue(&mut self, tid: ThreadId) -> bool {
        for queue in &mut self.queues {
            if let Some(pos) = queue.iter().position(|&t| t == tid) {
                queue.remove(pos);
                self.stats.ready_count -= 1;
                return true;
            }
        }
        false
    }

    /// Select the next thread to run
    pub fn select(&mut self) -> Option<ThreadId> {
        // Find highest priority non-empty queue
        for i in 0..N_PRIORITIES {
            if !self.queues[i].is_empty() {
                let tid = self.queues[i].pop_front().unwrap();
                self.stats.ready_count -= 1;
                self.stats.schedules += 1;
                return Some(tid);
            }
        }
        None
    }

    /// Get the current thread
    pub const fn current(&self) -> Option<ThreadId> {
        self.current
    }

    /// Set the current thread
    pub fn set_current(&mut self, tid: Option<ThreadId>) {
        self.current = tid;
    }

    /// Check if preemption is pending
    pub fn is_preempt_pending(&self) -> bool {
        self.preempt_pending.load(Ordering::Relaxed)
    }

    /// Clear preemption pending flag
    pub fn clear_preempt(&self) {
        self.preempt_pending.store(false, Ordering::Relaxed);
    }

    /// Request preemption
    pub fn request_preempt(&mut self) {
        self.preempt_pending.store(true, Ordering::Relaxed);
        self.stats.preemptions += 1;
    }

    /// Get the current time slice
    pub const fn time_slice(&self) -> u64 {
        self.current_time_slice
    }

    /// Set the time slice
    pub fn set_time_slice(&mut self, slice: u64) {
        self.current_time_slice = slice.clamp(MIN_TIME_SLICE, MAX_TIME_SLICE);
    }

    /// Get the last schedule time
    pub const fn last_schedule_time(&self) -> u64 {
        self.last_schedule_time
    }

    /// Update the last schedule time
    pub fn update_schedule_time(&mut self, time: u64) {
        self.last_schedule_time = time;
    }

    /// Check if run queue is empty
    pub fn is_empty(&self) -> bool {
        self.queues.iter().all(|q| q.is_empty())
    }

    /// Get number of threads in run queue
    pub fn len(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }

    /// Get statistics
    pub const fn stats(&self) -> &SchedulerStats {
        &self.stats
    }

    /// Get mutable statistics
    pub fn stats_mut(&mut self) -> &mut SchedulerStats {
        &mut self.stats
    }

    /// Convert priority (0-255) to queue index
    fn priority_to_index(&self, priority: u8) -> usize {
        // Map 0-255 to 0-N_PRIORITIES-1
        let idx = (priority as usize) * N_PRIORITIES / 256;
        idx.min(N_PRIORITIES - 1)
    }
}

/// ============================================================================
/// Scheduler Statistics
/// ============================================================================

/// Scheduler statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    /// Number of times scheduler was invoked
    pub schedules: u64,

    /// Number of voluntary yields
    pub yields: u64,

    /// Number of preemptions
    pub preemptions: u64,

    /// Number of idle cycles
    pub idle_cycles: u64,

    /// Current number of ready threads
    pub ready_count: usize,
}

impl SchedulerStats {
    pub const fn new() -> Self {
        Self {
            schedules: 0,
            yields: 0,
            preemptions: 0,
            idle_cycles: 0,
            ready_count: 0,
        }
    }
}

/// ============================================================================
/// Global Scheduler
/// ============================================================================

/// Per-CPU scheduler instances
///
/// In a real SMP system, this would be per-CPU data.
/// For now, we have a single global scheduler.
static mut GLOBAL_SCHEDULER: Option<Scheduler> = None;

/// Global scheduler lock
///
/// This protects the global scheduler state.
/// In a real implementation, this would be a spinlock or per-CPU data.
static SCHEDULER_LOCK: AtomicBool = AtomicBool::new(false);

/// Scheduler structure
pub struct Scheduler {
    /// Run queue
    runqueue: RunQueue,

    /// Idle thread ID
    idle_thread: Option<ThreadId>,

    /// Current CPU ID
    cpu_id: u64,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(cpu_id: u64) -> Self {
        Self {
            runqueue: RunQueue::new(),
            idle_thread: None,
            cpu_id,
        }
    }

    /// Schedule the next thread to run
    pub fn schedule(&mut self) -> Option<ThreadId> {
        // Check if we need to preempt
        if self.runqueue.is_preempt_pending() {
            let current = self.runqueue.current();

            // If we have a current thread, requeue it
            if let Some(tid) = current {
                if let Some(thread) = Self::get_thread_ref(tid) {
                    if thread.state() == ThreadState::Running {
                        // Move back to ready state
                        thread.set_state(ThreadState::Ready);

                        // Get thread priority
                        let priority = thread.priority();

                        // Re-enqueue
                        self.runqueue.enqueue(tid, priority);
                    }
                }
            }
        }

        // Select next thread
        let next = self.runqueue.select();

        // If no thread to run, use idle thread
        let tid = if let Some(tid) = next {
            tid
        } else if let Some(idle) = self.idle_thread {
            idle
        } else {
            // No idle thread - return None (CPU idle)
            self.runqueue.stats_mut().idle_cycles += 1;
            return None;
        };

        // Update current thread
        self.runqueue.set_current(Some(tid));

        // Update schedule time
        let now = Self::current_time();
        self.runqueue.update_schedule_time(now);

        log_trace!("Scheduled thread: tid={} cpu={}", tid, self.cpu_id);

        Some(tid)
    }

    /// Make a thread ready to run
    pub fn ready(&mut self, tid: ThreadId) {
        if let Some(thread) = Self::get_thread_ref(tid) {
            // Get thread priority
            let priority = thread.priority();

            // Set state to ready
            thread.set_state(ThreadState::Ready);

            // Add to run queue
            self.runqueue.enqueue(tid, priority);

            log_debug!("Thread ready: tid={} priority={}", tid, priority);
        }
    }

    /// Yield the current thread
    pub fn yield_current(&mut self) {
        if let Some(tid) = self.runqueue.current() {
            if let Some(thread) = Self::get_thread_ref(tid) {
                // Move to ready state
                thread.set_state(ThreadState::Ready);

                // Re-enqueue with same priority
                let priority = thread.priority();
                self.runqueue.enqueue(tid, priority);

                // Update statistics
                self.runqueue.stats.yields += 1;

                log_trace!("Thread yielded: tid={}", tid);
            }
        }
    }

    /// Block the current thread
    pub fn block_current(&mut self, reason: BlockReason) {
        if let Some(tid) = self.runqueue.current() {
            if let Some(thread) = Self::get_thread_ref(tid) {
                // Block the thread
                thread.block(reason);

                log_trace!("Thread blocked: tid={} reason={:?}", tid, reason);
            }
        }
    }

    /// Wake a blocked thread
    pub fn wake(&mut self, tid: ThreadId) {
        if let Some(thread) = Self::get_thread_ref(tid) {
            if thread.state() == ThreadState::Blocked {
                // Wake the thread
                thread.wake();

                // Add to run queue
                self.ready(tid);

                log_debug!("Thread woke: tid={}", tid);
            }
        }
    }

    /// Exit the current thread
    pub fn exit_current(&mut self, code: rx_status_t) {
        if let Some(tid) = self.runqueue.current() {
            if let Some(thread) = Self::get_thread_ref(tid) {
                // Exit the thread
                thread.exit(code);

                log_debug!("Thread exited: tid={} code={}", tid, code);
            }
        }
    }

    /// Handle timer tick (preemption check)
    pub fn timer_tick(&mut self) {
        // Request preemption
        self.runqueue.request_preempt();

        // Check if time slice expired
        let now = Self::current_time();
        let elapsed = now.saturating_sub(self.runqueue.last_schedule_time());

        if elapsed >= self.runqueue.time_slice() {
            self.runqueue.request_preempt();
        }
    }

    /// Set the idle thread
    pub fn set_idle_thread(&mut self, tid: ThreadId) {
        self.idle_thread = Some(tid);
    }

    /// Get statistics
    pub const fn stats(&self) -> &SchedulerStats {
        self.runqueue.stats()
    }

    /// Get current time (monotonic)
    fn current_time() -> u64 {
        // This would call into the timer subsystem
        // For now, return a stub value
        #[cfg(target_arch = "aarch64")]
        {
            crate::arch::arm64::timer::arm64_current_time()
        }

        #[cfg(target_arch = "x86_64")]
        {
            crate::kernel::arch::amd64::timer::amd64_current_time()
        }

        #[cfg(target_arch = "riscv64")]
        {
            crate::arch::riscv64::timer::riscv_current_time()
        }
    }

    /// Get a reference to a thread
    ///
    /// This is a stub - in a real implementation, this would
    /// look up the thread in a global thread table.
    fn get_thread_ref(_tid: ThreadId) -> Option<&'static Thread> {
        // Stub implementation
        None
    }
}

/// ============================================================================
/// Global Scheduler Functions
/// ============================================================================

/// Initialize the global scheduler
pub fn init_scheduler(cpu_id: u64) {
    unsafe {
        GLOBAL_SCHEDULER = Some(Scheduler::new(cpu_id));
    }
    log_info!("Scheduler initialized for CPU {}", cpu_id);
}

/// Schedule the next thread
pub fn schedule() -> Option<ThreadId> {
    with_scheduler_mut(|sched| sched.schedule())
}

/// Make a thread ready to run
pub fn ready(tid: ThreadId) {
    with_scheduler_mut(|sched| sched.ready(tid))
}

/// Yield the current thread
pub fn yield_current() {
    with_scheduler_mut(|sched| sched.yield_current());
}

/// Block the current thread
pub fn block_current(reason: BlockReason) {
    with_scheduler_mut(|sched| sched.block_current(reason));
}

/// Wake a blocked thread
pub fn wake(tid: ThreadId) {
    with_scheduler_mut(|sched| sched.wake(tid));
}

/// Exit the current thread
pub fn exit_current(code: rx_status_t) {
    with_scheduler_mut(|sched| sched.exit_current(code));
}

/// Handle timer tick
pub fn timer_tick() {
    with_scheduler_mut(|sched| sched.timer_tick());
}

/// Get scheduler statistics
pub fn get_stats() -> Option<SchedulerStats> {
    unsafe {
        GLOBAL_SCHEDULER.as_ref().map(|sched| *sched.stats())
    }
}

/// Execute a function with the scheduler
fn with_scheduler<F, R>(f: F) -> R
where
    F: FnOnce(&Scheduler) -> R,
{
    // Acquire lock
    while SCHEDULER_LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }

    let result = unsafe {
        GLOBAL_SCHEDULER.as_ref().map_or_else(
            || panic!("Scheduler not initialized"),
            |sched| f(sched),
        )
    };

    // Release lock
    SCHEDULER_LOCK.store(false, Ordering::Release);

    result
}

/// Execute a function with mutable scheduler
fn with_scheduler_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Scheduler) -> R,
{
    // Acquire lock
    while SCHEDULER_LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }

    let result = unsafe {
        GLOBAL_SCHEDULER.as_mut().map_or_else(
            || panic!("Scheduler not initialized"),
            |sched| f(sched),
        )
    };

    // Release lock
    SCHEDULER_LOCK.store(false, Ordering::Release);

    result
}

/// ============================================================================
// Module Initialization
// ============================================================================

/// Initialize the scheduler subsystem
pub fn init() {
    init_scheduler(0); // CPU 0
    log_info!("Scheduler subsystem initialized");
    log_info!("  Priority levels: {}", N_PRIORITIES);
    log_info!("  Default time slice: {} ms", DEFAULT_TIME_SLICE / 1_000_000);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_to_index() {
        let rq = RunQueue::new();

        // Test priority mapping
        assert_eq!(rq.priority_to_index(0), 0);
        assert_eq!(rq.priority_to_index(128), N_PRIORITIES / 2);
        assert_eq!(rq.priority_to_index(255), N_PRIORITIES - 1);
    }

    #[test]
    fn test_time_slice_clamping() {
        let mut rq = RunQueue::new();

        rq.set_time_slice(0);
        assert!(rq.time_slice() >= MIN_TIME_SLICE);

        rq.set_time_slice(1_000_000_000);
        assert!(rq.time_slice() <= MAX_TIME_SLICE);
    }

    #[test]
    fn test_stats_new() {
        let stats = SchedulerStats::new();
        assert_eq!(stats.schedules, 0);
        assert_eq!(stats.yields, 0);
        assert_eq!(stats.preemptions, 0);
    }
}
