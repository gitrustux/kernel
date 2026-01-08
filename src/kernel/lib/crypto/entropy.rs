// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Entropy Collector
//!
//! This module provides entropy collection for cryptographic purposes.
//! Different entropy sources can be registered and combined.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Maximum name length
const MAX_NAME_LEN: usize = 32;

/// Entropy VMO
static ENTROPY_VMO: Mutex<Option<*mut u8>> = Mutex::new(None);

/// Entropy loss flag
pub static ENTROPY_WAS_LOST: AtomicBool = AtomicBool::new(false);

/// Entropy collector
pub struct EntropyCollector {
    /// Collector name
    pub name: String,
    /// Entropy per 1000 bytes (in bits)
    pub entropy_per_1000_bytes: usize,
}

impl EntropyCollector {
    /// Create a new entropy collector
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the collector
    /// * `entropy_per_1000_bytes` - Entropy per 1000 bytes in bits
    ///
    /// # Returns
    ///
    /// New collector instance
    pub fn new(name: &str, entropy_per_1000_bytes: usize) -> Self {
        assert!(entropy_per_1000_bytes > 0, "Entropy rate must be positive");
        assert!(
            entropy_per_1000_bytes <= 8000,
            "Entropy rate must be <= 8000 bits/1000 bytes"
        );

        println!(
            "Entropy: Created collector '{}' ({} bits/1000 bytes)",
            name, entropy_per_1000_bytes
        );

        Self {
            name: name.to_string(),
            entropy_per_1000_bytes,
        }
    }

    /// Calculate bytes needed to get desired bits of entropy
    ///
    /// # Arguments
    ///
    /// * `bits` - Desired entropy in bits
    ///
    /// # Returns
    ///
    /// Number of bytes needed
    pub fn bytes_needed(&self, bits: usize) -> usize {
        // Avoid overflow and programming errors
        assert!(bits <= 1024 * 1024, "Requested too many bits");

        // Round up to ensure at least the requested amount of entropy
        (1000 * bits + self.entropy_per_1000_bytes - 1) / self.entropy_per_1000_bytes
    }

    /// Collect entropy
    ///
    /// # Arguments
    ///
    /// * `data` - Entropy data
    /// * `len` - Length of data
    pub fn collect(&self, data: &[u8]) {
        let _ = (data, self.entropy_per_1000_bytes);
        // TODO: Add entropy to the global entropy pool
    }
}

/// Hardware RNG collector
pub struct HwRngCollector {
    /// Base collector
    pub collector: EntropyCollector,
}

impl HwRngCollector {
    /// Create a new hardware RNG collector
    ///
    /// # Returns
    ///
    /// New collector instance
    pub fn new() -> Self {
        Self {
            collector: EntropyCollector::new("hw_rng", 8000), // Assume 8 bits/byte
        }
    }

    /// Read from hardware RNG
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to fill
    /// * `len` - Length of buffer
    ///
    /// # Returns
    ///
    /// Number of bytes read
    pub fn read(&self, buf: &mut [u8], len: usize) -> usize {
        // TODO: Implement actual hardware RNG reading
        let _ = (buf, len);
        println!("Entropy: HW RNG read requested ({} bytes)", len);
        0
    }
}

/// Jitter entropy collector
pub struct JitterEntropyCollector {
    /// Base collector
    pub collector: EntropyCollector,
}

impl JitterEntropyCollector {
    /// Create a new jitter entropy collector
    ///
    /// # Returns
    ///
    /// New collector instance
    pub fn new() -> Self {
        Self {
            collector: EntropyCollector::new("jitter", 1000), // Conservative estimate
        }
    }

    /// Read jitter entropy
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to fill
    /// * `len` - Length of buffer
    ///
    /// # Returns
    ///
    /// Number of bytes read
    pub fn read(&self, buf: &mut [u8], len: usize) -> usize {
        // TODO: Implement actual jitter entropy reading
        let _ = (buf, len);
        println!("Entropy: Jitter read requested ({} bytes)", len);
        0
    }
}

/// Initialize entropy collection system
///
/// This function sets up the entropy VMO and initializes collectors.
pub fn entropy_init() {
    println!("Entropy: Initializing entropy collection system");

    // TODO: Create entropy VMO
    let mut entropy_vmo = ENTROPY_VMO.lock();
    *entropy_vmo = None; // Will be set when VMO is created

    // TODO: Initialize collectors
}

/// Get entropy VMO
///
/// # Returns
///
/// Pointer to entropy VMO data, or None if not initialized
pub fn get_entropy_vmo() -> Option<*mut u8> {
    *ENTROPY_VMO.lock()
}

/// Set entropy VMO
///
/// # Arguments
///
/// * `vmo` - VMO pointer
pub fn set_entropy_vmo(vmo: *mut u8) {
    let mut entropy_vmo = ENTROPY_VMO.lock();
    *entropy_vmo = Some(vmo);
}

/// Mark entropy as lost
pub fn entropy_mark_lost() {
    ENTROPY_WAS_LOST.store(true, Ordering::Release);
    println!("Entropy: Entropy loss detected!");
}

/// Check if entropy was lost
pub fn entropy_was_lost() -> bool {
    ENTROPY_WAS_LOST.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector_creation() {
        let collector = EntropyCollector::new("test", 4000);
        assert_eq!(collector.name, "test");
        assert_eq!(collector.entropy_per_1000_bytes, 4000);
    }

    #[test]
    fn test_bytes_needed() {
        let collector = EntropyCollector::new("test", 1000); // 1 bit/byte
        assert_eq!(collector.bytes_needed(256), 256);
        assert_eq!(collector.bytes_needed(1024), 1024);
    }

    #[test]
    fn test_hw_rng_collector() {
        let collector = HwRngCollector::new();
        assert_eq!(collector.collector.name, "hw_rng");
        assert_eq!(collector.collector.entropy_per_1000_bytes, 8000);
    }

    #[test]
    fn test_jitter_collector() {
        let collector = JitterEntropyCollector::new();
        assert_eq!(collector.collector.name, "jitter");
        assert_eq!(collector.collector.entropy_per_1000_bytes, 1000);
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_NAME_LEN, 32);
    }
}
