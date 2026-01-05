// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux System Library (libsys)
//!
//! This library provides the core userspace API for Rustux, including:
//! - Raw syscall interface
//! - Handle types for kernel objects
//! - Error handling
//! - Object type definitions
//!
//! # Examples
//!
//! ```no_run
//! use libsys::*;
//!
//! fn main() -> Result<()> {
//!     // Create a VMO
//!     let vmo = Vmo::create(4096, None)?;
//!
//!     // Write to it
//!     let data = b"Hello, Rustux!";
//!     vmo.write(0, data)?;
//!
//!     // Read from it
//!     let mut buf = [0u8; 64];
//!     let n = vmo.read(0, &mut buf)?;
//!
//!     println!("Read: {}", core::str::from_utf8(&buf[..n]).unwrap());
//!
//!     Ok(())
//! }
//! ```

#![no_std]

// Core modules
pub mod error;
pub mod syscall;
pub mod handles;
pub mod object;

// Re-export commonly used types
pub use error::{Error, Result, Status};
pub use syscall::SyscallNumber;
pub use handles::{Handle, Rights, Process, Thread, Vmo, Channel, Event, Port};
pub use object::{ObjectType, ObjectInfo, HandleInfo, VmoInfo, ProcessInfo, ThreadInfo};

// C-compatible FFI exports
pub mod cffi {
    use super::*;

    /// Get the last error
    #[no_mangle]
    pub extern "C" fn sys_last_error() -> i32 {
        // TODO: Implement thread-local error storage
        0
    }

    /// Get error message
    #[no_mangle]
    pub extern "C" fn sys_strerror(err: i32) -> *const u8 {
        // TODO: Implement error string conversion
        // For now, return a static string
        static UNKNOWN: &[u8] = b"Unknown error\0";
        UNKNOWN.as_ptr()
    }
}

/// Process-related syscalls
pub mod process {
    use super::*;
    use crate::syscall::SyscallNumber;

    /// Create a new process
    pub fn create(
        parent_job: &Handle,
        name: &core::ffi::CStr,
        flags: u32,
    ) -> Result<Process> {
        unsafe {
            let ret = syscall3(
                SyscallNumber::ProcessCreate as u64,
                parent_job.raw() as u64,
                name.as_ptr() as u64,
                flags as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Process {
                handle: Handle::from_raw(ret as u32, Rights::all()),
            })
        }
    }

    /// Start a process
    pub fn start(process: &Process, thread: &Thread, entry: usize, stack: usize) -> Result<()> {
        unsafe {
            let ret = syscall4(
                SyscallNumber::ProcessStart as u64,
                process.handle().raw() as u64,
                thread.handle().raw() as u64,
                entry as u64,
                stack as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Exit the current process
    pub fn exit(code: i32) -> ! {
        Process::exit(code)
    }
}

/// Thread-related syscalls
pub mod thread {
    use super::*;
    use crate::syscall::SyscallNumber;

    /// Create a new thread
    pub fn create(
        process: &Process,
        name: &core::ffi::CStr,
    ) -> Result<Thread> {
        unsafe {
            let ret = syscall2(
                SyscallNumber::ThreadCreate as u64,
                process.handle().raw() as u64,
                name.as_ptr() as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Thread {
                handle: Handle::from_raw(ret as u32, Rights::all()),
            })
        }
    }

    /// Start a thread
    pub fn start(thread: &Thread, entry: usize, arg: usize) -> Result<()> {
        unsafe {
            let ret = syscall3(
                SyscallNumber::ThreadStart as u64,
                thread.handle().raw() as u64,
                entry as u64,
                arg as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Get a handle to the current thread
    pub fn self_handle() -> Result<Thread> {
        Thread::self_handle()
    }

    /// Exit the current thread
    pub fn exit() -> ! {
        Thread::exit()
    }
}

/// VMAR-related syscalls
pub mod vmar {
    use super::*;
    use crate::syscall::SyscallNumber;

    /// Get the root VMAR for the current process
    pub fn root_self() -> Result<Handle> {
        unsafe {
            let mut out: u64 = 0;

            let ret = syscall1(
                SyscallNumber::VmarRootSelf as u64,
                &mut out as *mut u64 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Handle::from_raw(out as u32, Rights::all()))
        }
    }

    /// Map memory into the address space
    pub fn map(
        vmar: &Handle,
        vaddr: usize,
        vmo: &Vmo,
        offset: u64,
        len: usize,
        flags: u32,
    ) -> Result<usize> {
        unsafe {
            let mut mapped_addr: u64 = 0;

            let ret = syscall6(
                SyscallNumber::VmarMap as u64,
                vmar.raw() as u64,
                vaddr as u64,
                0, // vmar_offset (not used)
                vmo.handle().raw() as u64,
                offset,
                len as u64,
                flags as u64,
                &mut mapped_addr as *mut u64 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(mapped_addr as usize)
        }
    }

    /// Unmap memory from the address space
    pub fn unmap(vmar: &Handle, vaddr: usize, len: usize) -> Result<()> {
        unsafe {
            let ret = syscall3(
                SyscallNumber::VmarUnmap as u64,
                vmar.raw() as u64,
                vaddr as u64,
                len as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }

    /// Change memory protection
    pub fn protect(
        vmar: &Handle,
        vaddr: usize,
        len: usize,
        flags: u32,
    ) -> Result<()> {
        unsafe {
            let ret = syscall4(
                SyscallNumber::VmarProtect as u64,
                vmar.raw() as u64,
                vaddr as u64,
                len as u64,
                flags as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(())
        }
    }
}

/// Bootstrap syscalls (available during process startup)
pub mod bootstrap {
    use super::*;
    use crate::syscall::SyscallNumber;

    /// Get process arguments
    pub fn proc_args() -> Result<(*const u8, usize)> {
        unsafe {
            let mut args: u64 = 0;
            let mut size: u64 = 0;

            let ret = syscall2(
                SyscallNumber::ProcArgs as u64,
                &mut args as *mut u64 as u64,
                &mut size as *mut u64 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok((args as *const u8, size as usize))
        }
    }

    /// Get the default job handle
    pub fn job_default() -> Result<Handle> {
        unsafe {
            let mut out: u64 = 0;

            let ret = syscall1(
                SyscallNumber::JobDefault as u64,
                &mut out as *mut u64 as u64,
            );

            if (ret as i32) < 0 {
                return Err(Error::from_raw(ret as i32));
            }

            Ok(Handle::from_raw(out as u32, Rights::all()))
        }
    }
}
