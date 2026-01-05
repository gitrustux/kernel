// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux Runtime Library (librt)
//!
//! This library provides runtime support for userspace programs:
//! - Thread creation and management
//! - Synchronization primitives (Mutex, Condvar)
//! - Timer helpers
//!
//! # Examples
//!
//! ```no_run
//! use librt::*;
//! use libsys::*;
//!
//! fn main() -> Result<()> {
//!     // Spawn a thread
//!     let thread = Thread::spawn(my_thread_func, Box::into_raw(Box::new(42)) as *mut u8)?;
//!
//!     // Wait for it to complete
//!     thread.join()?;
//!
//!     Ok(())
//! }
//!
//! extern "C" fn my_thread_func(arg: *mut u8) {
//!     let value = arg as usize;
//!     println!("Thread received: {}", value);
//! }
//! ```

#![no_std]

pub mod thread;
pub mod mutex;
pub mod condvar;
pub mod timer;

// Re-export commonly used types
pub use thread::{Thread, ThreadBuilder};
pub use mutex::{Mutex, MutexGuard};
pub use condvar::{Condvar, WaitResult};
pub use timer::{Timer, TimerId};
