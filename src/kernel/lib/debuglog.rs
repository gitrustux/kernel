// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Debug Log
//!
//! This module provides a circular buffer debug log implementation
//! for kernel diagnostics and logging.

#![no_std]

extern crate alloc;

use alloc::collections::LinkedList;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::sync::atomic::compiler_fence;
use spin::Mutex;

use crate::rustux::types::*;

/// Debug log size (128KB)
const DLOG_SIZE: usize = 128 * 1024;

/// Debug log mask (for circular buffer wrapping)
const DLOG_MASK: usize = DLOG_SIZE - 1;

/// Maximum data size per record (224 bytes)
const DLOG_MAX_DATA: usize = 224;

/// Minimum record size (header only)
const DLOG_MIN_RECORD: usize = 8;

/// Maximum record size (header + max data)
const DLOG_MAX_RECORD: usize = DLOG_MIN_RECORD + DLOG_MAX_DATA;

/// Assert that DLOG_SIZE is a power of two
const _: () = assert!(DLOG_SIZE & DLOG_MASK == 0, "DLOG_SIZE must be power of two");

/// Assert that DLOG_MAX_RECORD fits in DLOG_SIZE
const _: () = assert!(DLOG_MAX_RECORD <= DLOG_SIZE, "DLOG_MAX_RECORD too large");

/// Assert that DLOG_MAX_RECORD is 4-byte aligned
const _: () = assert!(DLOG_MAX_RECORD & 3 == 0, "DLOG_MAX_RECORD must be 4-byte aligned");

/// Debug log header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DlogHeader {
    /// Header word containing record info
    pub header: u32,
    /// Data length in bytes
    pub datalen: u16,
    /// Flags
    pub flags: u16,
    /// Timestamp
    pub timestamp: u64,
    /// Process ID
    pub pid: u64,
    /// Thread ID
    pub tid: u64,
}

// Header manipulation macros
const DLOG_HDR_FIFOLEN_SHIFT: u32 = 18;
const DLOG_HDR_FIFOLEN_MASK: u32 = 0x3FFF << DLOG_HDR_FIFOLEN_SHIFT;
const DLOG_HDR_READLEN_SHIFT: u32 = 2;
const DLOG_HDR_READLEN_MASK: u32 = (DLOG_MAX_RECORD as u32) << DLOG_HDR_READLEN_SHIFT;

/// Set FIFO length in header word
#[inline]
const fn dlog_hdr_set_fifolen(fifolen: u32) -> u32 {
    fifolen << DLOG_HDR_FIFOLEN_SHIFT
}

/// Get FIFO length from header word
#[inline]
pub const fn dlog_hdr_get_fifolen(header: u32) -> u32 {
    (header & DLOG_HDR_FIFOLEN_MASK) >> DLOG_HDR_FIFOLEN_SHIFT
}

/// Get read length from header word
#[inline]
pub const fn dlog_hdr_get_readlen(header: u32) -> usize {
    ((header & DLOG_HDR_READLEN_MASK) >> DLOG_HDR_READLEN_SHIFT) as usize
}

/// Create header word
#[inline]
pub const fn dlog_hdr_set(fifolen: u32, readlen: usize) -> u32 {
    dlog_hdr_set_fifolen(fifolen) | ((readlen as u32) << DLOG_HDR_READLEN_SHIFT)
}

/// Debug log reader
pub struct DlogReader {
    /// Pointer to the log
    log: *const Dlog,
    /// Current read position
    tail: u64,
    /// Notification callback
    notify: Option<fn(*mut u8)>,
    /// Cookie for notification callback
    cookie: *mut u8,
}

unsafe impl Send for DlogReader {}

/// Debug log structure
pub struct Dlog {
    /// Lock for log access
    lock: Mutex<()>,
    /// Head pointer (next write position)
    head: AtomicU64,
    /// Tail pointer (oldest record)
    tail: AtomicU64,
    /// Log data buffer
    data: [u8; DLOG_SIZE],
    /// Panic mode flag
    panic: AtomicBool,
    /// List of readers
    readers: Mutex<LinkedList<Arc<DlogReader>>>,
}

