// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V user space entry
//!
//! This module provides declarations for the user space entry functions
//! defined in uspace_entry.S.

use crate::rustux::types::*;

/// Enter user space (simple calling convention)
///
/// # Arguments
///
/// * `sp` - User stack pointer
/// * `pc` - User program counter
/// * `arg` - User function argument
///
/// # Safety
///
/// This function never returns. All arguments must be valid user space addresses.
extern "C" {
    pub fn riscv_uspace_entry_simple(sp: usize, pc: usize, arg: usize) -> !;

    /// Return to user space from exception
    ///
    /// # Arguments
    ///
    /// * `iframe` - Pointer to interrupt frame
    ///
    /// # Safety
    ///
    /// This function never returns. The iframe must be valid.
    pub fn riscv_uspace_exception_return(iframe: *mut ()) -> !;
}
