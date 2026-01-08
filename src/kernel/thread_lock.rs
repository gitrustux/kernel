// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Thread Lock Module (Stub)
//!
//! Minimal stub for thread locking functionality.

#![no_std]

/// Thread lock guard
pub struct ThreadLock;

impl ThreadLock {
    pub fn new() -> Self {
        Self
    }
}

/// Lock guard
pub struct Guard;

impl Guard {
    pub fn new() -> Self {
        Self
    }
}

/// IRQ save state
pub struct IrqSave;

impl IrqSave {
    pub fn new() -> Self {
        Self
    }
}
