// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Debug and Tracing Support (Top-Level)
//!
//! This is a minimal debug module at the crate root. The actual logging
//! implementation is in kernel::debug. This module exists for compatibility
//! with code that imports from the top-level debug module.

#![no_std]

// Re-export the actual debug module from kernel
pub use crate::kernel::debug::*;
