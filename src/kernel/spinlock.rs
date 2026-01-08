// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Spinlock primitives


/// Re-export SpinLock from sync module
pub use crate::kernel::sync::spin::SpinLock;

/// Create a new spinlock
pub fn spinlock<T>(val: T) -> SpinLock<T> {
    SpinLock::new(val)
}
