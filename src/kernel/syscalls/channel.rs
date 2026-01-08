// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Channel System Calls
//!
//! This module implements the Channel system calls for IPC.
//! Channels provide bidirectional message passing between processes.
//!
//! # Syscalls Implemented
//!
//! - `rx_channel_create` - Create a channel pair
//! - `rx_channel_write` - Write to a channel
//! - `rx_channel_read` - Read from a channel
//!
//! # Design
//!
//! - Channels are created as pairs of endpoints
//! - Messages contain both data bytes and handles
//! - Handles are transferred with rights validation
//! - FIFO ordering guaranteed
//! - Bounded queue with backpressure


use crate::kernel::object::channel::{self, Channel, ChannelId, Message, MAX_MSG_HANDLES, MAX_MSG_SIZE};
use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::sync::Mutex;
use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::vm::layout::*;
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::vec;

// Import logging macros
use crate::{log_debug, log_error, log_info};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// ============================================================================
/// Channel Registry
/// ============================================================================

/// Maximum number of channels in the system
const MAX_CHANNELS: usize = 65536;

/// Channel registry entry
struct ChannelEntry {
    /// Channel ID
    id: ChannelId,

    /// Channel object
    channel: Arc<Channel>,
}

/// Global channel registry
///
/// Maps channel IDs to channel objects. This is used to resolve handles to channels.
struct ChannelRegistry {
    /// Channel entries
    entries: [Option<ChannelEntry>; MAX_CHANNELS],

    /// Next channel index to allocate
    next_index: AtomicUsize,

    /// Number of active channels
    count: AtomicUsize,
}