/// Global debug log instance
static mut DLOG_DATA: [u8; DLOG_SIZE] = [0; DLOG_SIZE];

pub static DLOG: Dlog = Dlog::new();

impl Dlog {
    pub const fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            data: [0; DLOG_SIZE],
            panic: AtomicBool::new(false),
            readers: Mutex::new(LinkedList::new()),
        }
    }

    /// Write data to the debug log
    pub fn write(&self, flags: u16, data: &[u8]) -> Result<(), i32> {
        if data.len() > DLOG_MAX_DATA {
            return Err(-1); // ZX_ERR_OUT_OF_RANGE
        }

        if self.panic.load(Ordering::Acquire) {
            return Err(-2); // ZX_ERR_BAD_STATE
        }

        // Calculate wire size (4-byte aligned)
        let wiresize = DLOG_MIN_RECORD + ((data.len() + 3) & !3);

        // Prepare header before taking lock
        let header = DlogHeader {
            header: dlog_hdr_set(wiresize as u32, DLOG_MIN_RECORD + data.len()),
            datalen: data.len() as u16,
            flags,
            timestamp: 0, // TODO: current_time()
            pid: 0,      // TODO: get_current_thread()->user_pid
            tid: 0,      // TODO: get_current_thread()->user_tid
        };

        let _guard = self.lock.lock();

        // Discard old records until we have space
        let head = self.head.load(Ordering::Acquire);
        let mut tail = self.tail.load(Ordering::Acquire);

        while (head - tail) > (DLOG_SIZE - wiresize) as u64 {
            let offset = (tail as usize) & DLOG_MASK;
            let header_word = u32::from_le_bytes(
                self.data[offset..offset + 4]
                    .try_into()
                    .unwrap()
            );
            tail += dlog_hdr_get_fifolen(header_word) as u64;
            self.tail.store(tail, Ordering::Release);
        }

        // Write the record
        let offset = (head as usize) & DLOG_MASK;
        let header_bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const DlogHeader as *const u8,
                core::mem::size_of::<DlogHeader>()
            )
        };

        if DLOG_SIZE - offset >= wiresize {
            // Everything fits in one write
            self.data[offset..offset + header_bytes.len()].copy_from_slice(header_bytes);
            self.data[offset + header_bytes.len()..offset + header_bytes.len() + data.len()]
                .copy_from_slice(data);
        } else if DLOG_SIZE - offset < header_bytes.len() {
            // Header wraps around
            let remaining = DLOG_SIZE - offset;
            self.data[offset..].copy_from_slice(&header_bytes[..remaining]);
            self.data[..header_bytes.len() - remaining].copy_from_slice(&header_bytes[remaining..]);
            self.data[header_bytes.len() - remaining..].copy_from_slice(data);
        } else {
            // Data wraps around
            let fifospace = DLOG_SIZE - offset - header_bytes.len();
            self.data[offset..offset + header_bytes.len()].copy_from_slice(header_bytes);
            let data_offset = offset + header_bytes.len();
            self.data[data_offset..].copy_from_slice(&data[..fifospace]);
            self.data[..data.len() - fifospace].copy_from_slice(&data[fifospace..]);
        }

        self.head.store(head + wiresize as u64, Ordering::Release);

        // TODO: Notify readers
        compiler_fence(Ordering::Release);

        Ok(())
    }

    /// Read from the debug log
    pub fn read(&self, reader: &mut DlogReader, buffer: &mut [u8]) -> Result<usize, i32> {
        if buffer.len() < DLOG_MAX_RECORD {
            return Err(-1); // ZX_ERR_BUFFER_TOO_SMALL
        }

        let _guard = self.lock.lock();

        let head = self.head.load(Ordering::Acquire);
        let mut rtail = reader.tail;

        // Check if reader has been lapped
        if (head - self.tail.load(Ordering::Acquire)) < (head - rtail) {
            rtail = self.tail.load(Ordering::Acquire);
            reader.tail = rtail;
        }

        if rtail == head {
            return Err(-2); // ZX_ERR_SHOULD_WAIT
        }

        let offset = (rtail as usize) & DLOG_MASK;
        let header_word = u32::from_le_bytes(
            self.data[offset..offset + 4]
                .try_into()
                .unwrap()
        );

        let actual = dlog_hdr_get_readlen(header_word);

        if DLOG_SIZE - offset >= actual {
            buffer[..actual].copy_from_slice(&self.data[offset..offset + actual]);
        } else {
            let fifospace = DLOG_SIZE - offset;
            buffer[..fifospace].copy_from_slice(&self.data[offset..]);
            buffer[fifospace..actual].copy_from_slice(&self.data[..actual - fifospace]);
        }

        reader.tail += dlog_hdr_get_fifolen(header_word) as u64;

        Ok(actual)
    }

    /// Initialize a reader
    pub fn reader_init(&self, notify: Option<fn(*mut u8)>, cookie: *mut u8) -> Arc<DlogReader> {
        let _guard = self.lock.lock();
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        let reader = Arc::new(DlogReader {
            log: self as *const Dlog,
            tail,
            notify,
            cookie,
        });

        // Notify if there's already data
        if tail != head {
            if let Some(notify_fn) = notify {
                notify_fn(cookie);
            }
        }

        self.readers.lock().push_back(reader.clone());

        reader
    }

    /// Enable panic mode (bluescreen)
    pub fn set_panic(&self) {
        self.panic.store(true, Ordering::Release);
    }
}

