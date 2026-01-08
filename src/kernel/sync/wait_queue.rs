// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Wait Queue
//!
//! This module provides wait queues for the Rustux kernel.
//! Wait queues are used by synchronization primitives to
//! manage threads that are blocked waiting for an event.
//!
//! # Design
//!
//! - **Priority-ordered**: Threads queued by priority (higher first)
//! - **Fair ordering**: FIFO within same priority level
//! - **Multiple waiters**: Can handle many threads waiting simultaneously
//! - **Integrates with scheduler**: Properly blocks and wakes threads
//!
//! # Usage
//!
//! ```rust
//! let wq = WaitQueue::new();
//!
//! // Block current thread on the wait queue
//! wq.block(u64::MAX); // Infinite timeout
//!
//! // Wake one thread
//! wq.wake_one();
//!
//! // Wake all threads
//! wq.wake_all();
//! ```


use crate::kernel::thread::ThreadId;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::collections::VecDeque;
use crate::kernel::sync::spin::SpinMutex;
use crate::kernel::sync::Mutex;

// Import logging macros
use crate::log_debug;

/// ============================================================================
/// Wait Queue
/// ============================================================================

/// Magic number for wait queue validation
const WAIT_QUEUE_MAGIC: u32 = 0x57414954; // "WAIT" in hex

/// Wait queue entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WaitQueueEntry {
    /// Thread ID waiting
    pub tid: ThreadId,

    /// Thread priority
    pub priority: u8,

    /// Result code to return when woken
    pub wait_result: rx_status_t,
}

/// Wait queue
///
/// Manages threads waiting for a condition to become true.
pub struct WaitQueue {
    /// Queue of waiting threads (priority-sorted)
    queue: Mutex<VecDeque<WaitQueueEntry>>,

    /// Magic number for validation
    magic: u32,

    /// Number of threads currently waiting
    count: core::sync::atomic::AtomicUsize,
}

impl WaitQueue {
    /// Create a new wait queue
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            magic: WAIT_QUEUE_MAGIC,
            count: core::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Initialize a wait queue (for heap-allocated queues)
    pub fn init(&mut self) {
        self.queue.lock().clear();
        self.magic = WAIT_QUEUE_MAGIC;
        self.count.store(0, core::sync::atomic::Ordering::Release);
    }

    /// Destroy a wait queue
    ///
    /// Panics if there are threads still waiting.
    pub fn destroy(&self) {
        self.validate();

        if !self.is_empty() {
            panic!("wait_queue_destroy: threads still waiting");
        }
    }

    /// Block the current thread on this wait queue
    ///
    /// # Arguments
    ///
    /// * `deadline` - Deadline in nanoseconds (u64::MAX = infinite)
    ///
    /// # Returns
    ///
    /// - `Ok(())` if woken successfully
    /// - `Err(RX_ERR_TIMED_OUT)` if deadline reached
    pub fn block(&self, deadline: u64) -> Result {
        self.validate();

        // Get current thread (stub)
        let tid = ThreadId::current();
        let priority = 128; // Default priority

        // Add to queue
        {
            let mut queue = self.queue.lock();
            self.insert_sorted(&mut queue, WaitQueueEntry {
                tid,
                priority,
                wait_result: RX_OK,
            });
        }

        self.count.fetch_add(1, core::sync::atomic::Ordering::Release);

        // Block the thread
        log_debug!("Thread {} blocking on wait queue", tid);

        // In a real implementation, this would:
        // 1. Set thread state to Blocked
        // 2. Add to scheduler wait queues
        // 3. Check deadline periodically
        // 4. Return when woken or timed out

        // TODO: Integrate with scheduler
        // crate::kernel::sched::block_current(BlockReason::WaitQueue);

        Ok(())
    }

    /// Wake one thread from the wait queue
    ///
    /// # Returns
    ///
    /// Number of threads woken (0 or 1)
    pub fn wake_one(&self) -> i32 {
        self.validate();
        self.wake(1, false)
    }

    /// Wake all threads from the wait queue
    ///
    /// # Returns
    ///
    /// Number of threads woken
    pub fn wake_all(&self) -> i32 {
        self.validate();
        self.wake(i32::MAX, false)
    }

