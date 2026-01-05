// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Event and EventPair signaling
//!
//! Events are simple synchronization primitives that can be signaled
//! and waited upon. EventPairs provide bidirectional signaling.

#![no_std]

use libsys::{Handle, Result, Error, Status, syscall::SyscallNumber};

/// Event object
///
/// Events are simple synchronization primitives that can be:
/// - Signaled (set)
/// - Unsignaled (reset)
/// - Waited upon
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Event {
    handle: Handle,
}

impl Event {
    /// Create a new event
    ///
    /// # Arguments
    ///
    /// * `initial` - Whether the event starts signaled
    pub fn create(initial: bool) -> Result<Self> {
        let h = libsys::Event::create()?;
        if initial {
            h.signal()?;
        }
        Ok(Self { handle: *h.handle() })
    }

    /// Create an event from a raw handle
    ///
    /// # Safety
    ///
    /// The handle must be a valid event handle.
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Signal the event
    pub fn signal(&self) -> Result<()> {
        if !self.handle.rights().contains(libsys::Rights::SIGNAL) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = libsys::syscall::syscall1(
                SyscallNumber::ObjectSignal as u64,
                self.handle.raw() as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Reset the event
    pub fn reset(&self) -> Result<()> {
        if !self.handle.rights().contains(libsys::Rights::SIGNAL) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = libsys::syscall::syscall2(
                SyscallNumber::ObjectSignal as u64,
                self.handle.raw() as u64,
                0, // options to reset
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Wait for the event to be signaled
    ///
    /// # Arguments
    ///
    /// * `deadline` - Optional deadline in nanoseconds (0 = wait forever)
    pub fn wait(&self, deadline: u64) -> Result<()> {
        if !self.handle.rights().contains(libsys::Rights::WAIT) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = libsys::syscall::syscall3(
                SyscallNumber::ObjectWaitOne as u64,
                self.handle.raw() as u64,
                deadline,
                0, // signals
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }
}

/// EventPair object
///
/// EventPairs provide bidirectional signaling between two parties.
/// Each endpoint can signal and wait on its peer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EventPair {
    handle: Handle,
}

impl EventPair {
    /// Create a new event pair
    pub fn create() -> Result<(Self, Self)> {
        let (ep_a, ep_b) = libsys::EventPair::create()?;
        Ok((
            Self { handle: *ep_a.handle() },
            Self { handle: *ep_b.handle() },
        ))
    }

    /// Create an event pair from a raw handle
    ///
    /// # Safety
    ///
    /// The handle must be a valid event pair handle.
    pub unsafe fn from_handle(handle: Handle) -> Self {
        Self { handle }
    }

    /// Get the underlying handle
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Signal the peer
    pub fn signal_peer(&self) -> Result<()> {
        if !self.handle.rights().contains(libsys::Rights::SIGNAL_PEER) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = libsys::syscall::syscall1(
                SyscallNumber::ObjectSignalPeer as u64,
                self.handle.raw() as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Wait for the peer to signal
    ///
    /// # Arguments
    ///
    /// * `deadline` - Optional deadline in nanoseconds (0 = wait forever)
    pub fn wait(&self, deadline: u64) -> Result<()> {
        if !self.handle.rights().contains(libsys::Rights::WAIT) {
            return Err(Error::new(Status::AccessDenied));
        }

        unsafe {
            let ret = libsys::syscall::syscall3(
                SyscallNumber::ObjectWaitOne as u64,
                self.handle.raw() as u64,
                deadline,
                0, // signals
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }
}
