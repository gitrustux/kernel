// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Thread Lock Module (Stub)
//!
//! Minimal stub for thread locking functionality.


/// Thread lock guard
pub struct ThreadLock;

impl ThreadLock {
    pub fn new() -> Self {
        Self
    }

    /// Get the thread lock instance
    pub fn get() -> Self {
        Self
    }
}

/// Lock guard
pub struct Guard<T, U> {
    _phantom: core::marker::PhantomData<(T, U)>,
}

impl<T, U> Guard<T, U> {
    pub fn new(_lock: T) -> Self {
        Guard {
            _phantom: core::marker::PhantomData,
        }
    }
}

/// IRQ save state
pub struct IrqSave;

impl IrqSave {
    pub fn new() -> Self {
        Self
    }
}