    /// Wake threads with a result code
    ///
    /// # Arguments
    ///
    /// * `result` - Result code to return to woken threads
    ///
    /// # Returns
    ///
    /// Number of threads woken
    pub fn wake_one_with_result(&self, result: rx_status_t) -> i32 {
        self.validate();

        let entry = {
            let mut queue = self.queue.lock();
            queue.pop_front()
        };

        if let Some(mut entry) = entry {
            entry.wait_result = result;
            self.wake_thread(entry);
            self.count.fetch_sub(1, core::sync::atomic::Ordering::Release);
            1
        } else {
            0
        }
    }

    /// Check if the wait queue is empty
    pub fn is_empty(&self) -> bool {
        self.count.load(core::sync::atomic::Ordering::Acquire) == 0
    }

    /// Get the number of waiting threads
    pub fn len(&self) -> usize {
        self.count.load(core::sync::atomic::Ordering::Acquire)
    }

    /// Remove a specific thread from the wait queue
    ///
    /// # Arguments
    ///
    /// * `tid` - Thread ID to remove
    ///
    /// # Returns
    ///
    /// true if thread was found and removed, false otherwise
    pub fn remove(&self, tid: ThreadId) -> bool {
        self.validate();

        let mut queue = self.queue.lock();
        let len_before = queue.len();

        queue.retain(|entry| entry.tid != tid);

        let removed = len_before > queue.len();
        if removed {
            self.count.fetch_sub(1, core::sync::atomic::Ordering::Release);
        }

        removed
    }

    /// Internal wake implementation
    fn wake(&self, max: i32, reschedule: bool) -> i32 {
        let mut count = 0;

        while count < max {
            let entry = {
                let mut queue = self.queue.lock();
                queue.pop_front()
            };

            match entry {
                Some(entry) => {
                    self.wake_thread(entry);
                    self.count.fetch_sub(1, core::sync::atomic::Ordering::Release);
                    count += 1;
                }
                None => break,
            }
        }

        // Reschedule if requested and we woke something
        if reschedule && count > 0 {
            // TODO: crate::kernel::sched::yield_current();
        }

        count
    }

    /// Wake a single thread
    fn wake_thread(&self, entry: WaitQueueEntry) {
        log_debug!("Waking thread {} from wait queue", entry.tid);

        // In a real implementation, this would:
        // 1. Set thread state to Ready
        // 2. Set the thread's wait_result
        // 3. Add to scheduler run queue

        // TODO: Integrate with scheduler
        // crate::kernel::sched::wake(entry.tid);
    }

    /// Insert entry into priority-sorted queue
    fn insert_sorted(&self, queue: &mut VecDeque<WaitQueueEntry>, entry: WaitQueueEntry) {
        // Find insertion point (higher priority first)
        let mut pos = 0;
        for (i, e) in queue.iter().enumerate() {
            if entry.priority > e.priority {
                pos = i;
                break;
            }
            pos = i + 1;
        }

        queue.insert(pos, entry);
    }

    /// Validate that this is a valid wait queue
    fn validate(&self) {
        debug_assert_eq!(self.magic, WAIT_QUEUE_MAGIC, "invalid wait queue magic");
    }
}

/// ============================================================================
/// ThreadId Extension
/// ============================================================================

pub trait ThreadIdExt {
    fn current() -> Self;
}

impl ThreadIdExt for ThreadId {
    fn current() -> Self {
        // Stub implementation
        1
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_queue_new() {
        let wq = WaitQueue::new();
        assert!(wq.is_empty());
        assert_eq!(wq.len(), 0);
    }

    #[test]
    fn test_wait_queue_magic() {
        let wq = WaitQueue::new();
        assert_eq!(wq.magic, WAIT_QUEUE_MAGIC);
    }

    #[test]
    fn test_wait_queue_remove() {
        let wq = WaitQueue::new();

        // Try to remove non-existent thread
        assert!(!wq.remove(999));
    }

    #[test]
    fn test_wait_queue_entry() {
        let entry = WaitQueueEntry {
            tid: 1,
            priority: 128,
            wait_result: RX_OK,
        };

        assert_eq!(entry.tid, 1);
        assert_eq!(entry.priority, 128);
    }
}
