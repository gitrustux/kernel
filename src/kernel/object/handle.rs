// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Handle & Rights Model
//!
//! This module implements the capability-based handle system.
//! All kernel resources are accessed through handles with rights,
//! ensuring fine-grained access control.
//!
//! # Design
//!
//! - **Handles**: Capability tokens referencing kernel objects
//! - **Rights**: Bitmask specifying permitted operations
//! - **Enforcement**: Every syscall validates rights before operation
//! - **Transfer**: Handles can be sent via IPC with rights reduction
//!
//! # Usage
//!
//! ```rust
//! let handle = Handle::new(object, Rights::READ | Rights::WRITE);
//! handle.rights.require(Rights::READ)?;
//! ```

#![no_std]

use crate::kernel::sync::Mutex;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use crate::kernel::sync::spin::SpinMutex as SpinMutex;

/// ============================================================================
/// Handle Rights
/// ============================================================================

/// Handle rights bitmask
///
/// Rights are permissions that control what operations can be performed
/// on a kernel object through a handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights(pub u32);

impl Rights {
    /// No rights
    pub const NONE: Self = Self(0x00);

    /// Read state
    pub const READ: Self = Self(0x01);

    /// Modify state
    pub const WRITE: Self = Self(0x02);

    /// Execute code
    pub const EXECUTE: Self = Self(0x04);

    /// Signal
    pub const SIGNAL: Self = Self(0x08);

    /// Wait
    pub const WAIT: Self = Self(0x08);

    /// Map into VMAR
    pub const MAP: Self = Self(0x10);

    /// Duplicate handle
    pub const DUPLICATE: Self = Self(0x20);

    /// Transfer to process
    pub const TRANSFER: Self = Self(0x40);

    /// Admin control
    pub const MANAGE: Self = Self(0x80);

    /// Basic rights (READ | WRITE)
    pub const BASIC: Self = Self(0x03);

    /// Default rights (Basic + SIGNAL + MAP + DUPLICATE)
    pub const DEFAULT: Self = Self(0x1F);

    /// Keep same rights on dup
    pub const SAME_RIGHTS: Self = Self(0x8000_0000);

    /// Create a rights mask from raw value
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self.0
    }

    /// Check if this rights mask contains the specified right
    pub fn contains(self, right: Self) -> bool {
        (self.0 & right.0) == right.0
    }

    /// Check if this rights mask contains ANY of the specified rights
    pub fn contains_any(self, rights: Self) -> bool {
        (self.0 & rights.0) != 0
    }

    /// Require specific rights
    ///
    /// # Returns
    ///
    /// - Ok(()) if all rights are present
    /// - Err(RX_ERR_ACCESS_DENIED) if any right is missing
    pub fn require(self, required: Self) -> Result {
        if self.contains(required) {
            Ok(())
        } else {
            Err(RX_ERR_ACCESS_DENIED)
        }
    }

    /// Apply a reduction mask
    ///
    /// Returns the intersection of this rights with the mask.
    /// This is used for rights reduction during handle transfer.
    pub const fn reduce(self, mask: Self) -> Self {
        Self(self.0 & mask.0)
    }

    /// Add rights
    pub const fn add(self, rights: Self) -> Self {
        Self(self.0 | rights.0)
    }

    /// Remove rights
    pub const fn remove(self, rights: Self) -> Self {
        Self(self.0 & !rights.0)
    }

    /// Check if rights are NONE
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    /// Get default rights for a given object type
    pub fn default_for_type(obj_type: ObjectType) -> Self {
        match obj_type {
            ObjectType::Process => Self::MANAGE,
            ObjectType::Thread => Self::MANAGE,
            ObjectType::Vmo => Self::DEFAULT,
            ObjectType::Vmar => Self::MAP | Self::READ | Self::WRITE,
            ObjectType::Channel => Self::READ | Self::WRITE,
            ObjectType::Event => Self::SIGNAL | Self::WAIT,
            ObjectType::EventPair => Self::SIGNAL | Self::WAIT,
            ObjectType::Timer => Self::SIGNAL | Self::WRITE,
            ObjectType::Job => Self::MANAGE,
            ObjectType::Port => Self::READ | Self::WRITE,
            ObjectType::Unknown => Self::NONE,
        }
    }
}

