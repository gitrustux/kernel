// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! VMO System Calls
//!
//! This module implements the VMO (Virtual Memory Object) system calls.
//! VMOs represent contiguous regions of physical memory that can be
//! mapped into address spaces.
//!
//! # Syscalls Implemented
//!
//! - `rx_vmo_create` - Create a new VMO
//! - `rx_vmo_read` - Read from a VMO
//! - `rx_vmo_write` - Write to a VMO
//! - `rx_vmo_clone` - Clone a VMO (COW)
//!
//! # Design
//!
//! - VMOs are reference counted objects
//! - Handles to VMOs have rights (READ, WRITE, EXECUTE, MAP, DUPLICATE)
//! - All operations validate handle rights before proceeding
//! - User pointers are validated before access


use crate::kernel::object::vmo::{self, Vmo, VmoFlags};
use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::sync::Mutex;
use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::vm::layout::*;
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;

// Import logging macros
use crate::{log_debug, log_error, log_info};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// ============================================================================
/// VMO Registry
/// ============================================================================

/// Maximum number of VMOs in the system
const MAX_VMOS: usize = 65536;

/// VMO registry entry
struct VmoEntry {
    /// VMO ID
    id: vmo::VmoId,

    /// VMO object
    vmo: Arc<Vmo>,
}

/// Global VMO registry
///
/// Maps VMO IDs to VMO objects. This is used to resolve handles to VMOs.
struct VmoRegistry {
    /// VMO entries
    entries: [Option<VmoEntry>; MAX_VMOS],

    /// Next VMO index to allocate
    next_index: AtomicUsize,

    /// Number of active VMOs
    count: AtomicUsize,
}

