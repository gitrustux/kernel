// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Port IPC
//!
//! Ports provide a queue-based packet delivery mechanism for notifications
//! and events. Multiple channels can be bound to a single port for
//! multiplexed event delivery.

#![no_std]

use libsys::{Handle, Result, Error, Status, syscall::SyscallNumber};

/// Packet that can be sent to a port
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Packet {
    /// Key for the packet (used to identify the source)
    pub key: u64,
    /// Packet type
    pub packet_type: PacketType,
    /// Status value
    pub status: i32,
    /// Union for extra data
    pub extra: u64,
}

/// Packet types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PacketType {
    /// User packet
    User = 0,
    /// Signal packet (event signaled)
    Signal = 1,
    /// Signal pair packet
    SignalPair = 2,
    /// Exception
    Exception = 3,
}

impl Packet {
    /// Create a new user packet
    pub fn new_user(key: u64, data: u64) -> Self {
        Self {
            key,
            packet_type: PacketType::User,
            status: 0,
            extra: data,
        }
    }

    /// Create a new signal packet
    pub fn new_signal(key: u64, status: i32) -> Self {
        Self {
            key,
            packet_type: PacketType::Signal,
            status,
            extra: 0,
        }
    }
}

/// Result from waiting on a port
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PacketWaitResult {
    /// Number of packets received
    pub count: usize,
    /// Actual deadline observed
    pub deadline_observed: u64,
}

/// Port object
///
/// Ports provide a queue-based IPC mechanism for delivering packets
/// and notifications from multiple sources.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Port {
    handle: Handle,
}

impl Port {
    /// Create a new port
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of packets the port can hold (0 = default)
    pub fn create(capacity: usize) -> Result<Self> {
        let h = libsys::Port::create()?;
        // TODO: Set capacity if needed
        Ok(Self { handle: *h.handle() })
    }

    /// Create a port from a raw handle
    ///
    /// # Safety
    ///
    /// The handle must be a valid port handle.
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Queue a packet to the port
    ///
    /// # Arguments
    ///
    /// * `packet` - The packet to queue
    /// * `handle` - Optional handle to include with the packet
    pub fn queue(&self, packet: &Packet, handle: Option<&Handle>) -> Result<()> {
        if !self.handle.rights().contains(libsys::Rights::WRITE) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = libsys::syscall::syscall4(
                SyscallNumber::PortQueue as u64,
                self.handle.raw() as u64,
                packet as *const Packet as u64,
                handle.map_or(0, |h| h.raw() as u64),
                0, // options
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Wait for packets on the port
    ///
    /// # Arguments
    ///
    /// * `packets` - Buffer to store received packets
    /// * `deadline` - Optional deadline in nanoseconds (0 = wait forever)
    ///
    /// # Returns
    ///
    /// Number of packets received and the actual deadline observed
    pub fn wait(&self, packets: &mut [Packet], deadline: u64) -> Result<PacketWaitResult> {
        if !self.handle.rights().contains(libsys::Rights::READ) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let mut deadline_observed: u64 = 0;

            let ret = libsys::syscall::syscall5(
                SyscallNumber::PortWait as u64,
                self.handle.raw() as u64,
                packets.as_mut_ptr() as u64,
                packets.len() as u64,
                deadline,
                &mut deadline_observed as *mut u64 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(PacketWaitResult {
                count: ret as usize,
                deadline_observed,
            })
        }
    }

    /// Cancel packets matching a specific key
    ///
    /// # Arguments
    ///
    /// * `key` - The key to match (0 = match all)
    pub fn cancel(&self, key: u64) -> Result<()> {
        unsafe {
            let ret = libsys::syscall::syscall2(
                SyscallNumber::PortCancel as u64,
                self.handle.raw() as u64,
                key,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Bind a channel to this port
    ///
    /// When a message is available on the channel, a packet will be
    /// delivered to this port.
    ///
    /// # Arguments
    ///
    /// * `channel` - The channel to bind
    /// * `key` - The key to use for packets from this channel
    /// * `options` - Options for the binding
    pub fn bind_channel(&self, channel: &Handle, key: u64, options: u32) -> Result<()> {
        // TODO: Implement channel binding
        // This requires a specific syscall or object_set_property
        Ok(())
    }
}
