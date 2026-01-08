// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Synchronization Primitives
//!
//! This module provides core synchronization primitives for the Rustux kernel.
//! These are designed for kernel-internal use and provide proper locking,
//! waiting, and signaling mechanisms.
//!
//! # Primitives
//!
//! - **Mutex**: Mutual exclusion lock with thread ownership tracking
//! - **Event**: Single-signal synchronization primitive
//! - **Wait Queue**: Queue for threads waiting on a condition
//!
//! # Design
//!
//! All primitives are designed to work with the scheduler and provide
//! proper integration with the thread blocking/waking mechanisms.

#![no_std]

pub mod mutex;
pub mod event;
pub mod wait_queue;
pub mod spin;

// Re-exports
pub use mutex::*;
pub use event::*;
pub use wait_queue::*;
pub use spin::*;
