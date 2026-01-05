// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 I/O Port Management
//!
//! This module provides I/O port bitmap management for x86.

#![no_std]

/// I/O port bitmap
#[repr(C)]
pub struct IoBitmap {
    /// Bitmap data
    pub bitmap: [u8; 0x1000],
}

impl IoBitmap {
    /// Create a new I/O bitmap
    pub const fn new() -> Self {
        Self {
            bitmap: [0; 0x1000],
        }
    }
}
