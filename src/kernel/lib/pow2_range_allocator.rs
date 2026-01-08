// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Power-of-2 Range Allocator
//!
//! This module provides a power-of-2 range allocator that allocates
//! aligned ranges of power-of-2 size. It's commonly used for managing
//! resources like IRQs, IO ports, or memory ranges.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Block bookkeeping structure
#[derive(Debug, Clone)]
struct P2raBlock {
    /// Bucket index (log2 of size)
    bucket: u32,
    /// Start address/ID of this block
    start: u32,
}

/// Range structure
#[derive(Debug, Clone)]
struct P2raRange {
    /// Start of the range
    start: u32,
    /// Length of the range
    len: u32,
}

/// Power-of-2 range allocator state
pub struct P2raState {
    /// Number of buckets (log2(max_alloc_size) + 1)
    bucket_count: u32,
    /// Free blocks buckets (indexed by log2(size))
    free_blocks: Vec<Vec<P2raBlock>>,
    /// Allocated blocks (for tracking and debugging)
    allocated_blocks: Vec<P2raBlock>,
    /// Added ranges
    ranges: Vec<P2raRange>,
    /// Unused blocks (for recycling)
    unused_blocks: Vec<P2raBlock>,
    /// Maximum allocation size
    max_alloc_size: u32,
}

impl P2raState {
    /// Initialize a new power-of-2 range allocator
    ///
    /// # Arguments
    ///
    /// * `max_alloc_size` - Maximum allocation size (must be power of 2)
    ///
    /// # Returns
    ///
    /// Ok(state) on success, Err(status) on failure
    pub fn new(max_alloc_size: u32) -> Result<Self, i32> {
        if max_alloc_size == 0 || !max_alloc_size.is_power_of_two() {
            println!("max_alloc_size ({}) is not an integer power of two!", max_alloc_size);
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }

        let bucket_count = max_alloc_size.trailing_zeros() as u32 + 1;

        println!("P2RA: Initializing with {} buckets (max size: {})", bucket_count, max_alloc_size);

        let mut free_blocks = Vec::with_capacity(bucket_count as usize);
        for _ in 0..bucket_count {
            free_blocks.push(Vec::new());
        }

        Ok(Self {
            bucket_count,
            free_blocks,
            allocated_blocks: Vec::new(),
            ranges: Vec::new(),
            unused_blocks: Vec::new(),
            max_alloc_size,
        })
    }

    /// Add a range to the allocator
    ///
    /// # Arguments
    ///
    /// * `range_start` - Start of the range
    /// * `range_len` - Length of the range
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err(status) on failure
    pub fn add_range(&mut self, range_start: u32, range_len: u32) -> Result<(), i32> {
        if range_len == 0 {
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }
        if range_start.checked_add(range_len).is_none() {
            return Err(-1); // Overflow
        }

        println!("P2RA: Adding range [{:#x}, {:#x}]", range_start, range_start + range_len - 1);

        // Check for overlap with existing ranges
        for range in &self.ranges {
            let range_end = range.start + range.len;
            let new_end = range_start + range_len;

            if (range.start >= range_start && range.start < new_end)
                || (range_start >= range.start && range_start < range_end)
            {
                println!(
                    "Range [{:#x}, {:#x}] overlaps with existing range [{:#x}, {:#x}]",
                    range_start, new_end - 1, range.start, range_end - 1
                );
                return Err(-2); // ZX_ERR_ALREADY_EXISTS
            }
        }

        // Add the range
        self.ranges.push(P2raRange {
            start: range_start,
            len: range_len,
        });

        // Break the range into power-of-2 aligned chunks
        let mut range_start = range_start;
        let mut range_len = range_len;
        let max_bucket = self.bucket_count - 1;
        let mut bucket = max_bucket;
        let mut csize = 1u32 << bucket;
        let max_csize = csize;

        while range_len > 0 {
            // Shrink chunk size until aligned and fits
            let mut shrunk = false;
            while (range_start & (csize - 1)) != 0 || range_len < csize {
                csize >>= 1;
                bucket -= 1;
                shrunk = true;
            }

            // Try to grow chunk size if possible
            if !shrunk {
                let mut tmp = csize << 1;
                while tmp <= max_csize
                    && tmp <= range_len
                    && (range_start & (tmp - 1)) == 0
                {
                    bucket += 1;
                    csize = tmp;
                    tmp <<= 1;
                }
            }

            // Add the block to free list
            let block = P2raBlock {
                bucket,
                start: range_start,
            };
            self.return_free_block_internal(block, false);

            range_start += csize;
            range_len -= csize;
        }

        Ok(())
    }

