// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! FIFO System Calls
//!
//! This module implements the FIFO (First-In-First-Out) queue system calls.
//! FIFOs provide a simple byte-stream queue for IPC.
//!
//! # Syscalls Implemented
//!
//! - `rx_fifo_create` - Create a FIFO pair
//! - `rx_fifo_write` - Write to a FIFO
//! - `rx_fifo_read` - Read from a FIFO
//!
//! # Design
//!
//! - FIFO pairs are created together (like eventpairs)
//! - Fixed element size
//! - Fixed capacity
//! - Non-blocking reads/writes


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
/// FIFO Options
/// ============================================================================

/// FIFO options
pub mod fifo_options {
    /// No special options
    pub const NONE: u32 = 0x00;
}

/// ============================================================================
/// FIFO Registry
/// ============================================================================

/// Maximum number of FIFOs in the system
const MAX_FIFOS: usize = 32768;

/// FIFO entry
struct FifoEntry {
    /// FIFO ID
    id: u64,

    /// Element size
    elem_size: usize,

    /// Capacity (number of elements)
    capacity: usize,

    /// Data queue
    data: Mutex<VecDeque<u8>>,

    /// Peer FIFO ID
    peer_id: AtomicU64,

    /// Closed flag
    closed: AtomicBool,
}

impl FifoEntry {
    /// Create a new FIFO entry
    pub fn new(id: u64, elem_size: usize, capacity: usize) -> Self {
        Self {
            id,
            elem_size,
            capacity,
            data: Mutex::new(VecDeque::new()),
            peer_id: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        }
    }

    /// Get available read size
    pub fn available_read(&self) -> usize {
        let data = self.data.lock();
        data.len() / self.elem_size
    }

    /// Get available write space
    pub fn available_write(&self) -> usize {
        let data = self.data.lock();
        (self.capacity * self.elem_size - data.len()) / self.elem_size
    }

    /// Write data to FIFO
    pub fn write(&self, bytes: &[u8]) -> Result<usize> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(RX_ERR_PEER_CLOSED);
        }

        let mut data = self.data.lock();
        let avail = (self.capacity * self.elem_size) - data.len();

        if avail == 0 {
            return Err(RX_ERR_SHOULD_WAIT);
        }

        let to_write = bytes.len().min(avail);
        for &b in bytes.iter().take(to_write) {
            data.push_back(b);
        }

        Ok(to_write)
    }

    /// Read data from FIFO
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut data = self.data.lock();

        if data.is_empty() {
            if self.closed.load(Ordering::Relaxed) {
                return Err(RX_ERR_PEER_CLOSED);
            }
            return Err(RX_ERR_SHOULD_WAIT);
        }

        let to_read = buf.len().min(data.len());
        for i in 0..to_read {
            buf[i] = data.pop_front().unwrap();
        }

        Ok(to_read)
    }

    /// Close the FIFO
    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }

    /// Check if FIFO is closed
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }
}

/// Global FIFO registry
struct FifoRegistry {
    /// FIFO entries
    entries: [Option<Arc<FifoEntry>>; MAX_FIFOS],

    /// Next FIFO index to allocate
    next_index: AtomicUsize,

    /// Number of active FIFOs
    count: AtomicUsize,
}

