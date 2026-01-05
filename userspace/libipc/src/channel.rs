// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Channel IPC
//!
//! Channels provide a bidirectional message passing mechanism between processes.

#![no_std]

use libsys::{Handle, Result, Status, Error, syscall::SyscallNumber};

/// Arguments for reading from a channel
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChannelReadArgs {
    /// Pointer to buffer for message bytes
    pub bytes: *mut u8,
    /// Size of bytes buffer
    pub bytes_size: usize,
    /// Pointer to array for handles
    pub handles: *mut Handle,
    /// Capacity of handles array
    pub handles_capacity: usize,
}

/// Arguments for writing to a channel
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChannelWriteArgs {
    /// Pointer to message bytes
    pub bytes: *const u8,
    /// Size of message
    pub bytes_size: usize,
    /// Pointer to array of handles to transfer
    pub handles: *const Handle,
    /// Number of handles
    pub handles_count: usize,
}

/// Arguments for channel call (write + wait for read)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChannelCallEtcArgs {
    /// Pointer to write args
    pub wr_args: *const ChannelWriteArgs,
    /// Pointer to read args
    pub rd_args: *mut ChannelReadArgs,
    /// Deadline for the call
    pub deadline: u64,
}

/// Channel endpoint
///
/// Channels are bidirectional message pipes that support:
/// - Sending and receiving bytes
/// - Transferring handles along with messages
/// - Waiting for messages with timeouts
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Channel {
    handle: Handle,
}

impl Channel {
    /// Create a new channel pair
    ///
    /// Returns two channel endpoints that can communicate with each other.
    pub fn create() -> Result<(Self, Self)> {
        Handle::duplicate(&Handle::INVALID, libsys::Rights::all()).and_then(|_| {
            let (ch_a, ch_b) = libsys::Channel::create()?;
            Ok((
                Self { handle: *ch_a.handle() },
                Self { handle: *ch_b.handle() },
            ))
        })
    }

