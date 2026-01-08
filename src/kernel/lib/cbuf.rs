// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Circular Buffer
//!
//! This module provides a lock-free circular buffer implementation
//! for inter-thread communication.

#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use core::sync::atomic::compiler_fence;
use spin::Mutex;

use crate::rustux::types::*;

/// Circular buffer structure
pub struct CBuf {
    /// Buffer data
    buf: Box<[u8]>,
    /// Head pointer (write position)
    head: AtomicU32,
    /// Tail pointer (read position)
    tail: AtomicU32,
    /// Buffer length (power of 2)
    len_pow2: u32,
    /// Event for signaling data availability
    event: Arc<Mutex<bool>>,
    /// Lock for synchronization
    lock: Mutex<()>,
}

impl CBuf {
    /// Create a new circular buffer with the specified size
    ///
    /// The size must be a power of 2.
    pub fn new(len: usize) -> Option<Self> {
        // Check if length is a power of 2
        if len == 0 || (len & (len - 1)) != 0 {
            return None;
        }

        let len_pow2 = len.ilog2() as u32;
        let buf = vec![0u8; len].into_boxed_slice();

        Some(Self {
            buf,
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            len_pow2,
            event: Arc::new(Mutex::new(false)),
            lock: Mutex::new(()),
        })
    }

    /// Increment a pointer with wrap-around
    #[inline]
    fn inc_pointer(&self, ptr: u32, inc: u32) -> u32 {
        (ptr + inc) & (self.len_pow2 as u32 - 1)
    }

    /// Calculate available space in the buffer
    pub fn space_avail(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let consumed = head.wrapping_sub(tail) & ((1u32 << self.len_pow2) - 1);
        (1u32 << self.len_pow2) as usize - consumed as usize - 1
    }

    /// Calculate available data to read
    pub fn data_avail(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head.wrapping_sub(tail) as usize & ((1usize << self.len_pow2) - 1)
    }

    /// Write a single character to the buffer
    pub fn write_char(&self, c: u8) -> usize {
        let mut ret = 0;

        {
            let _guard = self.lock.lock();

            if self.space_avail() > 0 {
                let head = self.head.load(Ordering::Acquire) as usize;
                self.buf[head] = c;
                self.head.store(self.inc_pointer(head as u32, 1), Ordering::Release);
                ret = 1;
            }
        }

        if ret > 0 {
            *self.event.lock() = true;
        }

        ret
    }

    /// Write multiple bytes to the buffer
    pub fn write(&self, data: &[u8]) -> usize {
        let avail = self.space_avail();
        let to_write = data.len().min(avail);

        if to_write == 0 {
            return 0;
        }

        {
            let _guard = self.lock.lock();

            let mut head = self.head.load(Ordering::Acquire) as usize;
            let buf_len = self.buf.len();

            for &byte in data.iter().take(to_write) {
                self.buf[head] = byte;
                head = self.inc_pointer(head as u32, 1) as usize;
            }

            self.head.store(head as u32, Ordering::Release);
        }

        *self.event.lock() = true;
        to_write
    }

    /// Read a single character from the buffer
    pub fn read_char(&self, block: bool) -> Option<u8> {
        loop {
            {
                let _guard = self.lock.lock();

                let head = self.head.load(Ordering::Acquire);
                let tail = self.tail.load(Ordering::Acquire);

                if tail != head {
                    let data = self.buf[tail as usize];
                    let new_tail = self.inc_pointer(tail, 1);
                    self.tail.store(new_tail, Ordering::Release);

                    if new_tail == head {
                        *self.event.lock() = false;
                    }

                    return Some(data);
                }
            }

            if !block {
                return None;
            }

            // Wait for data
            // TODO: Proper event waiting
            compiler_fence(Ordering::Acquire);
        }
    }

