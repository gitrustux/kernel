// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Compiler-specific utilities
//!
//! This module provides compiler-specific attributes and utilities.

/// Compiler barrier
#[inline(always)]
pub fn compiler_barrier() {
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Optimize away function
#[inline(always)]
pub fn optimize_away() {
    core::hint::black_box(());
}

/// Hint that a condition is likely false (branch prediction hint)
#[inline(always)]
pub fn unlikely(b: bool) -> bool {
    if b {
        core::hint::black_box(());
    }
    b
}

/// Hint that a condition is likely true (branch prediction hint)
#[inline(always)]
pub fn likely(b: bool) -> bool {
    if !b {
        core::hint::black_box(());
    }
    b
}
