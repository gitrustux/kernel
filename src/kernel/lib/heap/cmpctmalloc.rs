// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Compact Malloc Allocator
//!
//! This is a space-optimized malloc implementation tuned for embedded systems.
//! It uses:
//! - A global mutex for thread safety
//! - Free lists with 8 different sizes per binary order of magnitude
//! - Two-word headers with eager coalescing on free
//! - Bucket-based allocation to avoid fragmentation
//!
//! ## Key Concepts
//!
//! **OS allocation**: A contiguous range of pages allocated from the OS using
//! heap_page_alloc(), typically via heap_grow(). Each OS allocation contains:
//! - A left sentinel (allocated, size = sizeof(header_t), left = NULL)
//! - A free memory area (available for allocation)
//! - A right sentinel (allocated, size = 0)
//!
//! **Memory area**: A sub-range of an OS allocation used to satisfy allocation
//! requests. Can be free (in a free bucket) or allocated.
//!
//! **Normal allocation**: An allocation less than HEAP_LARGE_ALLOC_BYTES that
//! fits in a free bucket.
//!
//! **Free buckets**: Freelist entries kept in linked lists with 8 different sizes
//! per binary order of magnitude. Allocations are rounded up to the nearest bucket
//! size to avoid fragmentation.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Size to grow heap by default
const HEAP_GROW_SIZE: usize = 1024 * 1024;

/// Virtual bits for heap allocation
const HEAP_ALLOC_VIRTUAL_BITS: u32 = 22;

/// Large allocation threshold (4MB)
const HEAP_LARGE_ALLOC_BYTES: usize = 1 << HEAP_ALLOC_VIRTUAL_BITS;

/// Number of buckets: 15 small buckets (8-120 bytes) + 8 buckets per order of magnitude
const NUMBER_OF_BUCKETS: usize = 1 + 15 + ((HEAP_ALLOC_VIRTUAL_BITS - 7) * 8) as usize;

/// Number of words for free list bits
const BUCKET_WORDS: usize = (NUMBER_OF_BUCKETS + 31) >> 5;

/// Page size
const PAGE_SIZE: usize = 4096;

/// Page size shift
const PAGE_SIZE_SHIFT: u32 = 12;

/// Free bit in header.left
const FREE_BIT: usize = 1;

/// Header left bit mask
const HEADER_LEFT_BIT_MASK: usize = FREE_BIT;

/// Header structure for each memory area
#[repr(C)]
#[derive(Debug)]
struct Header {
    /// Pointer to previous area (with FREE_BIT in low bit if free)
    left: *mut Header,
    /// Size of the memory area in bytes (including this header)
    size: usize,
}

/// Free structure (extends Header with doubly-linked list pointers)
#[repr(C)]
#[derive(Debug)]
struct FreeBlock {
    /// Header (must be first field)
    header: Header,
    /// Next free block in bucket
    next: *mut FreeBlock,
    /// Previous free block in bucket
    prev: *mut FreeBlock,
}

/// Heap structure
struct Heap {
    /// Total bytes allocated from OS
    size: AtomicUsize,
    /// Bytes of usable free space
    remaining: AtomicUsize,
    /// Cached OS allocation (non-large) that could have been freed
    cached_os_alloc: Mutex<Option<*mut Header>>,
    /// Free lists, bucketed by size
    free_lists: Mutex<Vec<*mut FreeBlock>>,
    /// Bitmask tracking which free_lists entries have elements
    free_list_bits: Mutex<[u32; BUCKET_WORDS]>,
}

unsafe impl Send for Heap {}
unsafe impl Sync for Heap {}

/// Global heap instance
static THE_HEAP: Heap = Heap {
    size: AtomicUsize::new(0),
    remaining: AtomicUsize::new(0),
    cached_os_alloc: Mutex::new(None),
    free_lists: Mutex::new(Vec::new()),
    free_list_bits: Mutex::new([0u32; BUCKET_WORDS]),
};

/// Initialize the compact malloc allocator
pub fn cmpct_init() {
    println!("CmpctMalloc: Initializing ({} buckets)", NUMBER_OF_BUCKETS);

    // Initialize the free lists
    let mut free_lists = THE_HEAP.free_lists.lock();
    free_lists.clear();
    for _ in 0..NUMBER_OF_BUCKETS {
        free_lists.push(core::ptr::null_mut());
    }

    // Initialize free list bits
    let mut bits = THE_HEAP.free_list_bits.lock();
    for bit in bits.iter_mut() {
        *bit = 0;
    }

    // Set remaining to 0
    THE_HEAP.remaining.store(0, Ordering::Release);

    // Grow the heap initially
    let initial_alloc = HEAP_GROW_SIZE - 2 * core::mem::size_of::<Header>();
    let _ = heap_grow(initial_alloc);

    println!("CmpctMalloc: Initialized");
}