    /// Allocate a range
    ///
    /// # Arguments
    ///
    /// * `size` - Size to allocate (must be power of 2)
    ///
    /// # Returns
    ///
    /// Ok(start) on success, Err(status) on failure
    pub fn allocate(&mut self, size: u32) -> Result<u32, i32> {
        if size == 0 || !size.is_power_of_two() {
            println!("Size ({}) is not an integer power of 2.", size);
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }

        let orig_bucket = size.trailing_zeros() as u32;
        let mut bucket = orig_bucket;

        if bucket >= self.bucket_count {
            println!(
                "Invalid size ({}). Valid sizes are integer powers of 2 from [1, {}]",
                size, 1u32 << (self.bucket_count - 1)
            );
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }

        // Find the smallest chunk that can hold the allocation
        let mut block: Option<P2raBlock> = None;

        while bucket < self.bucket_count {
            if let Some(b) = self.free_blocks[bucket as usize].pop() {
                block = Some(b);
                break;
            }
            bucket += 1;
        }

        let mut block = block.ok_or(-3)?; // ZX_ERR_NO_RESOURCES

        // Split the block if necessary
        while bucket > orig_bucket {
            let split_start = block.start + (1u32 << (bucket - 1));

            let split_block = P2raBlock {
                bucket: bucket - 1,
                start: split_start,
            };

            self.return_free_block_internal(split_block, false);

            block.bucket = bucket - 1;
            bucket -= 1;
        }

        // Mark as allocated
        self.allocated_blocks.push(block);
        Ok(block.start)
    }

    /// Free a range
    ///
    /// # Arguments
    ///
    /// * `range_start` - Start of range to free
    /// * `size` - Size of range (must be power of 2)
    pub fn free(&mut self, range_start: u32, size: u32) {
        if size == 0 || !size.is_power_of_two() {
            return;
        }

        let bucket = size.trailing_zeros() as u32;

        // Find and remove from allocated blocks
        let pos = self
            .allocated_blocks
            .iter()
            .position(|b| b.start == range_start && b.bucket == bucket);

        if let Some(pos) = pos {
            let block = self.allocated_blocks.remove(pos);
            self.return_free_block_internal(block, true);
        }
    }

    /// Return a block to the free list (internal)
    fn return_free_block_internal(&mut self, mut block: P2raBlock, merge_allowed: bool) {
        assert!(block.bucket < self.bucket_count);
        assert!((block.start & ((1u32 << block.bucket) - 1)) == 0);

        // Insert into free bucket (sorted by start)
        let bucket = block.bucket as usize;
        let block_len = 1u32 << block.bucket;

        // Find insertion point
        let pos = self.free_blocks[bucket]
            .iter()
            .position(|b| b.start > block.start)
            .unwrap_or(self.free_blocks[bucket].len());

        self.free_blocks[bucket].insert(pos, block);

        // Don't merge blocks in the largest bucket
        if block.bucket + 1 == self.bucket_count {
            return;
        }

        // Check for merge opportunities
        if (block.start & ((block_len << 1) - 1)) != 0 {
            // Odd alignment - might be second block of merge pair
            // Check previous block
            if pos > 0 {
                let first = &self.free_blocks[bucket][pos - 1];
                let first_len = 1u32 << first.bucket;
                if first.start + first_len == block.start {
                    // Found a merge pair!
                    if merge_allowed {
                        let first = self.free_blocks[bucket].remove(pos - 1);
                        let second = self.free_blocks[bucket].remove(pos - 1);

                        self.unused_blocks.push(second);

                        block.bucket += 1;
                        self.return_free_block_internal(block, merge_allowed);
                    }
                }
            }
        } else {
            // Even alignment - might be first block of merge pair
            // Check next block
            if pos + 1 < self.free_blocks[bucket].len() {
                let second = &self.free_blocks[bucket][pos + 1];
                if block.start + block_len == second.start {
                    // Found a merge pair!
                    if merge_allowed {
                        let second = self.free_blocks[bucket].remove(pos + 1);
                        let first = self.free_blocks[bucket].remove(pos);

                        self.unused_blocks.push(second);

                        block.bucket += 1;
                        self.return_free_block_internal(block, merge_allowed);
                    }
                }
            }
        }
    }