impl core::ops::BitOr for Rights {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for Rights {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

/// ============================================================================
/// Handle ID
/// ============================================================================

/// Handle identifier
///
/// Each handle has a unique ID within a process.
pub type HandleId = u64;

/// Next handle ID counter
static mut NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new handle ID
fn alloc_handle_id() -> HandleId {
    unsafe {
        NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed)
    }
}

/// ============================================================================
/// Kernel Object Types
/// ============================================================================

/// Kernel object type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    /// Unknown type
    Unknown = 0,

    /// Process object
    Process = 1,

    /// Thread object
    Thread = 2,

    /// Virtual Memory Object
    Vmo = 3,

    /// Virtual Memory Address Region
    Vmar = 4,

    /// Channel endpoint
    Channel = 5,

    /// Event object
    Event = 6,

    /// Event pair
    EventPair = 7,

    /// Timer object
    Timer = 8,

    /// Job object
    Job = 9,

    /// Port (waitset)
    Port = 10,
}

impl ObjectType {
    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Process,
            2 => Self::Thread,
            3 => Self::Vmo,
            4 => Self::Vmar,
            5 => Self::Channel,
            6 => Self::Event,
            7 => Self::EventPair,
            8 => Self::Timer,
            9 => Self::Job,
            10 => Self::Port,
            _ => Self::Unknown,
        }
    }

    /// Get raw value
    pub const fn into_raw(self) -> u32 {
        self as u32
    }

    /// Get name as string
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Process => "process",
            Self::Thread => "thread",
            Self::Vmo => "vmo",
            Self::Vmar => "vmar",
            Self::Channel => "channel",
            Self::Event => "event",
            Self::EventPair => "eventpair",
            Self::Timer => "timer",
            Self::Job => "job",
            Self::Port => "port",
        }
    }
}

/// ============================================================================
/// Kernel Object Base
/// ============================================================================

/// Kernel object base
///
/// All kernel objects share this common structure.
pub struct KernelObjectBase {
    /// Object type
    pub obj_type: ObjectType,

    /// Reference count
    pub ref_count: AtomicUsize,

    /// Whether object is being destroyed
    pub destroying: AtomicBool,
}

impl KernelObjectBase {
    /// Create a new kernel object base
    pub const fn new(obj_type: ObjectType) -> Self {
        Self {
            obj_type,
            ref_count: AtomicUsize::new(1),
            destroying: AtomicBool::new(false),
        }
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

    /// Get reference count
    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Check if object is being destroyed
    pub fn is_destroying(&self) -> bool {
        self.destroying.load(Ordering::Acquire)
    }

    /// Mark object as destroying
    pub fn mark_destroying(&self) {
        self.destroying.store(true, Ordering::Release);
    }
}

/// ============================================================================
/// Handle
/// ============================================================================

/// Handle to a kernel object
///
/// A handle is a capability token that references a kernel object
/// and specifies what operations are permitted on it.
#[repr(C)]
pub struct Handle {
    /// Handle ID
    pub id: HandleId,

    /// Pointer to kernel object base (opaque)
    pub base: *const KernelObjectBase,

    /// Rights mask
    pub rights: Rights,
}

unsafe impl Send for Handle {}

impl Clone for Handle {
    fn clone(&self) -> Self {
        Self {
            id: alloc_handle_id(),
            base: self.base,
            rights: self.rights,
        }
    }
}

impl Handle {
    /// Create a new handle
    ///
    /// # Arguments
    ///
    /// * `base` - Pointer to kernel object
    /// * `rights` - Rights mask
    pub fn new(base: *const KernelObjectBase, rights: Rights) -> Self {
        Self {
            id: alloc_handle_id(),
            base,
            rights,
        }
    }

    /// Create a handle with a specific ID
    ///
    /// Used when duplicating handles.
    pub const fn with_id(id: HandleId, base: *const KernelObjectBase, rights: Rights) -> Self {
        Self { id, base, rights }
    }

    /// Get handle ID
    pub const fn id(&self) -> HandleId {
        self.id
    }