impl FifoRegistry {
    /// Create a new FIFO registry
    const fn new() -> Self {
        const INIT: Option<Arc<FifoEntry>> = None;

        Self {
            entries: [INIT; MAX_FIFOS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert a FIFO into the registry
    pub fn insert(&mut self, id: u64, fifo: Arc<FifoEntry>) -> Result {
        let idx = (id as usize) % MAX_FIFOS;

        if self.entries[idx].is_none() {
            self.entries[idx] = Some(fifo);
            self.count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NO_RESOURCES)
        }
    }

    /// Get a FIFO from the registry
    pub fn get(&self, id: u64) -> Option<Arc<FifoEntry>> {
        let idx = (id as usize) % MAX_FIFOS;
        self.entries[idx].as_ref().filter(|f| f.id == id).cloned()
    }

    /// Remove a FIFO from the registry
    pub fn remove(&mut self, id: u64) -> Result {
        let idx = (id as usize) % MAX_FIFOS;

        if self.entries[idx].is_some() {
            self.entries[idx] = None;
            self.count.fetch_sub(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NOT_FOUND)
        }
    }

    /// Get the number of active FIFOs
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global FIFO registry
static mut FIFO_REGISTRY: FifoRegistry = FifoRegistry::new();

/// ============================================================================
/// FIFO ID Allocation
/// ============================================================================

/// Next FIFO ID counter
static mut NEXT_FIFO_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new FIFO ID
fn alloc_fifo_id() -> u64 {
    unsafe { NEXT_FIFO_ID.fetch_add(1, Ordering::Relaxed) }
}

/// ============================================================================
/// Syscall: FIFO Create
/// ============================================================================

/// Create a FIFO pair syscall handler
///
/// # Arguments
///
/// * `count` - Capacity (number of elements)
/// * `elem_size` - Size of each element
/// * `options` - Creation options (must be 0)
/// * `handle0_out` - User pointer to store first handle
/// * `handle1_out` - User pointer to store second handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_fifo_create_impl(
    count: usize,
    elem_size: usize,
    options: u32,
    handle0_out: usize,
    handle1_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_fifo_create: count={} elem_size={} options={:#x}",
        count, elem_size, options
    );

    // Validate options (must be 0)
    if options != fifo_options::NONE {
        log_error!("sys_fifo_create: invalid options {:#x}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate count
    if count == 0 || count > 65536 {
        log_error!("sys_fifo_create: invalid count {}", count);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate elem_size
    if elem_size == 0 || elem_size > 4096 {
        log_error!("sys_fifo_create: invalid elem_size {}", elem_size);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Allocate new FIFO IDs
    let id0 = alloc_fifo_id();
    let id1 = alloc_fifo_id();

    let fifo0 = Arc::new(FifoEntry::new(id0, elem_size, count));
    let fifo1 = Arc::new(FifoEntry::new(id1, elem_size, count));

    // Set peer IDs
    fifo0.peer_id.store(id1, Ordering::Relaxed);
    fifo1.peer_id.store(id0, Ordering::Relaxed);

    // Insert into FIFO registry
    unsafe {
        if let Err(err) = FIFO_REGISTRY.insert(id0, fifo0.clone()) {
            log_error!("sys_fifo_create: failed to insert fifo0: {:?}", err);
            return err_to_ret(err);
        }

        if let Err(err) = FIFO_REGISTRY.insert(id1, fifo1.clone()) {
            log_error!("sys_fifo_create: failed to insert fifo1: {:?}", err);
            FIFO_REGISTRY.remove(id0);
            return err_to_ret(err);
        }
    }

    // Write handles to user space
    if handle0_out != 0 {
        let user_ptr = UserPtr::<u8>::new(handle0_out);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, &id0 as *const u64 as *const u8, 8) {
                log_error!("sys_fifo_create: copy_to_user failed for handle0: {:?}", err);
                FIFO_REGISTRY.remove(id0);
                FIFO_REGISTRY.remove(id1);
                return err_to_ret(err.into());
            }
        }
    }

    if handle1_out != 0 {
        let user_ptr = UserPtr::<u8>::new(handle1_out);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, &id1 as *const u64 as *const u8, 8) {
                log_error!("sys_fifo_create: copy_to_user failed for handle1: {:?}", err);
                FIFO_REGISTRY.remove(id0);
                FIFO_REGISTRY.remove(id1);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_fifo_create: success fifo0={} fifo1={}", id0, id1);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: FIFO Write
/// ============================================================================

/// Write to a FIFO syscall handler
///
/// # Arguments
///
/// * `handle_val` - FIFO handle value
/// * `elem_size` - Size of each element (must match creation size)
/// * `data` - User pointer to data
/// * `count` - Number of elements to write
/// * `actual_count_out` - User pointer to store actual count written
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_fifo_write_impl(
    handle_val: u32,
    elem_size: usize,
    data: usize,
    count: usize,
    actual_count_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_fifo_write: handle={:#x} elem_size={} count={}",
        handle_val, elem_size, count
    );

    // Look up FIFO
    let fifo_id = handle_val as u64;
    let fifo = unsafe {
        match FIFO_REGISTRY.get(fifo_id) {
            Some(f) => f,
            None => {
                log_error!("sys_fifo_write: FIFO not found");
                return err_to_ret(RX_ERR_BAD_HANDLE);
            }
        }
    };

    // Validate element size
    if fifo.elem_size != elem_size {
        log_error!(
            "sys_fifo_write: elem_size mismatch (expected {}, got {})",
            fifo.elem_size, elem_size
        );
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Calculate total bytes to write
    let total_bytes = count * elem_size;

    // Allocate buffer for data
    let mut buf = alloc::vec![0u8; total_bytes];

    // Copy data from user
    let user_ptr = UserPtr::<u8>::new(data);
    unsafe {
        if let Err(err) = copy_from_user(buf.as_mut_ptr(), user_ptr, total_bytes) {
            log_error!("sys_fifo_write: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Write to FIFO
    let actual_written = match fifo.write(&buf) {
        Ok(n) => n / elem_size,
        Err(err) => {
            // Map SHOULD_WAIT to a specific error
            if err == RX_ERR_SHOULD_WAIT {
                return err_to_ret(RX_ERR_SHOULD_WAIT);
            }
            return err_to_ret(err);
        }
    };

    // Write actual count to user
    if actual_count_out != 0 {
        let user_ptr = UserPtr::<u8>::new(actual_count_out);
        unsafe {
            if let Err(err) = copy_to_user(
                user_ptr,
                &actual_written as *const _ as *const u8,
                core::mem::size_of::<usize>(),
            ) {
                log_error!("sys_fifo_write: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_fifo_write: success wrote {}", actual_written);

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: FIFO Read
/// ============================================================================

/// Read from a FIFO syscall handler
///
/// # Arguments
///
/// * `handle_val` - FIFO handle value
/// * `elem_size` - Size of each element (must match creation size)
/// * `data` - User pointer to buffer
/// * `count` - Number of elements to read
/// * `actual_count_out` - User pointer to store actual count read
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_fifo_read_impl(
    handle_val: u32,
    elem_size: usize,
    data: usize,
    count: usize,
    actual_count_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_fifo_read: handle={:#x} elem_size={} count={}",
        handle_val, elem_size, count
    );

    // Look up FIFO
    let fifo_id = handle_val as u64;
    let fifo = unsafe {
        match FIFO_REGISTRY.get(fifo_id) {
            Some(f) => f,
            None => {
                log_error!("sys_fifo_read: FIFO not found");
                return err_to_ret(RX_ERR_BAD_HANDLE);
            }
        }
    };

    // Validate element size
    if fifo.elem_size != elem_size {
        log_error!(
            "sys_fifo_read: elem_size mismatch (expected {}, got {})",
            fifo.elem_size, elem_size
        );
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Calculate total bytes to read
    let total_bytes = count * elem_size;

    // Allocate buffer for data
    let mut buf = alloc::vec![0u8; total_bytes];

    // Read from FIFO
    let actual_read = match fifo.read(&mut buf) {
        Ok(n) => n / elem_size,
        Err(err) => {
            // Map SHOULD_WAIT to a specific error
            if err == RX_ERR_SHOULD_WAIT {
                return err_to_ret(RX_ERR_SHOULD_WAIT);
            }
            return err_to_ret(err);
        }
    };

    // Copy data to user (only what was actually read)
    if actual_read > 0 {
        let bytes_to_copy = actual_read * elem_size;
        let user_ptr = UserPtr::<u8>::new(data);
        unsafe {
            if let Err(err) = copy_to_user(user_ptr, buf.as_ptr(), bytes_to_copy) {
                log_error!("sys_fifo_read: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    // Write actual count to user
    if actual_count_out != 0 {
        let user_ptr = UserPtr::<u8>::new(actual_count_out);
        unsafe {
            if let Err(err) = copy_to_user(
                user_ptr,
                &actual_read as *const _ as *const u8,
                core::mem::size_of::<usize>(),
            ) {
                log_error!("sys_fifo_read: copy_to_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }
    }

    log_debug!("sys_fifo_read: success read {}", actual_read);

    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get FIFO subsystem statistics
pub fn get_stats() -> FifoStats {
    FifoStats {
        total_fifos: unsafe { FIFO_REGISTRY.count() },
        total_bytes_queued: 0, // TODO: Track total bytes
    }
}

/// FIFO subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FifoStats {
    /// Total number of FIFOs
    pub total_fifos: usize,

    /// Total bytes queued
    pub total_bytes_queued: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the FIFO syscall subsystem
pub fn init() {
    log_info!("FIFO syscall subsystem initialized");
    log_info!("  Max FIFOs: {}", MAX_FIFOS);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fifo_create() {
        let result = sys_fifo_create_impl(16, 8, 0, 0, 0);
        assert!(result >= 0);
    }

    #[test]
    fn test_fifo_create_invalid_options() {
        let result = sys_fifo_create_impl(16, 8, 0xFF, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_fifo_create_invalid_count() {
        let result = sys_fifo_create_impl(0, 8, 0, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_fifo_create_invalid_elem_size() {
        let result = sys_fifo_create_impl(16, 0, 0, 0, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_fifo_write_invalid_handle() {
        let result = sys_fifo_write_impl(0, 8, 0, 1, 0);
        assert!(result < 0);
    }

    #[test]
    fn test_fifo_read_invalid_handle() {
        let result = sys_fifo_read_impl(0, 8, 0, 1, 0);
        assert!(result < 0);
    }
}
