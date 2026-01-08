// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture-Specific Virtual Address Space
//!
//! This module provides the interface for architecture-specific
//! address space implementations.


use crate::rustux::types::*;

/// Result type for VM operations
pub type Result = core::result::Result<(), Status>;

/// Architecture-specific VM address space interface
pub trait ArchVmAspaceInterface {
    /// Create a new address space
    fn new(base: VAddr, size: usize) -> Self where Self: Sized;

    /// Destroy the address space
    fn destroy(&mut self);

    /// Map a physical page
    fn map(&mut self, pa: PAddr, va: VAddr, flags: u64) -> Result;

    /// Unmap a virtual page
    fn unmap(&mut self, va: VAddr) -> Result;

    /// Protect a region
    fn protect(&mut self, va: VAddr, len: usize, flags: u64) -> Result;
}
