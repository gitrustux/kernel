// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Socket System Calls
//!
//! This module implements the socket-related system calls for stream-based IPC.
//!
//! # Syscalls Implemented
//!
//! - `rx_socket_create` - Create a socket pair
//! - `rx_socket_write` - Write to a socket
//! - `rx_socket_read` - Read from a socket
//! - `rx_socket_share` - Share a socket over another socket
//! - `rx_socket_accept` - Accept a shared socket
//! - `rx_socket_shutdown` - Shutdown socket operations
//!
//! # Design
//!
//! - Socket pairs are created together (like FIFOs)
//! - Stream-based data transfer
//! - Control plane support
//! - Socket sharing over sockets

#![no_std]

use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::sync::Mutex;
use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use crate::kernel::sync::spin::SpinMutex;

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Socket Options
/// ============================================================================

/// Socket options
pub mod socket_options {
    /// No special options
    pub const NONE: u32 = 0x00;

    /// Control plane
    pub const CONTROL: u32 = 0x01;
}

/// Shutdown options
pub mod shutdown_options {
    /// Shutdown read side
    pub const READ: u32 = 0x01;

    /// Shutdown write side
    pub const WRITE: u32 = 0x02;

    /// Shutdown both sides
    pub const READ_WRITE: u32 = 0x03;

    /// Shutdown mask
    pub const MASK: u32 = 0x03;
}

/// ============================================================================
/// Socket Registry
/// ============================================================================

/// Maximum number of sockets in the system
const MAX_SOCKETS: usize = 65536;

/// Socket entry
struct SocketEntry {
    /// Socket ID
    id: u64,

    /// Data buffer
    data: Mutex<VecDeque<u8>>,

    /// Control buffer
    control: Mutex<VecDeque<u8>>,

    /// Peer socket ID
    peer_id: AtomicU64,

    /// Write shutdown flag
    write_shutdown: AtomicBool,

    /// Read shutdown flag
    read_shutdown: AtomicBool,

    /// Pending shared socket
    pending_share: Mutex<Option<u64>>,
}

impl SocketEntry {
    /// Create a new socket entry
    pub fn new(id: u64) -> Self {
        Self {
            id,
            data: Mutex::new(VecDeque::new()),
            control: Mutex::new(VecDeque::new()),
            peer_id: AtomicU64::new(0),
            write_shutdown: AtomicBool::new(false),
            read_shutdown: AtomicBool::new(false),
            pending_share: Mutex::new(None),
        }
    }

    /// Write data to socket
    pub fn write(&self, bytes: &[u8]) -> Result<usize> {
        if self.write_shutdown.load(Ordering::Relaxed) {
            return Err(RX_ERR_PEER_CLOSED);
        }

        let mut data = self.data.lock();
        let original_len = data.len();

        for &b in bytes {
            data.push_back(b);
        }

        Ok(bytes.len())
    }

    /// Write control data to socket
    pub fn write_control(&self, bytes: &[u8]) -> Result<()> {
        if self.write_shutdown.load(Ordering::Relaxed) {
            return Err(RX_ERR_PEER_CLOSED);
        }

        let mut control = self.control.lock();

        // Control plane has limited capacity
        if control.len() > 4096 {
            return Err(RX_ERR_NO_RESOURCES);
        }

        for &b in bytes {
            control.push_back(b);
        }

        Ok(())
    }

    /// Read data from socket
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if self.read_shutdown.load(Ordering::Relaxed) {
            return Err(RX_ERR_PEER_CLOSED);
        }

        let mut data = self.data.lock();

        if data.is_empty() {
            return Err(RX_ERR_SHOULD_WAIT);
        }

        let to_read = buf.len().min(data.len());
        for i in 0..to_read {
            buf[i] = data.pop_front().unwrap();
        }

