// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! IPC Channels
//!
//! Channels provide bidirectional message passing between processes.
//! They support sending both bytes and handles (capability transfer).
//!
//! # Design
//!
//! - **Bidirectional**: Created as pairs of endpoints
//! - **FIFO ordering**: Messages delivered in order
//! - **Bounded queue**: Backpressure when full
//! - **Handle passing**: Handles can be transferred with rights reduction
//! - **Peer closure**: One end closed → PEER_CLOSED signal to other
//!
//! # Usage
//!
//! ```rust
//! let (channel_a, channel_b) = Channel::create()?;
//! channel_a.write(&data, &handles)?;
//! let (msg, handles) = channel_b.read(&mut buf)?;
//! ```


use crate::kernel::object::handle::{Handle, HandleId, Rights};
use crate::kernel::sync::event::{Event, EventFlags};
use crate::kernel::sync::Mutex;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// ============================================================================
/// Channel ID
/// ============================================================================

/// Channel identifier
pub type ChannelId = u64;

/// Next channel ID counter
static mut NEXT_CHANNEL_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new channel ID
fn alloc_channel_id() -> ChannelId {
    unsafe { NEXT_CHANNEL_ID.fetch_add(1, Ordering::Relaxed) }
}

/// ============================================================================
/// Message
/// ============================================================================

/// Maximum message size in bytes
pub const MAX_MSG_SIZE: usize = 64 * 1024;

/// Maximum handles per message
pub const MAX_MSG_HANDLES: usize = 64;

/// Message data
pub struct Message {
    /// Message bytes
    pub data: Vec<u8>,

    /// Handles being transferred
    pub handles: Vec<Handle>,
}

impl Message {
    /// Create a new message
    pub fn new(data: Vec<u8>, handles: Vec<Handle>) -> Self {
        Self { data, handles }
    }

    /// Get message data size
    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    /// Get handle count
    pub fn handle_count(&self) -> usize {
        self.handles.len()
    }

    /// Check if message is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty() && self.handles.is_empty()
    }
}

/// ============================================================================
/// Channel State
/// ============================================================================

/// Channel state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    /// Channel is active
    Active = 0,

    /// One endpoint closed
    PeerClosed = 1,

    /// Both endpoints closed
    Closed = 2,
}

impl ChannelState {
    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::PeerClosed,
            2 => Self::Closed,
            _ => Self::Active,
        }
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self as u32
    }
}

/// ============================================================================
/// Channel
/// ============================================================================

/// Channel endpoint
///
/// Represents one endpoint of a bidirectional channel.
pub struct Channel {
    /// Channel ID
    pub id: ChannelId,

    /// Peer channel ID
    pub peer: Mutex<Option<ChannelId>>,

    /// Message queue
    pub queue: Mutex<VecDeque<Message>>,

    /// Maximum queue depth (in bytes)
    pub max_queue_bytes: usize,

    /// Current queue size (in bytes)
    pub queue_size: AtomicUsize,

    /// Read event (signaled when messages available)
    pub read_event: Event,

    /// Write event (signaled when space available)
    pub write_event: Event,

    /// Channel state
    pub state: Mutex<ChannelState>,

    /// Number of waiters
    pub waiter_count: AtomicUsize,

    /// Reference count
    pub ref_count: AtomicUsize,
}

impl Channel {
    /// Create a channel pair
    ///
    /// # Returns
    ///
    /// Tuple of (endpoint_a, endpoint_b)
    pub fn create() -> Result<(Self, Self)> {
        let id_a = alloc_channel_id();
        let id_b = alloc_channel_id();

        let channel_a = Self {
            id: id_a,
            peer: Mutex::new(Some(id_b)),
            queue: Mutex::new(VecDeque::new()),
            max_queue_bytes: 256 * 1024, // 256KB default
            queue_size: AtomicUsize::new(0),
            read_event: Event::new(false, EventFlags::empty()),
            write_event: Event::new(true, EventFlags::empty()), // Initially writable
            state: Mutex::new(ChannelState::Active),
            waiter_count: AtomicUsize::new(0),
            ref_count: AtomicUsize::new(1),
        };

        let channel_b = Self {
            id: id_b,
            peer: Mutex::new(Some(id_a)),
            queue: Mutex::new(VecDeque::new()),
            max_queue_bytes: 256 * 1024,
            queue_size: AtomicUsize::new(0),
            read_event: Event::new(false, EventFlags::empty()),
            write_event: Event::new(true, EventFlags::empty()),
            state: Mutex::new(ChannelState::Active),
            waiter_count: AtomicUsize::new(0),
            ref_count: AtomicUsize::new(1),
        };

        Ok((channel_a, channel_b))
    }