    /// Create a channel from a raw handle
    ///
    /// # Safety
    ///
    /// The handle must be a valid channel handle.
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Write data to the channel
    ///
    /// # Arguments
    ///
    /// * `bytes` - Data to send
    /// * `handles` - Handles to transfer along with the data
    pub fn write(&self, bytes: &[u8], handles: &[Handle]) -> Result<()> {
        if !self.handle.rights().contains(libsys::Rights::WRITE) {
            return Err(Error::new(Status::AccessDenied));
        }

        let args = ChannelWriteArgs {
            bytes: bytes.as_ptr(),
            bytes_size: bytes.len(),
            handles: handles.as_ptr(),
            handles_count: handles.len(),
        };

        unsafe {
            let ret = libsys::syscall::syscall4(
                SyscallNumber::ChannelWrite as u64,
                self.handle.raw() as u64,
                &args as *const ChannelWriteArgs as u64,
                0, // options
                0,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Read data from the channel
    ///
    /// # Arguments
    ///
    /// * `bytes` - Buffer to read data into
    /// * `handles` - Vector to store received handles
    ///
    /// # Returns
    ///
    /// Number of bytes read
    pub fn read(&self, bytes: &mut [u8], handles: &mut Vec<Handle>) -> Result<usize> {
        if !self.handle.rights().contains(libsys::Rights::READ) {
            return Err(Error::new(Status::AccessDenied));
        }

        // First call to get the number of bytes needed
        let mut handle_count: u32 = 0;
        let mut byte_count: u64 = 0;

        unsafe {
            let ret = libsys::syscall::syscall6(
                SyscallNumber::ChannelRead as u64,
                self.handle.raw() as u64,
                0, // options
                0, // bytes pointer (null to get size)
                bytes.len() as u64,
                &mut byte_count as *mut u64 as u64,
                0, // handles pointer (null to get count)
                &mut handle_count as *mut u32 as u64,
            );

            if (ret as i32) < 0 && (ret as i32) != Status::BufferTooSmall as i32 {
                return Err(Error::from_raw(ret as i32));
            }
        }

        // Allocate space for handles if needed
        if handle_count > 0 {
            handles.reserve(handle_count as usize);
            handles.set_len(handle_count as usize);
        }

        unsafe {
            let ret = libsys::syscall::syscall6(
                SyscallNumber::ChannelRead as u64,
                self.handle.raw() as u64,
                0, // options
                bytes.as_mut_ptr() as u64,
                bytes.len() as u64,
                &mut byte_count as *mut u64 as u64,
                if handles.is_empty() { 0 } else { handles.as_mut_ptr() as u64 },
                &mut handle_count as *mut u32 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(byte_count as usize)
        }
    }

    /// Perform a channel call (write then wait for read)
    ///
    /// This is a combined operation that writes a message and waits for a response.
    ///
    /// # Arguments
    ///
    /// * `write_bytes` - Data to send
    /// * `write_handles` - Handles to transfer
    /// * `read_bytes` - Buffer for response data
    /// * `read_handles` - Vector for received handles
    /// * `deadline` - Optional deadline (0 = no deadline)
    pub fn call(
        &self,
        write_bytes: &[u8],
        write_handles: &[Handle],
        read_bytes: &mut [u8],
        read_handles: &mut Vec<Handle>,
        deadline: u64,
    ) -> Result<usize> {
        if !self.handle.rights().contains(libsys::Rights::READ) ||
           !self.handle.rights().contains(libsys::Rights::WRITE) {
            return Err(Error::new(Status::AccessDenied));
        }

        let wr_args = ChannelWriteArgs {
            bytes: write_bytes.as_ptr(),
            bytes_size: write_bytes.len(),
            handles: write_handles.as_ptr(),
            handles_count: write_handles.len(),
        };

        // First call to get sizes
        let mut handle_count: u32 = 0;
        let mut byte_count: u64 = 0;

        unsafe {
            let ret = libsys::syscall::syscall6(
                SyscallNumber::ChannelCallEtc as u64,
                self.handle.raw() as u64,
                0, // options
                &wr_args as *const ChannelWriteArgs as u64,
                0, // rd_args pointer (null to get size)
                deadline,
                &mut byte_count as *mut u64 as u64,
                &mut handle_count as *mut u32 as u64,
            );

            if (ret as i32) < 0 && (ret as i32) != Status::BufferTooSmall as i32 {
                return Err(Error::from_raw(ret as i32));
            }
        }

        // Allocate space for handles
        if handle_count > 0 {
            read_handles.reserve(handle_count as usize);
            read_handles.set_len(handle_count as usize);
        }

        let rd_args = ChannelReadArgs {
            bytes: read_bytes.as_mut_ptr(),
            bytes_size: read_bytes.len(),
            handles: if read_handles.is_empty() {
                core::ptr::null_mut()
            } else {
                read_handles.as_mut_ptr()
            },
            handles_capacity: read_handles.len(),
        };

        unsafe {
            let ret = libsys::syscall::syscall6(
                SyscallNumber::ChannelCallEtc as u64,
                self.handle.raw() as u64,
                0, // options
                &wr_args as *const ChannelWriteArgs as u64,
                &rd_args as *mut ChannelReadArgs as u64,
                deadline,
                &mut byte_count as *mut u64 as u64,
                &mut handle_count as *mut u32 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(byte_count as usize)
        }
    }

    /// Query for incoming message size
    ///
    /// Returns the number of bytes available to read and the number of handles.
    pub fn query(&self) -> Result<(usize, usize)> {
        unsafe {
            let mut byte_count: u64 = 0;
            let mut handle_count: u64 = 0;

            let ret = libsys::syscall::syscall4(
                SyscallNumber::ChannelRead as u64,
                self.handle.raw() as u64,
                0, // options
                0, // bytes pointer (null to query)
                0, // bytes size
                &mut byte_count as *mut u64 as u64,
                &mut handle_count as *mut u64 as u64,
            );

            if (ret as i32) < 0 && (ret as i32) != Status::BufferTooSmall as i32 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok((byte_count as usize, handle_count as usize))
        }
    }
}
