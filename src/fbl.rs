// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! FBL (Fuchsia Base Library) Compatibility Module
//!
//! This module provides minimal stubs for FBL types and utilities
//! that are used throughout the kernel.


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

// Algorithm support
pub mod algorithm {
    /// Find the first set bit in a word
    pub fn find_first_set_bit(word: u64) -> Option<u32> {
        if word == 0 {
            None
        } else {
            Some(word.trailing_zeros())
        }
    }

    /// Find the last set bit in a word
    pub fn find_last_set_bit(word: u64) -> Option<u32> {
        if word == 0 {
            None
        } else {
            Some(63 - word.leading_zeros())
        }
    }

    /// Count the number of set bits in a word
    pub fn count_set_bits(word: u64) -> u32 {
        word.count_ones()
    }
}

// Auto-call support
pub mod auto_call {
    /// Auto-call trait for initialization
    pub trait AutoCall {
        fn auto_call(&self);
    }
}

// Auto-lock support
pub mod auto_lock {
    use crate::kernel::sync::spin::SpinMutex;

    /// RAII-style lock guard
    pub struct AutoLock<'a, T> {
        _guard: core::sync::atomic::AtomicBool,
        _data: core::marker::PhantomData<&'a T>,
    }

    impl<'a, T> AutoLock<'a, T> {
        pub fn new(_mutex: &'a SpinMutex<T>) -> Self {
            Self {
                _guard: core::sync::atomic::AtomicBool::new(false),
                _data: core::marker::PhantomData,
            }
        }
    }
}

// RefPtr (smart pointer) support
pub mod ref_ptr {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use alloc::boxed::Box;

    /// Reference counted smart pointer
    pub struct RefPtr<T> {
        ptr: *mut T,
        ref_count: AtomicUsize,
    }

    impl<T> RefPtr<T> {
        pub fn new(val: T) -> Self {
            let boxed = Box::into_raw(Box::new(val));
            Self {
                ptr: boxed,
                ref_count: AtomicUsize::new(1),
            }
        }

        pub fn clone(&self) -> Self {
            self.ref_count.fetch_add(1, Ordering::Relaxed);
            Self {
                ptr: self.ptr,
                ref_count: AtomicUsize::new(0),
            }
        }

        pub unsafe fn deref(&self) -> &T {
            &*self.ptr
        }
    }

    impl<T> Drop for RefPtr<T> {
        fn drop(&mut self) {
            if self.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                unsafe {
                    let _ = Box::from_raw(self.ptr);
                }
            }
        }
    }
}

// Re-export RefPtr at crate level for convenience
pub use ref_ptr::RefPtr;
