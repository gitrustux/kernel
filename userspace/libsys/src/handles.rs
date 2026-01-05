// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Handle API
//!
//! This module provides Handle types and operations for kernel objects.
//! Handles are used to reference kernel objects like processes, threads,
//! VMOs, channels, etc.

#![no_std]

use bitflags::bitflags;
use crate::error::{Error, Result, Status};
use crate::syscall::{SyscallNumber, syscall1, syscall2, syscall3, syscall4};

bitflags! {
    /// Rights that can be held on a handle
    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Rights: u64 {
        /// Transfer handle (duplicate)
        const TRANSFER = 1 << 0;
        /// Duplicate handle
        const DUPLICATE = 1 << 1;
        /// Basic operation
        const READ = 1 << 2;
        /// Write operation
        const WRITE = 1 << 3;
        /// Execute operation
        const EXECUTE = 1 << 4;
        /// Map memory
        const MAP = 1 << 5;
        /// Get property
        const GET_PROPERTY = 1 << 6;
        /// Set property
        const SET_PROPERTY = 1 << 7;
        /// Enumerate
        const ENUMERATE = 1 << 8;
        /// Destroy
        const DESTROY = 1 << 9;
        /// Set policy
        const SET_POLICY = 1 << 10;
        /// Get policy
        const GET_POLICY = 1 << 11;
        /// Signal
        const SIGNAL = 1 << 12;
        /// Signal peer
        const SIGNAL_PEER = 1 << 13;
        /// Wait
        const WAIT = 1 << 14;
        /// All rights
        const ALL = u64::MAX;
        /// Same rights
        const SAME_RIGHTS = 1 << 63;

        // VMO-specific rights
        /// Property rights
        const PROP_VMO_CONTENT_SIZE = 1 << 20;
        const PROP_VMO_CHILD = 1 << 21;
    }
}

/// Handle to a kernel object
///
/// Handles are used to reference kernel objects. They are reference-counted
/// and automatically closed when dropped.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    /// Raw handle value
    raw: u32,
    /// Rights associated with this handle
    rights: Rights,
}

impl Handle {
    /// Invalid handle value (used for uninitialized handles)
    pub const INVALID: Self = Self {
        raw: !0u32,
        rights: Rights::empty(),
    };

    /// Create a new handle from a raw value and rights
    ///
    /// # Safety
    ///
    /// The raw handle value must be valid and the rights must accurately
    /// reflect the rights held on this handle.
    pub unsafe fn from_raw(raw: u32, rights: Rights) -> Self {
        Self { raw, rights }
    }

    /// Get the raw handle value
    pub fn raw(&self) -> u32 {
        self.raw
    }

    /// Get the rights associated with this handle
    pub fn rights(&self) -> Rights {
        self.rights
    }

    /// Check if this handle is valid
    pub fn is_valid(&self) -> bool {
        self.raw != !0u32
    }

    /// Duplicate this handle
    pub fn duplicate(&self, rights: Rights) -> Result<Self> {
        if !self.rights.contains(Rights::DUPLICATE) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = syscall3(
                SyscallNumber::HandleDuplicate as u64,
                self.raw as u64,
                if rights.contains(Rights::SAME_RIGHTS) {
                    self.rights.bits()
                } else {
                    rights.bits()
                },
                0, // No new handle value returned, we get it from syscall result
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Self {
                raw: ret as u32,
                rights,
            })
        }
    }

    /// Replace this handle with a new one
    pub fn replace(&mut self, new_handle: Self) -> Result<Self> {
        if !self.rights.contains(Rights::DUPLICATE) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = syscall2(
                SyscallNumber::HandleReplace as u64,
                new_handle.raw as u64,
                self.raw as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            let old = *self;
            self.raw = ret as u32;
            self.rights = new_handle.rights;
            Ok(old)
        }
    }

    /// Close this handle
    pub fn close(self) -> Result<()> {
        if !self.is_valid() {
            return Ok(());
        }

        unsafe {
            let ret = syscall1(SyscallNumber::HandleClose as u64, self.raw as u64);
            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }
        }
        Ok(())
    }

    /// Get a bootstrap handle
    ///
    /// This is used during process startup to get handles passed from
    /// the kernel or parent process.
    pub fn bootstrap() -> Self {
        // TODO: Implement proper bootstrap handle retrieval
        // For now, return an invalid handle
        Self::INVALID
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if self.is_valid() {
            // Note: We can't handle errors in drop
            unsafe {
                syscall1(SyscallNumber::HandleClose as u64, self.raw as u64);
            }
        }
    }
}

