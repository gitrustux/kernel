// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Atomic Operations Module
//!
//! This module provides atomic types and operations for kernel use.


// Re-export core atomic types with kernel-specific aliases
pub use core::sync::atomic::{
    AtomicBool,
    AtomicI16,
    AtomicI32,
    AtomicI64,
    AtomicI8,
    AtomicIsize,
    AtomicPtr,
    AtomicU16,
    AtomicU32,
    AtomicU64,
    AtomicU8,
    AtomicUsize,
    Ordering,
};

/// Atomic helper functions
pub mod atomic_ops {
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Atomic increment with specified ordering
    #[inline]
    pub fn atomic_fetch_add(ptr: &AtomicUsize, val: usize) -> usize {
        ptr.fetch_add(val, Ordering::AcqRel)
    }

    /// Atomic decrement with specified ordering
    #[inline]
    pub fn atomic_fetch_sub(ptr: &AtomicUsize, val: usize) -> usize {
        ptr.fetch_sub(val, Ordering::AcqRel)
    }

    /// Atomic exchange with specified ordering
    #[inline]
    pub fn atomic_swap(ptr: &AtomicUsize, val: usize) -> usize {
        ptr.swap(val, Ordering::AcqRel)
    }

    /// Compare and swap with specified ordering
    #[inline]
    pub fn atomic_cmpxchg(ptr: &AtomicUsize, old: usize, new: usize) -> Result<usize, usize> {
        ptr.compare_exchange(old, new, Ordering::AcqRel, Ordering::Acquire)
    }

    /// Signal fence - prevents compiler reordering across this point
    #[inline]
    pub fn atomic_signal_fence(order: Ordering) {
        core::sync::atomic::fence(order);
    }
}