    /// Get object type
    pub fn obj_type(&self) -> ObjectType {
        if self.base.is_null() {
            ObjectType::Unknown
        } else {
            unsafe { (*self.base).obj_type }
        }
    }

    /// Check if handle is valid
    pub fn is_valid(&self) -> bool {
        !self.base.is_null() && !self.rights.is_none()
    }

    /// Require specific rights
    pub fn require(&self, required: Rights) -> Result {
        if !self.is_valid() {
            return Err(RX_ERR_INVALID_ARGS);
        }
        self.rights.require(required)
    }

    /// Check if handle has specific rights
    pub fn has_right(&self, right: Rights) -> bool {
        self.is_valid() && self.rights.contains(right)
    }

    /// Get the rights for this handle
    pub fn rights(&self) -> Rights {
        self.rights
    }

    /// Get the object type for this handle
    pub fn object_type(&self) -> ObjectType {
        if !self.is_valid() {
            return ObjectType::Unknown;
        }
        unsafe {
            if !self.base.is_null() {
                (*self.base).type_id()
            } else {
                ObjectType::Unknown
            }
        }
    }

    /// Duplicate handle with same rights
    pub fn duplicate(&self) -> Result<Self> {
        if !self.is_valid() {
            return Err(RX_ERR_INVALID_ARGS);
        }

        self.require(Rights::DUPLICATE)?;

        // Increment reference count
        if !self.base.is_null() {
            unsafe {
                (*self.base).ref_inc();
            }
        }

        Ok(Self::with_id(alloc_handle_id(), self.base, self.rights))
    }

    /// Duplicate handle with reduced rights
    ///
    /// # Arguments
    ///
    /// * `mask` - Rights mask to apply
    pub fn duplicate_with_mask(&self, mask: Rights) -> Result<Self> {
        if !self.is_valid() {
            return Err(RX_ERR_INVALID_ARGS);
        }

        self.require(Rights::DUPLICATE)?;

        let new_rights = if mask.contains(Rights::SAME_RIGHTS) {
            self.rights
        } else {
            self.rights.reduce(mask)
        };

        // Increment reference count
        if !self.base.is_null() {
            unsafe {
                (*self.base).ref_inc();
            }
        }

        Ok(Self::with_id(alloc_handle_id(), self.base, new_rights))
    }

    /// Close the handle
    ///
    /// Decrements the object's reference count.
    /// Returns true if this was the last reference.
    pub fn close(&self) -> bool {
        if self.base.is_null() {
            return false;
        }

        unsafe {
            (*self.base).ref_dec()
        }
    }
}

/// ============================================================================
/// Handle Owner
/// ============================================================================

/// Owned handle that auto-closes on drop
///
/// This is a RAII wrapper that automatically closes the handle
/// when it goes out of scope.
#[repr(C)]
pub struct HandleOwner {
    /// The owned handle
    handle: Handle,
}

impl HandleOwner {
    /// Create a new owned handle
    pub fn new(base: *const KernelObjectBase, rights: Rights) -> Self {
        Self {
            handle: Handle::new(base, rights),
        }
    }

    /// Get the underlying handle
    pub fn get(&self) -> &Handle {
        &self.handle
    }

    /// Get the underlying handle ID
    pub fn id(&self) -> HandleId {
        self.handle.id()
    }

    /// Take the handle out (consuming the owner)
    pub fn take(self) -> Handle {
        let h = self.handle;
        // Prevent Drop from closing the handle
        core::mem::forget(self);
        h
    }
}

impl Drop for HandleOwner {
    fn drop(&mut self) {
        // Auto-close the handle
        self.handle.close();
    }
}

/// ============================================================================
/// Handle Table
/// ============================================================================

/// Maximum handles per process
pub const MAX_HANDLES: usize = 256;

/// Handle table
///
/// Manages handles for a process.
pub struct HandleTable {
    /// Array of handle slots
    slots: [Mutex<Option<Handle>>; MAX_HANDLES],

    /// Number of active handles
    count: SpinMutex<usize>,
}

impl HandleTable {
    /// Create a new handle table
    pub const fn new() -> Self {
        // Create array of None handles
        const INIT: Mutex<Option<Handle>> = Mutex::new(None);

        Self {
            slots: [INIT; MAX_HANDLES],
            count: SpinMutex::new(0),
        }
    }