impl VmoRegistry {
    /// Create a new VMO registry
    const fn new() -> Self {
        const INIT: Option<VmoEntry> = None;

        Self {
            entries: [INIT; MAX_VMOS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert a VMO into the registry
    pub fn insert(&mut self, vmo: Arc<Vmo>) -> Result<vmo::VmoId> {
        let id = vmo.id;

        // Find a free slot
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (id as usize) % MAX_VMOS;

        loop {
            // Try to allocate at current index
            if self.entries[idx].is_none() {
                self.entries[idx] = Some(VmoEntry { id, vmo });
                self.count.fetch_add(1, Ordering::Relaxed);
                self.next_index.store((idx + 1) % MAX_VMOS, Ordering::Relaxed);
                return Ok(id);
            }

            // Linear probe
            idx = (idx + 1) % MAX_VMOS;

            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    /// Get a VMO from the registry
    pub fn get(&self, id: vmo::VmoId) -> Option<Arc<Vmo>> {
        let idx = (id as usize) % MAX_VMOS;

        self.entries[idx]
            .as_ref()
            .filter(|entry| entry.id == id)
            .map(|entry| entry.vmo.clone())
    }

    /// Remove a VMO from the registry
    pub fn remove(&mut self, id: vmo::VmoId) -> Option<Arc<Vmo>> {
        let idx = (id as usize) % MAX_VMOS;

        if let Some(entry) = self.entries[idx].take() {
            if entry.id == id {
                self.count.fetch_sub(1, Ordering::Relaxed);
                return Some(entry.vmo);
            }
        }

        None
    }

    /// Get the number of active VMOs
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global VMO registry
static VMO_REGISTRY: Mutex<VmoRegistry> = Mutex::new(VmoRegistry::new());

/// ============================================================================
/// Handle to VMO Resolution
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

/// Look up a VMO from a handle value
///
/// This function:
/// 1. Gets the current process's handle table
/// 2. Looks up the handle in the table
/// 3. Validates the handle type and rights
/// 4. Returns the VMO object
fn lookup_vmo_from_handle(
    handle_val: u32,
    required_rights: Rights,
) -> Result<(Arc<Vmo>, Handle)> {
    // Get current process handle table
    let handle_table = current_process_handle_table()
        .ok_or(RX_ERR_NOT_SUPPORTED)?;

    // Get the handle from the table
    let handle = handle_table.get(handle_val)
        .ok_or(RX_ERR_INVALID_ARGS)?;

    // Validate object type
    if handle.obj_type() != ObjectType::Vmo {
        return Err(RX_ERR_WRONG_TYPE);
    }

    // Validate rights
    handle.require(required_rights)?;

    // Get VMO ID from handle (stored as part of base pointer for now)
    // In a real implementation, the handle would store the VMO ID directly
    let vmo_id = handle.id as vmo::VmoId;

    // Get VMO from registry
    let vmo = VMO_REGISTRY.lock().get(vmo_id)
        .ok_or(RX_ERR_NOT_FOUND)?;

    Ok((vmo, handle))
}

/// ============================================================================
/// VMO Kernel Object Base
/// ============================================================================

/// Create a kernel object base for a VMO
fn vmo_to_kernel_base(vmo: &Arc<Vmo>) -> KernelObjectBase {
    KernelObjectBase::new(ObjectType::Vmo)
}

/// ============================================================================
/// Syscall: VMO Create
/// ============================================================================

/// Create a new VMO syscall handler
///
/// # Arguments
///
/// * `args` - Syscall arguments
///   - args[0]: Size in bytes (must be page-aligned)
///   - args[1]: Options (0 for resizable, 1 for non-resizable)
///
/// # Returns
///
/// * On success: Handle value for the new VMO
/// * On error: Negative error code
pub fn sys_vmo_create_impl(size: usize, options: u32) -> SyscallRet {
    log_debug!("sys_vmo_create: size={:#x} options={}", size, options);

    // Validate size (must be page-aligned and non-zero)
    if size == 0 || (size & 0xFFF) != 0 {
        log_error!("sys_vmo_create: invalid size {:#x}", size);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Convert options to VMO flags
    let flags = match options {
        0 => VmoFlags::RESIZABLE,
        1 => VmoFlags::empty,
        _ => {
            log_error!("sys_vmo_create: invalid options {}", options);
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    // Create the VMO
    let vmo = match Vmo::create(size, flags) {
        Ok(vmo) => vmo,
        Err(err) => {
            log_error!("sys_vmo_create: failed to create VMO: {:?}", err);
            return err_to_ret(err);
        }
    };

    log_debug!("sys_vmo_create: created VMO id={}", vmo.id);

    // Wrap in Arc for registry
    let vmo_arc = Arc::new(vmo);

    // Insert into VMO registry
    let vmo_id = match VMO_REGISTRY.lock().insert(vmo_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_vmo_create: failed to insert VMO into registry: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Create kernel object base
    let base = vmo_to_kernel_base(&vmo_arc);

    // Create handle with default rights
    let rights = Rights::DEFAULT;
    let handle = Handle::new(&base as *const KernelObjectBase, rights);

    // TODO: Add handle to current process's handle table
    // For now, return the VMO ID as the handle value
    let handle_value = vmo_id as u32;

    log_debug!("sys_vmo_create: success handle={}", handle_value);

    ok_to_ret(handle_value as usize)
}

/// ============================================================================
/// Syscall: VMO Read
/// ============================================================================

/// Read from VMO syscall handler
///
/// # Arguments
///
/// * `args` - Syscall arguments
///   - args[0]: Handle value
///   - args[1]: User buffer pointer
///   - args[2]: Offset in VMO
///   - args[3]: Length to read
///
/// # Returns
///
/// * On success: Number of bytes read
/// * On error: Negative error code
pub fn sys_vmo_read_impl(
    handle_val: u32,
    user_ptr: usize,
    offset: usize,
    len: usize,
) -> SyscallRet {
    log_debug!(
        "sys_vmo_read: handle={} ptr={:#x} offset={:#x} len={:#x}",
        handle_val, user_ptr, offset, len
    );

    // Validate length
    if len == 0 {
        return ok_to_ret(0);
    }

    // Maximum reasonable read size (16 MB)
    if len > 0x100_0000 {
        log_error!("sys_vmo_read: len too large: {:#x}", len);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate user pointer
    let user_buf = UserPtr::new(user_ptr);
    if !user_buf.is_valid() {
        log_error!("sys_vmo_read: invalid user pointer {:#x}", user_ptr);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up VMO from handle (requires READ right)
    let (vmo, _handle) = match lookup_vmo_from_handle(handle_val, Rights::READ) {
        Ok(vmo) => vmo,
        Err(err) => {
            log_error!("sys_vmo_read: failed to lookup VMO: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Allocate kernel buffer for the read
    let mut kernel_buf = alloc::vec![0u8; len];

    // Read from VMO
    let bytes_read = match vmo.read(offset, &mut kernel_buf) {
        Ok(n) => n,
        Err(err) => {
            log_error!("sys_vmo_read: VMO read failed: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Copy to user space
    unsafe {
        if let Err(err) = copy_to_user(user_buf, kernel_buf.as_ptr(), bytes_read) {
            log_error!("sys_vmo_read: copy_to_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_vmo_read: success bytes_read={}", bytes_read);

    ok_to_ret(bytes_read)
}

/// ============================================================================
/// Syscall: VMO Write
/// ============================================================================

/// Write to VMO syscall handler
///
/// # Arguments
///
/// * `args` - Syscall arguments
///   - args[0]: Handle value
///   - args[1]: User buffer pointer
///   - args[2]: Offset in VMO
///   - args[3]: Length to write
///
/// # Returns
///
/// * On success: Number of bytes written
/// * On error: Negative error code
pub fn sys_vmo_write_impl(
    handle_val: u32,
    user_ptr: usize,
    offset: usize,
    len: usize,
) -> SyscallRet {
    log_debug!(
        "sys_vmo_write: handle={} ptr={:#x} offset={:#x} len={:#x}",
        handle_val, user_ptr, offset, len
    );

    // Validate length
    if len == 0 {
        return ok_to_ret(0);
    }

    // Maximum reasonable write size (16 MB)
    if len > 0x100_0000 {
        log_error!("sys_vmo_write: len too large: {:#x}", len);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Validate user pointer
    let user_buf = UserPtr::new(user_ptr);
    if !user_buf.is_valid() {
        log_error!("sys_vmo_write: invalid user pointer {:#x}", user_ptr);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up VMO from handle (requires WRITE right)
    let (vmo, _handle) = match lookup_vmo_from_handle(handle_val, Rights::WRITE) {
        Ok(vmo) => vmo,
        Err(err) => {
            log_error!("sys_vmo_write: failed to lookup VMO: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Allocate kernel buffer for the write
    let mut kernel_buf = alloc::vec![0u8; len];

    // Copy from user space
    unsafe {
        if let Err(err) = copy_from_user(kernel_buf.as_mut_ptr(), user_buf, len) {
            log_error!("sys_vmo_write: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Write to VMO
    let bytes_written = match vmo.write(offset, &kernel_buf) {
        Ok(n) => n,
        Err(err) => {
            log_error!("sys_vmo_write: VMO write failed: {:?}", err);
            return err_to_ret(err);
        }
    };

    log_debug!("sys_vmo_write: success bytes_written={}", bytes_written);

    ok_to_ret(bytes_written)
}

/// ============================================================================
/// Syscall: VMO Clone
/// ============================================================================

/// Clone VMO syscall handler
///
/// # Arguments
///
/// * `args` - Syscall arguments
///   - args[0]: Handle value to clone
///   - args[1]: Offset in parent VMO
///   - args[2]: Size of clone
///
/// # Returns
///
/// * On success: Handle value for the cloned VMO
/// * On error: Negative error code
pub fn sys_vmo_clone_impl(
    handle_val: u32,
    offset: usize,
    size: usize,
) -> SyscallRet {
    log_debug!(
        "sys_vmo_clone: handle={} offset={:#x} size={:#x}",
        handle_val, offset, size
    );

    // Validate size (must be page-aligned and non-zero)
    if size == 0 || (size & 0xFFF) != 0 {
        log_error!("sys_vmo_clone: invalid size {:#x}", size);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Look up VMO from handle (requires READ and DUPLICATE rights)
    let (parent_vmo, _handle) = match lookup_vmo_from_handle(
        handle_val,
        Rights::READ | Rights::DUPLICATE,
    ) {
        Ok(vmo) => vmo,
        Err(err) => {
            log_error!("sys_vmo_clone: failed to lookup VMO: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Clone the VMO (COW)
    let vmo = match (*parent_vmo).clone(offset, size) {
        Ok(vmo) => vmo,
        Err(err) => {
            log_error!("sys_vmo_clone: VMO clone failed: {:?}", err);
            return err_to_ret(err);
        }
    };

    log_debug!("sys_vmo_clone: created VMO clone id={}", vmo.id);

    // Wrap in Arc for registry
    let vmo_arc = Arc::new(vmo);

    // Insert into VMO registry
    let vmo_id = match VMO_REGISTRY.lock().insert(vmo_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_vmo_clone: failed to insert VMO into registry: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Create kernel object base
    let base = vmo_to_kernel_base(&vmo_arc);

    // Create handle with default rights (add WRITE for COW clones)
    let mut rights = Rights::DEFAULT;
    rights = rights.add(Rights::WRITE); // COW clones are writable
    let handle = Handle::new(&base as *const KernelObjectBase, rights);

    // TODO: Add handle to current process's handle table
    // For now, return the VMO ID as the handle value
    let handle_value = vmo_id as u32;

    log_debug!("sys_vmo_clone: success handle={}", handle_value);

    ok_to_ret(handle_value as usize)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get VMO subsystem statistics
pub fn get_stats() -> VmoStats {
    VmoStats {
        total_vmos: VMO_REGISTRY.lock().count(),
        total_pages: 0, // TODO: Track total pages
        committed_pages: 0, // TODO: Track committed pages
    }
}

/// VMO subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmoStats {
    /// Total number of VMOs
    pub total_vmos: usize,

    /// Total pages across all VMOs
    pub total_pages: usize,

    /// Number of committed pages
    pub committed_pages: usize,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the VMO syscall subsystem
pub fn init() {
    log_info!("VMO syscall subsystem initialized");
    log_info!("  Max VMOs: {}", MAX_VMOS);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vmo_registry_insert_get() {
        let vmo = Vmo::create(0x1000, VmoFlags::empty()).unwrap();
        let vmo_arc = Arc::new(vmo);

        let id = VMO_REGISTRY.lock().insert(vmo_arc.clone()).unwrap();
        assert_eq!(id, vmo_arc.id);

        let retrieved = VMO_REGISTRY.lock().get(id).unwrap();
        assert_eq!(retrieved.id, vmo_arc.id);
    }

    #[test]
    fn test_vmo_registry_remove() {
        let vmo = Vmo::create(0x1000, VmoFlags::empty()).unwrap();
        let vmo_arc = Arc::new(vmo);

        let id = VMO_REGISTRY.lock().insert(vmo_arc.clone()).unwrap();
        let removed = VMO_REGISTRY.lock().remove(id).unwrap();

        assert_eq!(removed.id, vmo_arc.id);
        assert!(VMO_REGISTRY.lock().get(id).is_none());
    }

    #[test]
    fn test_vmo_flags() {
        let vmo_resizable = Vmo::create(0x1000, VmoFlags::RESIZABLE).unwrap();
        assert!(vmo_resizable.flags.is_resizable());

        let vmo_normal = Vmo::create(0x1000, VmoFlags::empty()).unwrap();
        assert!(!vmo_normal.flags.is_resizable());
    }

    #[test]
    fn test_vmo_size_validation() {
        // Invalid: zero size
        assert!(Vmo::create(0, VmoFlags::empty()).is_err());

        // Invalid: not page-aligned
        assert!(Vmo::create(0x1001, VmoFlags::empty()).is_err());

        // Valid: page-aligned
        assert!(Vmo::create(0x1000, VmoFlags::empty()).is_ok());
    }

    #[test]
    fn test_vmo_read_write() {
        let vmo = Vmo::create(0x1000, VmoFlags::empty()).unwrap();
        let data = b"Hello, World!";

        // Write data
        let written = vmo.write(0, data).unwrap();
        assert_eq!(written, data.len());

        // Read back
        let mut buf = [0u8; 64];
        let read = vmo.read(0, &mut buf).unwrap();
        assert_eq!(read, data.len());
        assert_eq!(&buf[..data.len()], data);
    }

    #[test]
    fn test_vmo_clone() {
        let parent = Vmo::create(0x2000, VmoFlags::empty()).unwrap();
        let parent_arc = Arc::new(parent);

        let data = b"Hello, clone!";
        parent_arc.write(0, data).unwrap();

        // Clone the VMO
        let clone = parent_arc.clone(0, 0x1000).unwrap();

        // Verify clone is a COW clone
        assert!(clone.flags.is_cow());

        // Read from clone should get parent's data
        let mut buf = [0u8; 64];
        let read = clone.read(0, &mut buf).unwrap();
        assert_eq!(read, data.len());
        assert_eq!(&buf[..data.len()], data);
    }

    #[test]
    fn test_vmo_resize() {
        let vmo = Vmo::create(0x1000, VmoFlags::RESIZABLE).unwrap();

        // Resize to larger
        assert!(vmo.resize(0x2000).is_ok());
        assert_eq!(vmo.size(), 0x2000);

        // Resize to smaller
        assert!(vmo.resize(0x1000).is_ok());
        assert_eq!(vmo.size(), 0x1000);
    }

    #[test]
    fn test_vmo_resize_non_resizable() {
        let vmo = Vmo::create(0x1000, VmoFlags::empty()).unwrap();

        // Should fail - not resizable
        assert!(vmo.resize(0x2000).is_err());
    }
}