    /// Get an unused block (from pool or allocate new)
    fn get_unused_block(&mut self) -> P2raBlock {
        self.unused_blocks
            .pop()
            .unwrap_or_else(|| P2raBlock {
                bucket: 0,
                start: 0,
            })
    }

    /// Dump allocator state for debugging
    pub fn dump(&self) {
        println!("P2RA State:");
        println!("  Bucket count: {}", self.bucket_count);
        println!("  Max alloc size: {}", self.max_alloc_size);
        println!("  Ranges: {}", self.ranges.len());
        println!("  Allocated blocks: {}", self.allocated_blocks.len());

        for (i, bucket) in self.free_blocks.iter().enumerate() {
            if !bucket.is_empty() {
                println!("  Free bucket {} (size {}): {} blocks", i, 1u32 << i, bucket.len());
            }
        }
    }
}

/// Check if a number is a power of 2
pub fn ispow2(n: u32) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

/// Get floor of log2
pub fn log2_uint_floor(n: u32) -> u32 {
    n.trailing_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2ra_creation() {
        let state = P2raState::new(256);
        assert!(state.is_ok());

        let state = state.unwrap();
        assert_eq!(state.bucket_count, 9); // log2(256) + 1
        assert_eq!(state.max_alloc_size, 256);
    }

    #[test]
    fn test_p2ra_invalid_max_size() {
        assert!(P2raState::new(0).is_err());
        assert!(P2raState::new(100).is_err()); // Not power of 2
    }

    #[test]
    fn test_p2ra_add_range() {
        let mut state = P2raState::new(256).unwrap();
        assert!(state.add_range(0, 256).is_ok());
    }

    #[test]
    fn test_p2ra_allocate() {
        let mut state = P2raState::new(256).unwrap();
        state.add_range(0, 256).unwrap();

        let addr = state.allocate(64).unwrap();
        assert_eq!(addr, 0);

        let addr = state.allocate(64).unwrap();
        assert_eq!(addr, 64);
    }

    #[test]
    fn test_p2ra_free() {
        let mut state = P2raState::new(256).unwrap();
        state.add_range(0, 256).unwrap();

        let addr = state.allocate(64).unwrap();
        state.free(addr, 64);

        // Should be able to allocate again
        let addr2 = state.allocate(64).unwrap();
        assert_eq!(addr2, addr);
    }

    #[test]
    fn test_ispow2() {
        assert!(ispow2(1));
        assert!(ispow2(2));
        assert!(ispow2(4));
        assert!(ispow2(256));
        assert!(!ispow2(0));
        assert!(!ispow2(3));
        assert!(!ispow2(100));
    }

    #[test]
    fn test_log2_uint_floor() {
        assert_eq!(log2_uint_floor(1), 0);
        assert_eq!(log2_uint_floor(2), 1);
        assert_eq!(log2_uint_floor(4), 2);
        assert_eq!(log2_uint_floor(8), 3);
        assert_eq!(log2_uint_floor(256), 8);
    }
}