    /// Write a message to the channel
    ///
    /// # Arguments
    ///
    /// * `data` - Message data bytes
    /// * `handles` - Handles to transfer (must have TRANSFER right)
    ///
    /// # Returns
    ///
    /// Number of bytes written
    pub fn write(&self, data: &[u8], handles: Vec<Handle>) -> Result<usize> {
        // Check state
        let state = *self.state.lock();
        if state == ChannelState::Closed {
            return Err(RX_ERR_BAD_STATE);
        }

        // Validate data size
        if data.len() > MAX_MSG_SIZE {
            return Err(RX_ERR_INVALID_ARGS);
        }

        // Validate handle count
        if handles.len() > MAX_MSG_HANDLES {
            return Err(RX_ERR_INVALID_ARGS);
        }

        // Validate handles have TRANSFER right
        for h in &handles {
            h.require(Rights::TRANSFER)?;
        }

        // Check queue capacity
        let data_size = data.len();
        let handles_size = handles.len() * core::mem::size_of::<Handle>();
        let total_size = data_size + handles_size;

        if self.queue_size.load(Ordering::Acquire) + total_size > self.max_queue_bytes {
            return Err(RX_ERR_SHOULD_WAIT);
        }

        // Check peer
        let peer_id = {
            let peer = self.peer.lock();
            *peer
        };

        if peer_id.is_none() {
            return Err(RX_ERR_PEER_CLOSED);
        }

        // Create message
        let msg = Message::new(data.to_vec(), handles);

        // Add to queue
        {
            let mut queue = self.queue.lock();
            queue.push_back(msg);
        }

        self.queue_size.fetch_add(total_size, Ordering::Release);

        // Signal read event
        self.read_event.signal();

        Ok(data_size)
    }

    /// Read a message from the channel
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to read data into
    /// * `handle_buf` - Buffer for transferred handles
    ///
    /// # Returns
    ///
    /// Tuple of (bytes_read, handles_received)
    pub fn read(&self, buf: &mut [u8], handle_buf: &mut Vec<Handle>) -> Result<(usize, usize)> {
        // Check state
        let state = *self.state.lock();
        if state == ChannelState::Closed {
            return Err(RX_ERR_BAD_STATE);
        }

        // Get message from queue
        let msg = {
            let mut queue = self.queue.lock();
            queue.pop_front().ok_or(RX_ERR_SHOULD_WAIT)?
        };

        // Calculate sizes
        let msg_data_size = msg.data_size();
        let msg_handle_count = msg.handle_count();

        // Validate buffer sizes
        if buf.len() < msg_data_size {
            return Err(RX_ERR_BUFFER_TOO_SMALL);
        }

        if handle_buf.capacity() < msg_handle_count {
            return Err(RX_ERR_BUFFER_TOO_SMALL);
        }

        // Update queue size
        let handles_size = msg_handle_count * core::mem::size_of::<Handle>();
        self.queue_size.fetch_sub(msg_data_size + handles_size, Ordering::Release);

        // Copy data
        buf[..msg_data_size].copy_from_slice(&msg.data);

        // Transfer handles
        *handle_buf = msg.handles;

        // If queue is now empty, unsignal read event
        if self.queue.lock().is_empty() {
            self.read_event.unsignal();
        }

        // Signal write event (space available)
        self.write_event.signal();

        Ok((msg_data_size, msg_handle_count))
    }

    /// Get message count in queue
    pub fn msg_count(&self) -> usize {
        self.queue.lock().len()
    }

    /// Get queue size in bytes
    pub fn queue_size(&self) -> usize {
        self.queue_size.load(Ordering::Acquire)
    }

    /// Check if peer is still alive
    pub fn is_peer_alive(&self) -> bool {
        self.peer.lock().is_some()
    }

    /// Close this endpoint
    ///
    /// Notifies peer of closure.
    pub fn close(&mut self) -> Result {
        // Clear peer reference
        let peer_id = self.peer.lock().take();

        // Update state
        *self.state.lock() = ChannelState::Closed;

        // Unsignal events
        self.read_event.unsignal();
        self.write_event.unsignal();

        // Notify peer (in real implementation, would signal peer's read event)
        let _ = peer_id;

        Ok(())
    }

    /// Handle peer closure
    ///
    /// Called when peer endpoint is closed.
    pub fn on_peer_closed(&self) {
        *self.peer.lock() = None;
        *self.state.lock() = ChannelState::PeerClosed;

        // Signal read event so reader can detect peer closure
        self.read_event.signal();
    }

    /// Increment reference count
    pub fn ref_inc(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference.
    pub fn ref_dec(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Release) == 1
    }

    /// Get waiter count
    pub fn waiter_count(&self) -> usize {
        self.waiter_count.load(Ordering::Relaxed)
    }

    /// Increment waiter count
    pub fn add_waiter(&self) {
        self.waiter_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement waiter count
    pub fn remove_waiter(&self) {
        self.waiter_count.fetch_sub(1, Ordering::Relaxed);
    }
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_create() {
        let (ch_a, ch_b) = Channel::create().unwrap();
        assert!(ch_a.is_peer_alive());
        assert!(ch_b.is_peer_alive());
        assert_eq!(ch_a.msg_count(), 0);
    }

    #[test]
    fn test_channel_state() {
        let state = ChannelState::PeerClosed;
        assert_eq!(ChannelState::from_raw(1), state);
        assert_eq!(state.into_raw(), 1);
    }

    #[test]
    fn test_message() {
        let data = vec![1, 2, 3, 4];
        let msg = Message::new(data.clone(), vec![]);

        assert_eq!(msg.data_size(), 4);
        assert_eq!(msg.handle_count(), 0);
        assert!(!msg.is_empty());
    }
}