/// Allocate memory
///
/// # Arguments
///
/// * `size` - Size to allocate in bytes
///
/// # Returns
///
/// Pointer to allocated memory, or null if allocation failed
pub fn cmpct_alloc(size: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }

    // Large allocations are no longer allowed
    if size > (HEAP_LARGE_ALLOC_BYTES - core::mem::size_of::<Header>()) {
        println!("CmpctMalloc: Allocation too large: {}", size);
        return core::ptr::null_mut();
    }

    let rounded_up;
    let start_bucket = size_to_index_allocating(size, &mut rounded_up);
    let rounded_up = rounded_up + core::mem::size_of::<Header>();

    // Find non-empty bucket
    let bucket = match find_nonempty_bucket(start_bucket) {
        Some(b) => b,
        None => {
            // Need to grow heap
            let grow_by = HEAP_LARGE_ALLOC_BYTES
                .min(size.max(HEAP_GROW_SIZE).max(THE_HEAP.size.load(Ordering::Acquire) >> 3));

            if heap_grow(grow_by).is_err() {
                return core::ptr::null_mut();
            }

            match find_nonempty_bucket(start_bucket) {
                Some(b) => b,
                None => return core::ptr::null_mut(),
            }
        }
    };

    // Get head of free list
    let mut free_lists = THE_HEAP.free_lists.lock();
    let head = *free_lists.get(bucket).unwrap();
    let left_over = unsafe { (*head).header.size - rounded_up };

    // Check if we should carve off remainder
    let should_split = left_over >= core::mem::size_of::<FreeBlock>() && left_over > (size >> 6);

    let result = if should_split {
        // Split the block
        unsafe {
            let right = right_header(&(*head).header);
            unlink_free(head, bucket, &mut free_lists);
            let free_ptr = (head as *mut u8).add(rounded_up);
            create_free_area(
                free_ptr as *mut FreeBlock,
                head as *mut Header,
                left_over,
                &mut free_lists,
            );
            fix_left_pointer(right, free_ptr as *mut Header);
            (*head).header.size -= left_over;
            create_allocation_header(
                head as *mut u8,
                0,
                (*head).header.size,
                (*head).header.left,
            )
        }
    } else {
        // Use the entire block
        unsafe {
            unlink_free(head, bucket, &mut free_lists);
            create_allocation_header(
                head as *mut u8,
                0,
                (*head).header.size,
                (*head).header.left,
            )
        }
    };

    result
}

/// Allocate aligned memory
///
/// # Arguments
///
/// * `size` - Size to allocate in bytes
/// * `alignment` - Alignment requirement (must be power of 2)
///
/// # Returns
///
/// Pointer to allocated memory, or null if allocation failed
pub fn cmpct_memalign(size: usize, alignment: usize) -> *mut u8 {
    if alignment < 8 {
        return cmpct_alloc(size);
    }

    // Allocate extra space for alignment
    let padded_size = size + alignment + core::mem::size_of::<FreeBlock>() + core::mem::size_of::<Header>();

    let unaligned = cmpct_alloc(padded_size);
    if unaligned.is_null() {
        return core::ptr::null_mut();
    }

    // Find aligned position
    let mask = alignment - 1;
    let payload_int = (unaligned as usize)
        + core::mem::size_of::<FreeBlock>()
        + core::mem::size_of::<Header>()
        + mask;
    let payload = ((payload_int) & !mask) as *mut u8;

    if unaligned != payload {
        // Need to split the allocation
        unsafe {
            let unaligned_header = (unaligned as *mut Header).offset(-1);
            let header = (payload as *mut Header).offset(-1);
            let left_over = payload as usize - unaligned as usize;

            let mut free_lists = THE_HEAP.free_lists.lock();
            create_allocation_header(
                header as *mut u8,
                0,
                (*unaligned_header).size - left_over,
                (*unaligned_header).left,
            );
            let right = right_header(unaligned_header);
            (*unaligned_header).size = left_over;
            fix_left_pointer(right, header);
            drop(free_lists);

            cmpct_free(unaligned);
        }
    }

    // TODO: Free the part after the aligned allocation
    payload
}

