// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Rustux common types and utilities

#![no_std]

pub mod types;
pub mod errors;
pub mod compiler;
pub mod tls;

// Thread annotations for lock analysis (stubs for compatibility)
pub mod thread_annotations {
    // Empty module - annotations are compiler hints that don't affect code generation
}

// Type aliases for C compatibility
pub use types::VAddr as vaddr_t;
pub use types::PAddr as paddr_t;
pub use types::Size as size_t;
pub use types::SSize as ssize_t;
pub use types::Status as rx_status_t;

// Re-export common types
pub use types::*;
pub use types::err::*;
pub use errors::*;
pub use compiler::*;

// Re-export StatusTrait for status checking
pub use types::StatusTrait;

// Re-export syscalls from kernel for compatibility
// This allows code to use crate::rustux::syscalls
pub use crate::kernel::syscalls;