    /// Add a handle to the table
    ///
    /// # Returns
    ///
    /// Handle value for userspace
    pub fn add(&self, handle: Handle) -> Result<u32> {
        // Find free slot
        for (i, slot) in self.slots.iter().enumerate() {
            let mut slot_guard = slot.lock();
            if slot_guard.is_none() {
                *slot_guard = Some(handle);
                *self.count.lock() += 1;
                return Ok(i as u32);
            }
        }

        Err(RX_ERR_NO_RESOURCES)
    }

    /// Get a handle from the table
    pub fn get(&self, handle_val: u32) -> Option<Handle> {
        if handle_val as usize >= MAX_HANDLES {
            return None;
        }

        let slot = &self.slots[handle_val as usize];
        let slot_guard = slot.lock();

        slot_guard.as_ref().map(|h| Handle {
            id: h.id,
            base: h.base,
            rights: h.rights,
        })
    }

    /// Remove a handle from the table
    ///
    /// # Returns
    ///
    /// true if the handle was closed, false if not found
    pub fn remove(&self, handle_val: u32) -> Result<bool> {
        if handle_val as usize >= MAX_HANDLES {
            return Err(RX_ERR_INVALID_ARGS);
        }

        let slot = &self.slots[handle_val as usize];
        let mut slot_guard = slot.lock();

        match slot_guard.take() {
            Some(handle) => {
                *self.count.lock() -= 1;
                Ok(handle.close())
            }
            None => Ok(false),
        }
    }

    /// Duplicate a handle in the table
    pub fn duplicate(&self, handle_val: u32, mask: Rights) -> Result<u32> {
        let handle = {
            let slot = &self.slots[handle_val as usize];
            let slot_guard = slot.lock();

            let h = slot_guard.as_ref().ok_or(RX_ERR_INVALID_ARGS)?;

            if mask.contains(Rights::SAME_RIGHTS) {
                h.duplicate()?
            } else {
                h.duplicate_with_mask(mask)?
            }
        };

        self.add(handle)
    }

    /// Get handle count
    pub fn count(&self) -> usize {
        *self.count.lock()
    }

    /// Check if handle table is full
    pub fn is_full(&self) -> bool {
        self.count() >= MAX_HANDLES
    }
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rights_basic() {
        let r = Rights::READ | Rights::WRITE;
        assert!(r.contains(Rights::READ));
        assert!(r.contains(Rights::WRITE));
        assert!(!r.contains(Rights::EXECUTE));
    }

    #[test]
    fn test_rights_require() {
        let r = Rights::READ | Rights::WRITE;
        assert_eq!(r.require(Rights::READ), Ok(()));
        assert_eq!(r.require(Rights::EXECUTE), Err(RX_ERR_ACCESS_DENIED));
    }

    #[test]
    fn test_rights_reduce() {
        let r = Rights::READ | Rights::WRITE | Rights::EXECUTE;
        let mask = Rights::READ | Rights::WRITE;
        let reduced = r.reduce(mask);

        assert!(reduced.contains(Rights::READ));
        assert!(reduced.contains(Rights::WRITE));
        assert!(!reduced.contains(Rights::EXECUTE));
    }

    #[test]
    fn test_object_type() {
        assert_eq!(ObjectType::from_raw(1), ObjectType::Process);
        assert_eq!(ObjectType::Process.into_raw(), 1);
        assert_eq!(ObjectType::Process.name(), "process");
    }

    #[test]
    fn test_handle_table() {
        let table = HandleTable::new();

        // Create a dummy object
        let obj_base = KernelObjectBase::new(ObjectType::Event);
        let handle = Handle::new(&obj_base as *const _, Rights::DEFAULT);

        // Add to table
        let hv = table.add(handle).unwrap();
        assert_eq!(table.count(), 1);

        // Get from table
        let retrieved = table.get(hv);
        assert!(retrieved.is_some());

        // Remove from table
        let closed = table.remove(hv).unwrap();
        assert_eq!(table.count(), 0);
    }
}