/// Reallocate memory
///
/// # Arguments
///
/// * `payload` - Existing pointer (or null for new allocation)
/// * `size` - New size in bytes
///
/// # Returns
///
/// Pointer to reallocated memory, or null if allocation failed
pub fn cmpct_realloc(payload: *mut u8, size: usize) -> *mut u8 {
    if payload.is_null() {
        return cmpct_alloc(size);
    }

    unsafe {
        let header = (payload as *mut Header).offset(-1);
        let old_size = (*header).size - core::mem::size_of::<Header>();

        let new_payload = cmpct_alloc(size);
        if new_payload.is_null() {
            return core::ptr::null_mut();
        }

        // Copy old data to new allocation
        let copy_len = size.min(old_size);
        core::ptr::copy_nonoverlapping(payload, new_payload, copy_len);

        cmpct_free(payload);
        new_payload
    }
}

/// Free memory
///
/// # Arguments
///
/// * `payload` - Pointer to memory to free (null is safe)
pub fn cmpct_free(payload: *mut u8) {
    if payload.is_null() {
        return;
    }

    unsafe {
        let header = (payload as *mut Header).offset(-1);

        // Check for double free
        if is_tagged_as_free(header) {
            panic!("CmpctMalloc: Double free detected at {:p}", payload);
        }

        let size = (*header).size;
        let left = (*header).left;

        let mut free_lists = THE_HEAP.free_lists.lock();

        // Check if left neighbor is free (coalesce)
        if !left.is_null() && is_tagged_as_free(left) {
            unlink_free_unknown_bucket(left as *mut FreeBlock, &mut free_lists);
            let right = right_header(header);

            if is_tagged_as_free(right) {
                // Coalesce both sides
                unlink_free_unknown_bucket(right as *mut FreeBlock, &mut free_lists);
                let right_right = right_header(right);
                fix_left_pointer(right_right, left);
                free_memory(
                    left,
                    (*left).left,
                    (*left).size + size + (*right).size,
                    &mut free_lists,
                );
            } else {
                // Coalesce only left
                fix_left_pointer(right, left);
                free_memory(left, (*left).left, (*left).size + size, &mut free_lists);
            }
        } else {
            let right = right_header(header);

            if is_tagged_as_free(right) {
                // Coalesce only right
                let right_right = right_header(right);
                unlink_free_unknown_bucket(right as *mut FreeBlock, &mut free_lists);
                fix_left_pointer(right_right, header);
                free_memory(header, left, size + (*right).size, &mut free_lists);
            } else {
                // No coalescing
                free_memory(header, left, size, &mut free_lists);
            }
        }
    }
}

/// Trim the heap by returning free pages to the OS
pub fn cmpct_trim() {
    println!("CmpctMalloc: Trimming heap");

    // TODO: Implement trimming logic
    // Look at free list entries >= PAGE_SIZE + sizeof(Header)
    // Trim pages from start or end of OS allocations
}

/// Dump heap information
///
/// # Arguments
///
/// * `panic_time` - Whether this is being called during a panic
pub fn cmpct_dump(panic_time: bool) {
    let size = THE_HEAP.size.load(Ordering::Acquire);
    let remaining = THE_HEAP.remaining.load(Ordering::Acquire);

    println!("CmpctMalloc: Heap dump (panic_time: {})", panic_time);
    println!("  Total size: {} bytes", size);
    println!("  Remaining: {} bytes", remaining);

    let cached = THE_HEAP.cached_os_alloc.lock();
    if let Some(ptr) = *cached {
        unsafe {
            println!("  Cached OS alloc: {:p} ({} bytes)", ptr, (*ptr).size);
        }
    }
    drop(cached);

    // Dump free lists
    let free_lists = THE_HEAP.free_lists.lock();
    for (i, &free_list) in free_lists.iter().enumerate() {
        if !free_list.is_null() {
            let mut count = 0;
            let mut current = free_list;
            unsafe {
                while !current.is_null() {
                    count += 1;
                    current = (*current).next;
                }
            }
            if count > 0 {
                println!("  Bucket {}: {} entries", i, count);
            }
        }
    }
}

/// Get heap information
///
/// # Arguments
///
/// * `size_bytes` - Output: total heap size
/// * `free_bytes` - Output: free bytes available
pub fn cmpct_get_info(size_bytes: &mut usize, free_bytes: &mut usize) {
    *size_bytes = THE_HEAP.size.load(Ordering::Acquire);
    *free_bytes = THE_HEAP.remaining.load(Ordering::Acquire);
}

/// Run heap tests
pub fn cmpct_test() {
    println!("CmpctMalloc: Running tests");

    // Test basic allocation and free
    let ptr1 = cmpct_alloc(100);
    let ptr2 = cmpct_alloc(200);
    cmpct_free(ptr1);
    let ptr3 = cmpct_alloc(50);
    cmpct_free(ptr2);
    cmpct_free(ptr3);

    println!("CmpctMalloc: Tests completed");
}

