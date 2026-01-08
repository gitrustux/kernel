// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 Exceptions Module (Stub)
//!
//! Minimal stub for exception handling.

#![no_std]

/// Exception frame
#[repr(C)]
pub struct ExceptionFrame {
    pub regs: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    pub pstate: u64,
    pub esr: u64,
    pub far: u64,
}

/// Initialize exception handling
pub fn init() {
    // TODO: Initialize exception handling
}