        Ok(to_read)
    }

    /// Read control data from socket
    pub fn read_control(&self, buf: &mut [u8]) -> Result<usize> {
        let mut control = self.control.lock();

        if control.is_empty() {
            return Err(RX_ERR_SHOULD_WAIT);
        }

        let to_read = buf.len().min(control.len());
        for i in 0..to_read {
            buf[i] = control.pop_front().unwrap();
        }

        Ok(to_read)
    }

    /// Share a socket over this socket
    pub fn share(&self, socket_id: u64) -> Result<()> {
        let mut pending = self.pending_share.lock();
        if pending.is_some() {
            return Err(RX_ERR_ALREADY_EXISTS);
        }
        *pending = Some(socket_id);
        Ok(())
    }

    /// Accept a shared socket
    pub fn accept(&self) -> Result<u64> {
        let mut pending = self.pending_share.lock();
        pending.take().ok_or(RX_ERR_SHOULD_WAIT)
    }

    /// Shutdown socket operations
    pub fn shutdown(&self, options: u32) -> Result {
        if options & shutdown_options::READ != 0 {
            self.read_shutdown.store(true, Ordering::Relaxed);
        }
        if options & shutdown_options::WRITE != 0 {
            self.write_shutdown.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Check if socket has pending data
    pub fn has_data(&self) -> bool {
        !self.data.lock().is_empty()
    }

    /// Check if socket has pending control data
    pub fn has_control(&self) -> bool {
        !self.control.lock().is_empty()
    }
}

/// Global socket registry
struct SocketRegistry {
    /// Socket entries
    entries: [Option<Arc<SocketEntry>>; MAX_SOCKETS],

    /// Next socket index to allocate
    next_index: AtomicUsize,

    /// Number of active sockets
    count: AtomicUsize,
}

impl SocketRegistry {
    /// Create a new socket registry
    const fn new() -> Self {
        const INIT: Option<Arc<SocketEntry>> = None;

        Self {
            entries: [INIT; MAX_SOCKETS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert a socket into the registry
    pub fn insert(&mut self, id: u64, socket: Arc<SocketEntry>) -> Result {
        let idx = (id as usize) % MAX_SOCKETS;

        if self.entries[idx].is_none() {
            self.entries[idx] = Some(socket);
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NO_RESOURCES)
        }
    }

    /// Get a socket from the registry
    pub fn get(&self, id: u64) -> Option<Arc<SocketEntry>> {
        let idx = (id as usize) % MAX_SOCKETS;
        self.entries[idx].as_ref().filter(|s| s.id == id).cloned()
    }

    /// Remove a socket from the registry
    pub fn remove(&mut self, id: u64) -> Result {
        let idx = (id as usize) % MAX_SOCKETS;

        if self.entries[idx].is_some() {
            self.entries[idx] = None;
            self.count.fetch_sub(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NOT_FOUND)
        }
    }

    /// Get the number of active sockets
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global socket registry
static SOCKET_REGISTRY: SocketRegistry = SocketRegistry::new();

/// ============================================================================
/// Socket ID Allocation
/// ============================================================================

/// Next socket ID counter
static mut NEXT_SOCKET_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new socket ID
fn alloc_socket_id() -> u64 {
    unsafe { NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed) }
}

/// ============================================================================
/// Syscall: Socket Create
/// ============================================================================

/// Create a socket pair syscall handler
///
/// # Arguments
///
/// * `options` - Creation options
/// * `handle0_out` - User pointer to store first handle
/// * `handle1_out` - User pointer to store second handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_socket_create_impl(
    options: u32,
    handle0_out: usize,
    handle1_out: usize,
) -> SyscallRet {
    log_debug!("sys_socket_create: options={:#x}", options);

    // Validate options
    if options != socket_options::NONE {
        log_error!("sys_socket_create: invalid options {:#x}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Allocate new socket IDs
    let id0 = alloc_socket_id();
    let id1 = alloc_socket_id();

    let socket0 = Arc::new(SocketEntry::new(id0));
    let socket1 = Arc::new(SocketEntry::new(id1));

    // Set peer IDs
    socket0.peer_id.store(id1, Ordering::Relaxed);
    socket1.peer_id.store(id0, Ordering::Relaxed);

    // Insert into socket registry
    if let Err(err) = SOCKET_REGISTRY.insert(id0, socket0.clone()) {
        log_error!("sys_socket_create: failed to insert socket0: {:?}", err);
        return err_to_ret(err);
    }

    if let Err(err) = SOCKET_REGISTRY.insert(id1, socket1.clone()) {
        log_error!("sys_socket_create: failed to insert socket1: {:?}", err);
        SOCKET_REGISTRY.remove(id0);
        return err_to_ret(err);
    }

    // Write handles to user space
    if handle0_out != 0 {
        let user_ptr = UserPtr::<u64>::new(handle0_out);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, &id0 as *const u64 as *const u8, 8) {
                log_error!("sys_socket_create: copy_to_user failed for handle0: {:?}", err);
                SOCKET_REGISTRY.remove(id0);
                SOCKET_REGISTRY.remove(id1);
                return err_to_ret(err.into());
            }
        }
    }

    if handle1_out != 0 {
        let user_ptr = UserPtr::<u64>::new(handle1_out);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, &id1 as *const u64 as *const u8, 8) {
                log_error!("sys_socket_create: copy_to_user failed for handle1: {:?}", err);
                SOCKET_REGISTRY.remove(id0);
                SOCKET_REGISTRY.remove(id1);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_socket_create: success socket0={} socket1={}", id0, id1);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Socket Write
/// ============================================================================

/// Write to a socket syscall handler
///
/// # Arguments
///
/// * `handle_val` - Socket handle value
/// * `options` - Write options
/// * `buffer` - User pointer to data
/// * `size` - Number of bytes to write
/// * `actual_out` - User pointer to store actual bytes written
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_socket_write_impl(
    handle_val: u32,
    options: u32,
    buffer: usize,
    size: usize,
    actual_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_socket_write: handle={:#x} options={:#x} size={}",
        handle_val, options, size
    );

    // Validate buffer
    if size > 0 && buffer == 0 {
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up socket
    let socket_id = handle_val as u64;
    let socket = match SOCKET_REGISTRY.get(socket_id) {
        Some(s) => s,
        None => {
            log_error!("sys_socket_write: socket not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Allocate buffer for data
    let mut buf = alloc::vec![0u8; size];

    // Copy data from user
    let user_ptr = UserPtr::<u8>::new(buffer);
    unsafe {
        if let Err(err) = copy_from_user(buf.as_mut_ptr(), user_ptr, size) {
            log_error!("sys_socket_write: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Write to socket
    let actual = match options {
        socket_options::NONE => socket.write(&buf)?,
        socket_options::CONTROL => {
            socket.write_control(&buf)?;
            size
        }
        _ => {
            log_error!("sys_socket_write: invalid options {:#x}", options);
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    // Write actual count to user
    if actual_out != 0 {
        let user_ptr = UserPtr::<usize>::new(actual_out);
        unsafe {
            if let Err(err) = copy_to_user(
                user_ptr,
                &actual as *const usize as *const u8,
                core::mem::size_of::<usize>(),
            ) {
                log_error!("sys_socket_write: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_socket_write: success wrote {}", actual);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Socket Read
/// ============================================================================

/// Read from a socket syscall handler
///
/// # Arguments
///
/// * `handle_val` - Socket handle value
/// * `options` - Read options
/// * `buffer` - User pointer to buffer
/// * `size` - Number of bytes to read
/// * `actual_out` - User pointer to store actual bytes read
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_socket_read_impl(
    handle_val: u32,
    options: u32,
    buffer: usize,
    size: usize,
    actual_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_socket_read: handle={:#x} options={:#x} size={}",
        handle_val, options, size
    );

    // Validate buffer
    if size > 0 && buffer == 0 {
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up socket
    let socket_id = handle_val as u64;
    let socket = match SOCKET_REGISTRY.get(socket_id) {
        Some(s) => s,
        None => {
            log_error!("sys_socket_read: socket not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Allocate buffer for data
    let mut buf = alloc::vec![0u8; size];

    // Read from socket
    let actual = match options {
        socket_options::NONE => socket.read(&mut buf)?,
        socket_options::CONTROL => socket.read_control(&mut buf)?,
        _ => {
            log_error!("sys_socket_read: invalid options {:#x}", options);
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    // Copy data to user (only what was actually read)
    if actual > 0 {
        let user_ptr = UserPtr::<u8>::new(buffer);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, buf.as_ptr(), actual) {
                log_error!("sys_socket_read: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    // Write actual count to user
    if actual_out != 0 {
        let user_ptr = UserPtr::<usize>::new(actual_out);
        unsafe {
            if let Err(err) = copy_to_user(
                user_ptr,
                &actual as *const usize as *const u8,
                core::mem::size_of::<usize>(),
            ) {
                log_error!("sys_socket_read: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_socket_read: success read {}", actual);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Socket Share
/// ============================================================================

/// Share a socket over another socket syscall handler
///
/// # Arguments
///
/// * `handle_val` - Socket handle value
/// * `socket_to_share` - Socket handle to share
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_socket_share_impl(handle_val: u32, socket_to_share: u32) -> SyscallRet {
    log_debug!(
        "sys_socket_share: handle={:#x} socket_to_share={:#x}",
        handle_val, socket_to_share
    );

    // Look up socket
    let socket_id = handle_val as u64;
    let socket = match SOCKET_REGISTRY.get(socket_id) {
        Some(s) => s,
        None => {
            log_error!("sys_socket_share: socket not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Validate socket to share
    let share_id = socket_to_share as u64;
    if !SOCKET_REGISTRY.get(share_id).is_some() {
        return err_to_ret(RX_ERR_BAD_HANDLE);
    }

    // Share the socket
    socket.share(share_id)?;

    log_debug!("sys_socket_share: success");

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Socket Accept
/// ============================================================================

/// Accept a shared socket syscall handler
///
/// # Arguments
///
/// * `handle_val` - Socket handle value
/// * `handle_out` - User pointer to store accepted socket handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_socket_accept_impl(handle_val: u32, handle_out: usize) -> SyscallRet {
    log_debug!("sys_socket_accept: handle={:#x}", handle_val);

    // Look up socket
    let socket_id = handle_val as u64;
    let socket = match SOCKET_REGISTRY.get(socket_id) {
        Some(s) => s,
        None => {
            log_error!("sys_socket_accept: socket not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Accept shared socket
    let accepted_id = socket.accept()?;

    // Write handle to user
    if handle_out != 0 {
        let user_ptr = UserPtr::<u64>::new(handle_out);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, &accepted_id as *const u64 as *const u8, 8) {
                log_error!("sys_socket_accept: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_socket_accept: success accepted={}", accepted_id);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Socket Shutdown
/// ============================================================================

/// Shutdown socket operations syscall handler
///
/// # Arguments
///
/// * `handle_val` - Socket handle value
/// * `options` - Shutdown options
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_socket_shutdown_impl(handle_val: u32, options: u32) -> SyscallRet {
    log_debug!(
        "sys_socket_shutdown: handle={:#x} options={:#x}",
        handle_val, options
    );

    // Validate options
    if options & shutdown_options::MASK != options {
        log_error!("sys_socket_shutdown: invalid options {:#x}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up socket
    let socket_id = handle_val as u64;
    let socket = match SOCKET_REGISTRY.get(socket_id) {
        Some(s) => s,
        None => {
            log_error!("sys_socket_shutdown: socket not found");
            return err_to_ret(RX_ERR_BAD_HANDLE);
        }
    };

    // Shutdown socket
    socket.shutdown(options)?;

    log_debug!("sys_socket_shutdown: success");

    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get socket subsystem statistics
pub fn get_stats() -> SocketStats {
    SocketStats {
        total_sockets: SOCKET_REGISTRY.count(),
        total_bytes_sent: 0, // TODO: Track bytes sent
        total_bytes_received: 0, // TODO: Track bytes received
    }
}

/// Socket subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SocketStats {
    /// Total number of sockets
    pub total_sockets: usize,

    /// Total bytes sent
    pub total_bytes_sent: u64,

    /// Total bytes received
    pub total_bytes_received: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the socket syscall subsystem
pub fn init() {
    log_info!("Socket syscall subsystem initialized");
    log_info!("  Max sockets: {}", MAX_SOCKETS);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_create() {
        let result = sys_socket_create_impl(0, 0, 0);
        assert!(result >= 0);
    }

    #[test]
    fn test_socket_create_invalid_options() {
        let result = sys_socket_create_impl(0xFF, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_socket_write_invalid_handle() {
        let result = sys_socket_write_impl(0, 0, 0, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_socket_read_invalid_handle() {
        let result = sys_socket_read_impl(0, 0, 0, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_socket_shutdown_invalid_options() {
        // First create a socket
        sys_socket_create_impl(0, 0, 0);

        // Try invalid shutdown options
        let result = sys_socket_shutdown_impl(1, 0xFF);
        assert!(result < 0);
    }
}