impl ChannelRegistry {
    /// Create a new channel registry
    const fn new() -> Self {
        const INIT: Option<ChannelEntry> = None;

        Self {
            entries: [INIT; MAX_CHANNELS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert a channel into the registry
    pub fn insert(&mut self, channel: Arc<Channel>) -> Result<ChannelId> {
        let id = channel.id;

        // Find a free slot
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (id as usize) % MAX_CHANNELS;

        loop {
            // Try to allocate at current index
            if self.entries[idx].is_none() {
                self.entries[idx] = Some(ChannelEntry { id, channel });
                self.count.fetch_add(1, Ordering::Relaxed);
                self.next_index.store((idx + 1) % MAX_CHANNELS, Ordering::Relaxed);
                return Ok(id);
            }

            // Linear probe
            idx = (idx + 1) % MAX_CHANNELS;

            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    /// Get a channel from the registry
    pub fn get(&self, id: ChannelId) -> Option<Arc<Channel>> {
        let idx = (id as usize) % MAX_CHANNELS;

        self.entries[idx]
            .as_ref()
            .filter(|entry| entry.id == id)
            .map(|entry| entry.channel.clone())
    }

    /// Remove a channel from the registry
    pub fn remove(&mut self, id: ChannelId) -> Option<Arc<Channel>> {
        let idx = (id as usize) % MAX_CHANNELS;

        if let Some(entry) = self.entries[idx].take() {
            if entry.id == id {
                self.count.fetch_sub(1, Ordering::Relaxed);
                return Some(entry.channel);
            }
        }

        None
    }

    /// Get the number of active channels
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

// SAFETY: ChannelRegistry is accessed only through a Mutex and contains Arc which is thread-safe
unsafe impl Send for ChannelRegistry {}
unsafe impl Sync for ChannelRegistry {}

/// Global channel registry
static CHANNEL_REGISTRY: Mutex<ChannelRegistry> = Mutex::new(ChannelRegistry::new());

/// ============================================================================
/// Handle to Channel Resolution
/// ============================================================================

/// Get the current process's handle table
///
/// This is a placeholder that returns NULL for now.
/// In a real implementation, this would use thread-local storage
/// or per-CPU data to get the current process.
fn current_process_handle_table() -> Option<&'static HandleTable> {
    // TODO: Implement proper current process tracking
    // For now, return None to indicate not implemented
    None
}

/// Look up a channel from a handle value
///
/// This function:
/// 1. Gets the current process's handle table
/// 2. Looks up the handle in the table
/// 3. Validates the handle type and rights
/// 4. Returns the channel object
fn lookup_channel_from_handle(
    handle_val: u32,
    required_rights: Rights,
) -> Result<(Arc<Channel>, Handle)> {
    // Get current process handle table
    let handle_table = current_process_handle_table()
        .ok_or(RX_ERR_NOT_SUPPORTED)?;

    // Get the handle from the table
    let handle = handle_table.get(handle_val)
        .ok_or(RX_ERR_INVALID_ARGS)?;

    // Validate object type
    if handle.obj_type() != ObjectType::Channel {
        return Err(RX_ERR_WRONG_TYPE);
    }

    // Validate rights
    handle.require(required_rights)?;

    // Get channel ID from handle (stored as part of base pointer for now)
    // In a real implementation, the handle would store the channel ID directly
    let channel_id = handle.id as ChannelId;

    // Get channel from registry
    let channel = CHANNEL_REGISTRY.lock().get(channel_id)
        .ok_or(RX_ERR_NOT_FOUND)?;

    Ok((channel, handle))
}

/// ============================================================================
/// Channel Kernel Object Base
/// ============================================================================

/// Create a kernel object base for a channel
fn channel_to_kernel_base(channel: &Arc<Channel>) -> KernelObjectBase {
    KernelObjectBase::new(ObjectType::Channel)
}

/// ============================================================================
/// Syscall: Channel Create
/// ============================================================================

/// Create a new channel pair syscall handler
///
/// # Arguments
///
/// * `args` - Syscall arguments
///   - args[0]: Options (must be 0)
///
/// # Returns
///
/// * On success: Returns two handle values (encoded in lower/higher 32 bits)
/// * On error: Negative error code
pub fn sys_channel_create_impl(options: u32) -> SyscallRet {
    log_debug!("sys_channel_create: options={}", options);

    // Validate options (must be 0)
    if options != 0 {
        log_error!("sys_channel_create: invalid options {}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Create the channel pair
    let (channel_a, channel_b) = match Channel::create() {
        Ok(channels) => channels,
        Err(err) => {
            log_error!("sys_channel_create: failed to create channel: {:?}", err);
            return err_to_ret(err);
        }
    };

    log_debug!("sys_channel_create: created channels id_a={} id_b={}",
        channel_a.id, channel_b.id);

    // Wrap in Arc for registry
    let channel_a_arc = Arc::new(channel_a);
    let channel_b_arc = Arc::new(channel_b);

    // Insert into channel registry
    let id_a = match CHANNEL_REGISTRY.lock().insert(channel_a_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_channel_create: failed to insert channel_a: {:?}", err);
            return err_to_ret(err);
        }
    };

    let id_b = match CHANNEL_REGISTRY.lock().insert(channel_b_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_channel_create: failed to insert channel_b: {:?}", err);
            // Cleanup channel_a
            CHANNEL_REGISTRY.lock().remove(id_a);
            return err_to_ret(err);
        }
    };

    // Create kernel object bases
    let base_a = channel_to_kernel_base(&channel_a_arc);
    let base_b = channel_to_kernel_base(&channel_b_arc);

    // Create handles with default rights (READ | WRITE)
    let rights = Rights::READ | Rights::WRITE;
    let handle_a = Handle::new(&base_a as *const KernelObjectBase, rights);
    let handle_b = Handle::new(&base_b as *const KernelObjectBase, rights);

    // TODO: Add handles to current process's handle table
    // For now, return the channel IDs as the handle values
    let handle_value_a = id_a as u32;
    let handle_value_b = id_b as u32;

    // Pack two handles into return value: lower 32 bits = handle_a, upper 32 bits = handle_b
    let packed = (handle_value_b as u64) << 32 | (handle_value_a as u64);

    log_debug!("sys_channel_create: success handle_a={} handle_b={}",
        handle_value_a, handle_value_b);

    ok_to_ret(packed as usize)
}

/// ============================================================================
/// Syscall: Channel Write
/// ============================================================================

/// Write to channel syscall handler
///
/// # Arguments
///
/// * `handle_val` - Channel handle value
/// * `options` - Options (must be 0)
/// * `user_data` - User pointer to message data
/// * `data_size` - Size of message data
/// * `user_handles` - User pointer to handles array
/// * `handle_count` - Number of handles to transfer
///
/// # Returns
///
/// * On success: Number of bytes written
/// * On error: Negative error code
pub fn sys_channel_write_impl(
    handle_val: u32,
    options: u32,
    user_data: usize,
    data_size: usize,
    user_handles: usize,
    handle_count: usize,
) -> SyscallRet {
    log_debug!(
        "sys_channel_write: handle={} options={} data={:#x} size={} handles={:#x} count={}",
        handle_val, options, user_data, data_size, user_handles, handle_count
    );

    // Validate options (must be 0)
    if options != 0 {
        log_error!("sys_channel_write: invalid options {}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate data size
    if data_size > MAX_MSG_SIZE {
        log_error!("sys_channel_write: data size too large: {}", data_size);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate handle count
    if handle_count > MAX_MSG_HANDLES {
        log_error!("sys_channel_write: handle count too large: {}", handle_count);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up channel from handle (requires WRITE right)
    let (channel, _handle) = match lookup_channel_from_handle(handle_val, Rights::WRITE) {
        Ok(ch) => ch,
        Err(err) => {
            log_error!("sys_channel_write: failed to lookup channel: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Read data from user space
    let mut data = vec![0u8; data_size];
    if data_size > 0 {
        let user_ptr = UserPtr::new(user_data);
        unsafe {
            if let Err(err) = copy_from_user(data.as_mut_ptr(), user_ptr, data_size) {
                log_error!("sys_channel_write: copy_from_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    // Read handles from user space
    let mut handles = Vec::new();
    if handle_count > 0 {
        let user_handles_ptr = UserPtr::<u32>::new(user_handles);

        // For each handle, look it up in the handle table and validate
        // TODO: This is a simplified version - in a real implementation,
        // we would remove the handles from the sender's handle table
        // and transfer them to the receiver
        for i in 0..handle_count {
            // This is a placeholder - in a real implementation,
            // we would read the handle value from user space
            // and validate it has the TRANSFER right
            let _ = i;
        }
    }

    // Write message to channel
    match channel.write(&data, handles) {
        Ok(bytes_written) => {
            log_debug!("sys_channel_write: success bytes_written={}", bytes_written);
            ok_to_ret(bytes_written)
        }
        Err(err) => {
            log_error!("sys_channel_write: channel write failed: {:?}", err);
            err_to_ret(err)
        }
    }
}

/// ============================================================================
/// Syscall: Channel Read
/// ============================================================================

/// Read from channel syscall handler
///
/// # Arguments
///
/// * `handle_val` - Channel handle value
/// * `options` - Read options (MAY_DISCARD)
/// * `user_data` - User pointer to data buffer
/// * `data_capacity` - Capacity of data buffer
/// * `user_handles` - User pointer to handles buffer
/// * `handles_capacity` - Capacity of handles buffer
///
/// # Returns
///
/// * On success: Number of bytes read (encoded with handle count in upper bits)
/// * On error: Negative error code
///
/// # Return Value Encoding
///
/// The return value encodes both bytes read and handles received:
/// - Lower 32 bits: bytes read
/// - Upper 32 bits: handles received
pub fn sys_channel_read_impl(
    handle_val: u32,
    options: u32,
    user_data: usize,
    data_capacity: usize,
    user_handles: usize,
    handles_capacity: usize,
) -> SyscallRet {
    log_debug!(
        "sys_channel_read: handle={} options={} data={:#x} cap={} handles={:#x} cap={}",
        handle_val, options, user_data, data_capacity, user_handles, handles_capacity
    );

    // Validate options (only MAY_DISCARD allowed)
    const MAY_DISCARD: u32 = 1;
    if options & !MAY_DISCARD != 0 {
        log_error!("sys_channel_read: invalid options {}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up channel from handle (requires READ right)
    let (channel, _handle) = match lookup_channel_from_handle(handle_val, Rights::READ) {
        Ok(ch) => ch,
        Err(err) => {
            log_error!("sys_channel_read: failed to lookup channel: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Allocate buffers for reading
    let mut data_buf = vec![0u8; data_capacity];
    let mut handle_buf = Vec::with_capacity(handles_capacity);

    // Read message from channel
    let (bytes_read, handles_received) = match channel.read(&mut data_buf, &mut handle_buf) {
        Ok(result) => result,
        Err(err) => {
            // Check if buffer too small
            if err == RX_ERR_BUFFER_TOO_SMALL {
                // In a real implementation, we would query the message size
                // and return it to the user
                log_debug!("sys_channel_read: buffer too small");
            }
            log_error!("sys_channel_read: channel read failed: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Copy data to user space
    if bytes_read > 0 {
        let user_ptr = UserPtr::new(user_data);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, data_buf.as_ptr(), bytes_read) {
                log_error!("sys_channel_read: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    // Copy handles to user space
    if handles_received > 0 {
        let user_handles_ptr = UserPtr::<u32>::new(user_handles);

        // TODO: Transfer handles to the receiving process
        // In a real implementation, we would add the handles to the
        // receiver's handle table and write the new handle values
        for i in 0..handles_received {
            // This is a placeholder
            let _ = (user_handles_ptr, i);
        }
    }

    // Pack result: lower 32 bits = bytes, upper 32 bits = handles
    let packed = (handles_received as u64) << 32 | (bytes_read as u64);

    log_debug!("sys_channel_read: success bytes_read={} handles={}",
        bytes_read, handles_received);

    ok_to_ret(packed as usize)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get channel subsystem statistics
pub fn get_stats() -> ChannelStats {
    ChannelStats {
        total_channels: CHANNEL_REGISTRY.lock().count(),
        total_messages: 0, // TODO: Track total messages
        total_bytes: 0,    // TODO: Track total bytes
    }
}

/// Channel subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChannelStats {
    /// Total number of channels
    pub total_channels: usize,

    /// Total messages sent
    pub total_messages: u64,

    /// Total bytes sent
    pub total_bytes: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the channel syscall subsystem
pub fn init() {
    log_info!("Channel syscall subsystem initialized");
    log_info!("  Max channels: {}", MAX_CHANNELS);
    log_info!("  Max message size: {}", MAX_MSG_SIZE);
    log_info!("  Max handles per message: {}", MAX_MSG_HANDLES);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_registry_insert_get() {
        let (channel_a, _channel_b) = Channel::create().unwrap();
        let channel_arc = Arc::new(channel_a);

        let id = CHANNEL_REGISTRY.lock().insert(channel_arc.clone()).unwrap();
        assert_eq!(id, channel_arc.id);

        let retrieved = CHANNEL_REGISTRY.lock().get(id).unwrap();
        assert_eq!(retrieved.id, channel_arc.id);
    }

    #[test]
    fn test_channel_registry_remove() {
        let (channel_a, _channel_b) = Channel::create().unwrap();
        let channel_arc = Arc::new(channel_a);

        let id = CHANNEL_REGISTRY.lock().insert(channel_arc.clone()).unwrap();
        let removed = CHANNEL_REGISTRY.lock().remove(id).unwrap();

        assert_eq!(removed.id, channel_arc.id);
        assert!(CHANNEL_REGISTRY.lock().get(id).is_none());
    }

    #[test]
    fn test_channel_create() {
        let (ch_a, ch_b) = Channel::create().unwrap();
        assert!(ch_a.is_peer_alive());
        assert!(ch_b.is_peer_alive());
        assert_eq!(ch_a.msg_count(), 0);
        assert_eq!(ch_b.msg_count(), 0);
    }

    #[test]
    fn test_channel_write_read() {
        let (ch_a, ch_b) = Channel::create().unwrap();
        let ch_a_arc = Arc::new(ch_a);
        let ch_b_arc = Arc::new(ch_b);

        let data = b"Hello, Channel!";
        let handles = vec![];

        // Write to channel A
        let written = ch_a_arc.write(data, handles).unwrap();
        assert_eq!(written, data.len());

        // Read from channel B
        let mut buf = [0u8; 64];
        let mut handle_buf = Vec::new();
        let (bytes_read, handles_read) = ch_b_arc.read(&mut buf, &mut handle_buf).unwrap();

        assert_eq!(bytes_read, data.len());
        assert_eq!(handles_read, 0);
        assert_eq!(&buf[..data.len()], data);
    }

    #[test]
    fn test_message_sizes() {
        assert!(MAX_MSG_SIZE <= 64 * 1024);
        assert!(MAX_MSG_HANDLES <= 64);
    }
}