/// Grow the heap by allocating from the OS
fn heap_grow(size: usize) -> Result<(), i32> {
    let size_with_sentinels = size + 2 * core::mem::size_of::<Header>();
    let page_aligned = round_up(size_with_sentinels, PAGE_SIZE);

    println!(
        "CmpctMalloc: Growing heap by {} bytes (requested: {})",
        page_aligned, size
    );

    // TODO: Allocate pages from OS
    // For now, just update the size
    THE_HEAP.size.fetch_add(page_aligned, Ordering::AcqRel);

    // TODO: Call add_to_heap with the new memory

    Ok(())
}

/// Create a free area in the heap
unsafe fn create_free_area(
    address: *mut FreeBlock,
    left: *mut Header,
    size: usize,
    free_lists: &mut Vec<*mut FreeBlock>,
) {
    (*address).header.size = size;
    (*address).header.left = tag_as_free(left);

    let index = size_to_index_freeing(size - core::mem::size_of::<Header>());
    set_free_list_bit(index);
    let bucket = &mut free_lists[index];

    let old_head = *bucket;
    if !old_head.is_null() {
        (*old_head).prev = address;
    }
    (*address).next = old_head;
    (*address).prev = core::ptr::null_mut();
    *bucket = address;

    THE_HEAP.remaining.fetch_add(size, Ordering::AcqRel);
}

/// Create an allocation header
unsafe fn create_allocation_header(
    address: *mut u8,
    offset: usize,
    size: usize,
    left: *mut Header,
) -> *mut u8 {
    let standalone = (address.add(offset)) as *mut Header;
    (*standalone).left = untag(left);
    (*standalone).size = size;
    standalone.add(1) as *mut u8
}

/// Free memory to a free bucket or to the OS
unsafe fn free_memory(
    address: *mut Header,
    left: *mut Header,
    size: usize,
    free_lists: &mut Vec<*mut FreeBlock>,
) {
    let left = untag(left);

    // Check if this covers an entire OS allocation
    if is_page_aligned(left as usize)
        && is_start_of_os_allocation(left)
        && is_end_of_os_allocation((address as *mut u8).add(size) as *mut Header)
    {
        // TODO: Return to OS
        println!("CmpctMalloc: Would return OS allocation");
    } else {
        create_free_area(address as *mut FreeBlock, left, size, free_lists);
    }
}

/// Unlink a free block from its bucket
unsafe fn unlink_free(
    free_area: *mut FreeBlock,
    bucket: usize,
    free_lists: &mut Vec<*mut FreeBlock>,
) {
    THE_HEAP.remaining.fetch_sub((*free_area).header.size, Ordering::AcqRel);

    let next = (*free_area).next;
    let prev = (*free_area).prev;

    if free_lists[bucket] == free_area {
        free_lists[bucket] = next;
        if next.is_null() {
            clear_free_list_bit(bucket);
        }
    }

    if !prev.is_null() {
        (*prev).next = next;
    }
    if !next.is_null() {
        (*next).prev = prev;
    }
}

/// Unlink a free block when bucket is unknown
unsafe fn unlink_free_unknown_bucket(free_area: *mut FreeBlock, free_lists: &mut Vec<*mut FreeBlock>) {
    let bucket = size_to_index_freeing((*free_area).header.size - core::mem::size_of::<Header>());
    unlink_free(free_area, bucket, free_lists);
}

/// Convert size to bucket index when allocating
fn size_to_index_allocating(size: usize, rounded_up_out: &mut usize) -> usize {
    let rounded = round_up(size, 8);
    size_to_index_helper(rounded, rounded_up_out, -8, 1)
}

/// Convert size to bucket index when freeing
fn size_to_index_freeing(size: usize) -> usize {
    let mut dummy = 0;
    size_to_index_helper(size, &mut dummy, 0, 0)
}

/// Helper for size-to-bucket-index conversion
fn size_to_index_helper(size: usize, rounded_up_out: &mut usize, adjust: i32, increment: i32) -> usize {
    // First 15 buckets are 8-spaced up to 128
    if size <= 128 {
        let min_size = core::mem::size_of::<FreeBlock>() - core::mem::size_of::<Header>();
        if core::mem::size_of::<usize>() == 8 && size <= min_size {
            *rounded_up_out = min_size;
        } else {
            *rounded_up_out = size;
        }
        return (size >> 3) - 1;
    }

    // After 128, logarithmically spaced: 8 buckets per order of magnitude
    let size = (size as i32 + adjust) as usize;
    let row = (core::mem::size_of::<usize>() * 8 - 4 - (size.leading_zeros() as usize)) as i32;
    let column = ((size >> row) & 7) as i32;
    let row_column = (row << 3) | column;
    let row_column = row_column + increment;
    let size = (8 + (row_column & 7)) << (row_column >> 3);
    *rounded_up_out = size;

    // 15 small buckets + row-based buckets
    let answer = row_column + 15 - 32;
    answer as usize
}

