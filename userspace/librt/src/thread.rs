// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Thread management
//!
//! This module provides thread creation and management functionality.

#![no_std]

use core::cell::UnsafeCell;
use libsys::{Result, Handle, Error, Status, Process, Thread as SysThread, syscall::SyscallNumber};

/// Default stack size for new threads (8 MB)
const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Thread identifier
pub type ThreadId = usize;

/// Thread object
///
/// Represents a thread that can be joined or detached.
#[repr(C)]
#[derive(Debug)]
pub struct Thread {
    /// Handle to the thread
    handle: SysThread,
    /// Thread ID
    id: ThreadId,
    /// Whether the thread has been detached
    detached: bool,
}

// Thread is Send and Sync because the handle is managed safely
unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    /// Spawn a new thread
    ///
    /// # Arguments
    ///
    /// * `func` - The thread entry function
    /// * `arg` - Argument to pass to the thread function
    pub fn spawn(func: extern "C" fn(*mut u8), arg: *mut u8) -> Result<Self> {
        ThreadBuilder::new().spawn(func, arg)
    }

    /// Get the thread ID
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// Join the thread, waiting for it to complete
    pub fn join(self) -> Result<()> {
        if self.detached {
            return Err(Error::new(Status::InvalidArgs));
        }

        // TODO: Implement proper thread joining
        // For now, we'll use a simple wait on the thread handle
        unsafe {
            let ret = libsys::syscall::syscall3(
                SyscallNumber::ObjectWaitOne as u64,
                self.handle.handle().raw() as u64,
                0, // deadline (wait forever)
                0, // signals
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Detach the thread, allowing it to run independently
    pub fn detach(mut self) {
        self.detached = true;
        // Drop self to release the handle
    }

    /// Get a handle to the current thread
    pub fn current() -> Result<Self> {
        let handle = SysThread::self_handle()?;
        Ok(Self {
            id: handle.handle().raw() as ThreadId,
            handle,
            detached: false,
        })
    }

    /// Yield execution to another thread
    pub fn yield_now() {
        unsafe {
            libsys::syscall::syscall0(SyscallNumber::ThreadYield as u64);
        }
    }

    /// Sleep for the specified duration
    ///
    /// # Arguments
    ///
    /// * `nanos` - Duration to sleep in nanoseconds
    pub fn sleep(nanos: u64) {
        unsafe {
            libsys::syscall::syscall2(
                SyscallNumber::ThreadSleep as u64,
                nanos,
                0, // deadline (relative sleep)
            );
        }
    }

    /// Exit the current thread
    pub fn exit() -> ! {
        SysThread::exit()
    }
}

/// Builder for creating threads with custom configuration
#[repr(C)]
#[derive(Debug, Clone)]
pub struct ThreadBuilder {
    /// Stack size for the new thread
    stack_size: usize,
    /// Thread name (optional)
    name: Option<&'static str>,
    /// Initial thread state
    suspended: bool,
}

impl ThreadBuilder {
    /// Create a new ThreadBuilder with default settings
    pub fn new() -> Self {
        Self {
            stack_size: DEFAULT_STACK_SIZE,
            name: None,
            suspended: false,
        }
    }

    /// Set the stack size for the new thread
    pub fn stack_size(mut self, size: usize) -> Self {
        self.stack_size = size;
        self
    }

    /// Set the name for the new thread
    pub fn name(mut self, name: &'static str) -> Self {
        self.name = Some(name);
        self
    }

    /// Set whether the thread starts suspended
    pub fn suspended(mut self, suspended: bool) -> Self {
        self.suspended = suspended;
        self
    }

    /// Spawn the thread
    pub fn spawn(self, func: extern "C" fn(*mut u8), arg: *mut u8) -> Result<Thread> {
        // TODO: Get the process handle
        // For now, we'll create a thread in the current process
        let process_handle = Handle::INVALID;

        unsafe {
            // Create a VMO for the stack
            let stack_vmo = libsys::Vmo::create(self.stack_size as u64, None)?;

            // Allocate stack memory
            let stack_bottom = self.stack_size as usize;

            // Create the thread
            let name_cstr = if let Some(name) = self.name {
                core::ffi::CStr::from_bytes_with_nul_unchecked(name.as_bytes())
            } else {
                core::ffi::CStr::from_bytes_with_nul_unchecked(b"thread\0")
            };

            let thread = libsys::thread::create(&Process { handle: process_handle }, name_cstr)?;

            // Allocate and set up the stack
            // TODO: Map the VMO into the address space

            // Start the thread
            if !self.suspended {
                libsys::thread::start(&thread, func as usize, arg as usize)?;
            }

            Ok(Thread {
                id: thread.handle().raw() as ThreadId,
                handle: thread,
                detached: false,
            })
        }
    }
}

impl Default for ThreadBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-local storage key
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadLocalKey {
    key: u32,
}

impl ThreadLocalKey {
    /// Create a new thread-local storage key
    pub fn new() -> Result<Self> {
        // TODO: Implement TLS key creation
        Ok(Self { key: 0 })
    }

    /// Get the value for this key in the current thread
    pub fn get(&self) -> *mut u8 {
        // TODO: Implement TLS get
        core::ptr::null_mut()
    }

    /// Set the value for this key in the current thread
    pub fn set(&self, value: *mut u8) {
        // TODO: Implement TLS set
    }
}
