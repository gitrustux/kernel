// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Heap Allocator
//!
//! This module provides a simple bump allocator for the kernel.
//! For now, it uses a static buffer as the heap.

#![no_std]

use alloc::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_SIZE: usize = 1024 * 1024; // 1 MB heap

#[repr(align(16))]
struct AlignedHeap {
    data: [u8; HEAP_SIZE],
}

static mut HEAP: AlignedHeap = AlignedHeap { data: [0; HEAP_SIZE] };

static HEAP_OFFSET: AtomicUsize = AtomicUsize::new(0);

/// Simple bump allocator for kernel heap
struct BumpAllocator;

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let offset = HEAP_OFFSET.fetch_add(layout.size(), Ordering::Relaxed);
        let aligned_offset = (offset + layout.align() - 1) & !(layout.align() - 1);

        if aligned_offset + layout.size() > HEAP_SIZE {
            // Out of memory
            return core::ptr::null_mut();
        }

        HEAP.data.as_ptr().add(aligned_offset) as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't free memory
        // TODO: Implement a proper allocator with deallocation
    }
}

/// Global allocator instance
#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;

/// Initialize the heap allocator
pub fn init() {
    HEAP_OFFSET.store(0, Ordering::Release);
}

/// Get heap usage statistics
pub fn heap_usage() -> usize {
    HEAP_OFFSET.load(Ordering::Relaxed)
}

/// Get total heap size
pub fn heap_size() -> usize {
    HEAP_SIZE
}
