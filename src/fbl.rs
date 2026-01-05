// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! FBL (Fuchsia Base Library) Compatibility Module
//!
//! This module provides minimal stubs for FBL types and utilities
//! that are used throughout the kernel.

#![no_std]

// Atomic types from FBL
pub mod atomic {
    use core::sync::atomic::{AtomicI32 as CoreAtomicI32, AtomicI64 as CoreAtomicI64, Ordering};

    /// Atomic integer type (32-bit)
    pub type AtomicInt = CoreAtomicI32;

    /// Atomic integer type (64-bit)
    pub type AtomicInt64 = CoreAtomicI64;
}

// Canary for stack protection
pub mod canary {
    /// Stack canary value for overflow detection
    #[derive(Debug, Clone, Copy)]
    pub struct Canary {
        value: u64,
    }

    impl Canary {
        /// Create a new canary with a random value
        pub fn new() -> Self {
            // TODO: Use proper random source
            Self { value: 0xDEADBEEFCAFEBABE }
        }

        /// Create a new canary with a specific magic value
        pub fn with_magic(magic: u32) -> Self {
            Self { value: magic as u64 }
        }

        /// Get the canary value
        pub fn value(&self) -> u64 {
            self.value
        }

        /// Assert that the canary has the expected magic value
        pub fn assert_magic(&self, expected: u32) -> bool {
            self.value == expected as u64
        }
    }

    impl Default for Canary {
        fn default() -> Self {
            Self::new()
        }
    }
}

// Mutex support
pub mod mutex {
    use crate::kernel::sync::spin::SpinMutex as InnerMutex;

    /// FBL-compatible Mutex
    pub struct Mutex<T> {
        inner: InnerMutex<T>,
    }

    impl<T: Default> Mutex<T> {
        pub fn new() -> Self {
            Self {
                inner: InnerMutex::new(T::default()),
            }
        }

        pub fn lock(&self) -> core::sync::atomic::Ordering {
            // Return ordering for compatibility
            core::sync::atomic::Ordering::Acquire
        }
    }

    impl<T: Default> Default for Mutex<T> {
        fn default() -> Self {
            Self::new()
        }
    }
}

// Re-export Mutex at crate level
pub use mutex::Mutex;

// Atomic boolean support
pub mod atomic_bool {
    use core::sync::atomic::{AtomicBool, Ordering};

    /// FBL-compatible AtomicBool
    pub struct AtomicBoolType {
        inner: AtomicBool,
    }

    impl AtomicBoolType {
        pub fn new(val: bool) -> Self {
            Self {
                inner: AtomicBool::new(val),
            }
        }

        pub fn load(&self, ordering: Ordering) -> bool {
            self.inner.load(ordering)
        }
    }
}

// Re-export at crate level
pub use atomic_bool::AtomicBoolType;

// Reference counting support
pub mod rc {
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Reference counted base trait
    pub trait RefCounted {
        /// Add a reference
        fn add_ref(&self);
        /// Release a reference
        fn release(&self) -> bool;
    }

    /// Atomic reference counter
    #[repr(C)]
    pub struct RefCount {
        count: AtomicUsize,
    }

    impl RefCount {
        pub const fn new() -> Self {
            Self {
                count: AtomicUsize::new(1),
            }
        }

        pub fn add_ref(&self) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }

        pub fn release(&self) -> bool {
            self.count.fetch_sub(1, Ordering::Release) == 1
        }

        pub fn count(&self) -> usize {
            self.count.load(Ordering::Relaxed)
        }
    }

    impl Default for RefCount {
        fn default() -> Self {
            Self::new()
        }
    }
}
