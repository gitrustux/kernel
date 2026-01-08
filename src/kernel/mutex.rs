// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Mutex Module (Stub)
//!
//! Minimal stub for mutex functionality.

#![no_std]

use crate::kernel::sync::spin::SpinMutex;

/// Simple mutex type alias
pub type Mutex<T> = SpinMutex<T>;

/// Create a new mutex
pub fn new_mutex<T>(val: T) -> Mutex<T> {
    SpinMutex::new(val)
}