/// Write to debug log
pub fn dlog_write(flags: u16, data: &[u8]) -> Result<(), i32> {
    DLOG.write(flags, data)
}

/// Write a formatted string to the debug log
pub fn dlog_printf(flags: u16, format: &str) {
    // TODO: Implement proper printf formatting
    let _ = flags;
    let _ = DLOG.write(0, format.as_bytes());
}

/// Initialize debug log bypass for early console output
pub fn dlog_bypass_init_early() {
    // TODO: Read compile-time switch
}

/// Initialize debug log bypass from cmdline
pub fn dlog_bypass_init() {
    // TODO: Read kernel.bypass-debuglog cmdline option
}

/// Check if debug log bypass is enabled
pub fn dlog_bypass() -> bool {
    false
}

/// Write to serial console (bypassing debug log)
pub fn dlog_serial_write(data: &[u8]) {
    if dlog_bypass() {
        // Direct to serial with spinlock
        // TODO: __kernel_serial_write
        let _ = data;
    } else {
        // Use mutex for proper synchronization
        // TODO: platform_dputs_thread
        let _ = data;
    }
}

/// Initialize bluescreen (panic) mode
pub fn dlog_bluescreen_init() {
    DLOG.set_panic();

    // TODO: udisplay_bind_gfxconsole
    // TODO: Print panic information
}

/// Shutdown debug log threads
pub fn dlog_shutdown() {
    // TODO: Implement thread shutdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_macros() {
        // Test that header macros work correctly
        let fifolen = 256u32;
        let readlen = 100usize;
        let header = dlog_hdr_set(fifolen, readlen);

        assert_eq!(dlog_hdr_get_fifolen(header), fifolen);
        assert_eq!(dlog_hdr_get_readlen(header), readlen);
    }

    #[test]
    fn test_write_read_cycle() {
        let data = b"Test log message";
        let flags = 0x01;

        // Write
        let result = DLOG.write(flags, data);
        assert!(result.is_ok());

        // Read
        let reader = DLOG.reader_init(None, core::ptr::null_mut());
        let mut buffer = [0u8; DLOG_MAX_RECORD];
        let mut reader_clone = DlogReader {
            log: core::ptr::null(),
            tail: reader.tail,
            notify: None,
            cookie: core::ptr::null_mut(),
        };

        let result = DLOG.read(&mut reader_clone, &mut buffer);
        assert!(result.is_ok());

        // Verify data (note: header is included in buffer)
        let actual_len = result.unwrap();
        assert!(actual_len >= data.len());
    }
}