/// Wrapper for a Process handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Process {
    handle: Handle,
}

impl Process {
    /// Create a new process handle from a raw handle
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Exit the current process
    pub fn exit(code: i32) -> ! {
        unsafe {
            syscall1(SyscallNumber::ProcessExit as u64, code as u64);
            core::hint::unreachable_unchecked();
        }
    }
}

/// Wrapper for a Thread handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Thread {
    handle: Handle,
}

impl Thread {
    /// Create a new thread handle from a raw handle
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Get a handle to the current thread
    pub fn self_handle() -> Result<Self> {
        unsafe {
            let ret = syscall0(SyscallNumber::ThreadSelf as u64);

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Self {
                handle: Handle::from_raw(ret as u32, Rights::all()),
            })
        }
    }

    /// Exit the current thread
    pub fn exit() -> ! {
        unsafe {
            syscall0(SyscallNumber::ThreadExit as u64);
            core::hint::unreachable_unchecked();
        }
    }
}

/// Wrapper for a VMO (Virtual Memory Object) handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vmo {
    handle: Handle,
}

impl Vmo {
    /// Create a new VMO handle from a raw handle
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Create a new VMO
    pub fn create(size: u64, name: Option<&str>) -> Result<Self> {
        // TODO: Implement proper VMO creation
        unsafe {
            let ret = syscall2(
                SyscallNumber::VmoCreate as u64,
                size,
                0, // options
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Self {
                handle: Handle::from_raw(ret as u32, Rights::all()),
            })
        }
    }

    /// Get the size of the VMO
    pub fn get_size(&self) -> Result<u64> {
        if !self.handle.rights.contains(Rights::GET_PROPERTY) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = syscall1(
                SyscallNumber::VmoGetSize as u64,
                self.handle.raw() as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(ret)
        }
    }

    /// Set the size of the VMO
    pub fn set_size(&self, size: u64) -> Result<()> {
        if !self.handle.rights.contains(Rights::SET_PROPERTY) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = syscall2(
                SyscallNumber::VmoSetSize as u64,
                self.handle.raw() as u64,
                size,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Read from the VMO
    pub fn read(&self, offset: u64, data: &mut [u8]) -> Result<usize> {
        if !self.handle.rights.contains(Rights::READ) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = syscall4(
                SyscallNumber::VmoRead as u64,
                self.handle.raw() as u64,
                data.as_ptr() as u64,
                data.len() as u64,
                offset,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(ret as usize)
        }
    }

    /// Write to the VMO
    pub fn write(&self, offset: u64, data: &[u8]) -> Result<usize> {
        if !self.handle.rights.contains(Rights::WRITE) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = syscall4(
                SyscallNumber::VmoWrite as u64,
                self.handle.raw() as u64,
                data.as_ptr() as u64,
                data.len() as u64,
                offset,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(ret as usize)
        }
    }
}

/// Wrapper for a Channel handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Channel {
    handle: Handle,
}

impl Channel {
    /// Create a new channel handle from a raw handle
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Create a new channel
    pub fn create() -> Result<(Self, Self)> {
        unsafe {
            let mut out0: u64 = 0;
            let mut out1: u64 = 0;

            let ret = syscall3(
                SyscallNumber::ChannelCreate as u64,
                0, // options
                &mut out0 as *mut u64 as u64,
                &mut out1 as *mut u64 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok((
                Self {
                    handle: Handle::from_raw(out0 as u32, Rights::all()),
                },
                Self {
                    handle: Handle::from_raw(out1 as u32, Rights::all()),
                },
            ))
        }
    }
}

/// Wrapper for an Event handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Event {
    handle: Handle,
}

impl Event {
    /// Create a new event handle from a raw handle
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Create a new event
    pub fn create() -> Result<Self> {
        unsafe {
            let mut out: u64 = 0;

            let ret = syscall2(
                SyscallNumber::EventCreate as u64,
                0, // options
                &mut out as *mut u64 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Self {
                handle: Handle::from_raw(out as u32, Rights::all()),
            })
        }
    }
}

/// Wrapper for a Port handle
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Port {
    handle: Handle,
}

impl Port {
    /// Create a new port handle from a raw handle
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Create a new port
    pub fn create() -> Result<Self> {
        unsafe {
            let mut out: u64 = 0;

            let ret = syscall1(
                SyscallNumber::PortCreate as u64,
                0, // options
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Self {
                handle: Handle::from_raw(ret as u32, Rights::all()),
            })
        }
    }
}