/// Find the next non-empty bucket starting from index
fn find_nonempty_bucket(index: usize) -> Option<usize> {
    let free_list_bits = THE_HEAP.free_list_bits.lock();

    // Check current word
    let word_idx = index >> 5;
    let mask = (1u32 << (31 - (index & 0x1f))) - 1;
    let mask = mask * 2 + 1;
    let masked = mask & free_list_bits[word_idx];

    if masked != 0 {
        let bit = masked.leading_zeros() as usize;
        return Some((index & !0x1f) + bit);
    }

    // Check subsequent words
    for idx in ((index + 32) & !0x1f)..NUMBER_OF_BUCKETS {
        if idx % 32 == 0 {
            let word = idx >> 5;
            if word < BUCKET_WORDS {
                let bits = free_list_bits[word];
                if bits != 0 {
                    let bit = bits.leading_zeros() as usize;
                    return Some(idx + bit);
                }
            }
        }
    }

    None
}

/// Set a free list bit
fn set_free_list_bit(index: usize) {
    let mut bits = THE_HEAP.free_list_bits.lock();
    bits[index >> 5] |= 1u32 << (31 - (index & 0x1f));
}

/// Clear a free list bit
fn clear_free_list_bit(index: usize) {
    let mut bits = THE_HEAP.free_list_bits.lock();
    bits[index >> 5] &= !(1u32 << (31 - (index & 0x1f)));
}

/// Tag a header pointer as free
fn tag_as_free(left: *mut Header) -> *mut Header {
    ((left as usize) | FREE_BIT) as *mut Header
}

/// Check if a header is tagged as free
unsafe fn is_tagged_as_free(header: *const Header) -> bool {
    ((*header).left as usize & FREE_BIT) != 0
}

/// Untag a header pointer
fn untag(left: *mut Header) -> *mut Header {
    ((left as usize) & !HEADER_LEFT_BIT_MASK) as *mut Header
}

/// Get the right header (next memory area)
unsafe fn right_header(header: *const Header) -> *mut Header {
    ((header as *const u8).add((*header).size)) as *mut Header
}

/// Fix the left pointer of a right header
unsafe fn fix_left_pointer(right: *mut Header, new_left: *mut Header) {
    let tag = (*right).left as usize & 1;
    (*right).left = ((new_left as usize & !1) | tag) as *mut Header;
}

/// Check if this is the start of an OS allocation
fn is_start_of_os_allocation(header: *mut Header) -> bool {
    unsafe { untag((*header).left).is_null() }
}

/// Check if this is the end of an OS allocation (right sentinel)
fn is_end_of_os_allocation(address: *mut Header) -> bool {
    unsafe { (*address).size == 0 }
}

/// Check if an address is page-aligned
fn is_page_aligned(addr: usize) -> bool {
    addr & (PAGE_SIZE - 1) == 0
}

/// Round up to alignment
fn round_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

/// Round down to alignment
fn round_down(value: usize, alignment: usize) -> usize {
    value & !(alignment - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_to_index_small() {
        let mut rounded = 0;
        let idx = size_to_index_allocating(8, &mut rounded);
        assert_eq!(idx, 0); // First bucket
        assert_eq!(rounded, 8);

        let idx = size_to_index_allocating(16, &mut rounded);
        assert_eq!(idx, 1);
        assert_eq!(rounded, 16);
    }

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(0, 8), 0);
        assert_eq!(round_up(1, 8), 8);
        assert_eq!(round_up(8, 8), 8);
        assert_eq!(round_up(9, 8), 16);
    }

    #[test]
    fn test_is_page_aligned() {
        assert!(is_page_aligned(0));
        assert!(is_page_aligned(4096));
        assert!(!is_page_aligned(1));
        assert!(!is_page_aligned(100));
    }

    #[test]
    fn test_constants() {
        assert_eq!(NUMBER_OF_BUCKETS, 1 + 15 + (22 - 7) * 8);
        assert_eq!(HEAP_LARGE_ALLOC_BYTES, 1 << 22);
    }
}
