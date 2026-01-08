// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Heap Allocator
//!
//! This module provides a linked list allocator for the kernel heap.
//! It supports allocation, deallocation, and memory reuse.


use alloc::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MB heap

#[repr(align(16))]
struct AlignedHeap {
    data: [u8; HEAP_SIZE],
}

static mut HEAP: AlignedHeap = AlignedHeap { data: [0; HEAP_SIZE] };

/// Heap block header for free list
#[repr(C)]
struct BlockHeader {
    /// Size of this block (including header)
    size: usize,

    /// Whether this block is free
    free: bool,

    /// Previous block in the list
    prev: Option<*mut BlockHeader>,

    /// Next block in the list
    next: Option<*mut BlockHeader>,
}

impl BlockHeader {
    /// Get the end of this block
    fn end(&self) -> *mut u8 {
        (self as *const BlockHeader as usize + self.size) as *mut u8
    }
}

/// Linked list allocator
struct LinkedListAllocator {
    /// First free block
    free_list: AtomicUsize,
}

unsafe impl GlobalAlloc for LinkedListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        // Calculate aligned size (at least size of BlockHeader)
        let block_size = core::cmp::max(size, core::mem::size_of::<BlockHeader>())
            .next_power_of_two();

        // Get the free list head
        let mut free_list = self.free_list.load(Ordering::Acquire) as *mut BlockHeader;

        // Search for a suitable free block
        let mut current = free_list;
        let mut prev: *mut BlockHeader = core::ptr::null_mut();

        while !current.is_null() {
            let block = &*current;

            if block.free && block.size >= block_size {
                // Found a suitable block
                let remaining = block.size - block_size;

                if remaining >= core::mem::size_of::<BlockHeader>() {
                    // Split the block
                    let new_block = (current as usize + block_size) as *mut BlockHeader;
                    (*new_block).size = remaining;
                    (*new_block).free = true;
                    (*new_block).prev = Some(current);
                    (*new_block).next = block.next;

                    (*current).size = block_size;
                    (*current).next = Some(new_block);
                }

                // Mark block as used
                (*current).free = false;

                // Remove from free list
                if !prev.is_null() {
                    (*prev).next = (*current).next;
                } else {
                    if let Some(next) = (*current).next {
                        self.free_list.store(next as usize, Ordering::Release);
                    } else {
                        self.free_list.store(0, Ordering::Release);
                    }
                }
                if let Some(next) = (*current).next {
                    (*next).prev = Some(prev);
                }

                // Return pointer after the header
                return (current as usize + core::mem::size_of::<BlockHeader>()) as *mut u8;
            }

            prev = current;
            current = if let Some(next) = block.next { next } else { core::ptr::null_mut() };
        }

        // No suitable free block found
        // For now, we'll extend the heap (simplified approach)
        // In a full implementation, this would use sbrk or similar
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }

        // Get the block header
        let block = (ptr as usize - core::mem::size_of::<BlockHeader>()) as *mut BlockHeader;

        // Mark block as free
        (*block).free = true;

        // Try to merge with next block if it's free
        if let Some(next) = (*block).next {
            if (*next).free {
                // Merge with next block
                (*block).size += (*next).size;
                (*block).next = (*next).next;
                if let Some(next_next) = (*next).next {
                    (*next_next).prev = Some(block);
                }
            }
        }

        // Try to merge with previous block if it's free
        if let Some(prev) = (*block).prev {
            if (*prev).free {
                // Merge with previous block
                (*prev).size += (*block).size;
                (*prev).next = (*block).next;
                if let Some(next) = (*block).next {
                    (*next).prev = Some(prev);
                }
            }
        }

        // Add to free list
        let free_list = self.free_list.load(Ordering::Acquire) as *mut BlockHeader;
        (*block).next = Some(free_list);
        if !free_list.is_null() {
            (*free_list).prev = Some(block);
        }
        (*block).prev = None;
        self.free_list.store(block as usize, Ordering::Release);
    }
}

/// Global allocator instance
#[global_allocator]
static ALLOCATOR: LinkedListAllocator = LinkedListAllocator {
    free_list: AtomicUsize::new(0),
};

/// Initialize the heap allocator
pub fn init() {
    unsafe {
        // Initialize the heap as a single free block
        let heap_start = HEAP.data.as_ptr() as usize;
        let block = heap_start as *mut BlockHeader;

        (*block).size = HEAP_SIZE - core::mem::size_of::<BlockHeader>();
        (*block).free = true;
        (*block).prev = None;
        (*block).next = None;

        ALLOCATOR.free_list.store(block as usize, Ordering::Release);
    }
}

/// Get heap usage statistics
pub fn heap_usage() -> usize {
    let mut used = 0usize;
    unsafe {
        let mut current = ALLOCATOR.free_list.load(Ordering::Acquire) as *mut BlockHeader;

        // Count all blocks (both free and used)
        let heap_start = HEAP.data.as_ptr() as usize;
        let heap_end = heap_start + HEAP_SIZE;
        let mut block = heap_start as *mut BlockHeader;

        while (block as usize) < heap_end {
            if !(*block).free {
                used += (*block).size;
            }
            block = ((*block).end()) as *mut BlockHeader;
        }
    }
    used
}

/// Get total heap size
pub fn heap_size() -> usize {
    HEAP_SIZE
}