    /// Read multiple bytes from the buffer
    pub fn read(&self, buffer: &mut [u8], block: bool) -> usize {
        let mut read_count = 0;

        {
            let _guard = self.lock.lock();

            let head = self.head.load(Ordering::Acquire) as usize;
            let mut tail = self.tail.load(Ordering::Acquire) as usize;

            for slot in buffer.iter_mut() {
                if tail == head {
                    break;
                }

                *slot = self.buf[tail];
                tail = self.inc_pointer(tail as u32, 1) as usize;
                read_count += 1;
            }

            self.tail.store(tail as u32, Ordering::Release);

            if tail == head {
                *self.event.lock() = false;
            }
        }

        if block && read_count == 0 && !buffer.is_empty() {
            // TODO: Proper blocking wait
            return 0;
        }

        read_count
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head == tail
    }

    /// Check if the buffer is full
    pub fn is_full(&self) -> bool {
        self.space_avail() == 0
    }

    /// Clear the buffer
    pub fn clear(&self) {
        let _guard = self.lock.lock();
        self.tail.store(self.head.load(Ordering::Acquire), Ordering::Release);
        *self.event.lock() = false;
    }

    /// Get the buffer capacity
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbuf_creation() {
        // Valid sizes (powers of 2)
        assert!(CBuf::new(16).is_some());
        assert!(CBuf::new(256).is_some());
        assert!(CBuf::new(1024).is_some());

        // Invalid sizes
        assert!(CBuf::new(0).is_none());
        assert!(CBuf::new(15).is_none());
        assert!(CBuf::new(100).is_none());
    }

    #[test]
    fn test_cbuf_write_read_char() {
        let cbuf = CBuf::new(16).unwrap();

        assert_eq!(cbuf.write_char(b'A'), 1);
        assert_eq!(cbuf.read_char(false), Some(b'A'));
        assert_eq!(cbuf.read_char(false), None);
    }

    #[test]
    fn test_cbuf_write_read() {
        let cbuf = CBuf::new(16).unwrap();

        let data = b"Hello";
        assert_eq!(cbuf.write(data), 5);

        let mut buffer = [0u8; 16];
        assert_eq!(cbuf.read(&mut buffer, false), 5);
        assert_eq!(&buffer[..5], b"Hello");
    }

    #[test]
    fn test_cbuf_wraparound() {
        let cbuf = CBuf::new(8).unwrap();

        // Fill the buffer
        let data = b"1234567";
        assert_eq!(cbuf.write(data), 7);

        // Read some
        let mut buffer = [0u8; 8];
        assert_eq!(cbuf.read(&mut buffer, false), 7);

        // Write more (should wrap)
        let more = b"ABCDEFG";
        assert_eq!(cbuf.write(more), 7);

        // Read all
        assert_eq!(cbuf.read(&mut buffer, false), 7);
        assert_eq!(&buffer[..7], more);
    }

    #[test]
    fn test_cbuf_space_avail() {
        let cbuf = CBuf::new(16).unwrap();

        // Empty buffer has capacity - 1 available
        assert_eq!(cbuf.space_avail(), 15);

        cbuf.write_char(b'A');
        assert_eq!(cbuf.space_avail(), 14);

        // Fill to capacity
        for _ in 0..14 {
            cbuf.write_char(b'X');
        }
        assert_eq!(cbuf.space_avail(), 0);
    }

    #[test]
    fn test_cbuf_empty_full() {
        let cbuf = CBuf::new(8).unwrap();

        assert!(cbuf.is_empty());
        assert!(!cbuf.is_full());

        // Fill buffer (capacity - 1 due to circular buffer design)
        for _ in 0..7 {
            cbuf.write_char(b'X');
        }

        assert!(cbuf.is_full());
        assert!(!cbuf.is_empty());
    }

    #[test]
    fn test_cbuf_clear() {
        let cbuf = CBuf::new(16).unwrap();

        cbuf.write(b"Hello World");
        assert!(!cbuf.is_empty());

        cbuf.clear();
        assert!(cbuf.is_empty());
    }
}
