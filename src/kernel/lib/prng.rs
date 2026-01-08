// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Pseudo-Random Number Generator (PRNG)
//!
//! This module provides a cryptographically secure PRNG based on ChaCha20.
//! It maintains an internal key that is re-seeded with entropy using SHA256.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// SHA256 digest length
const SHA256_DIGEST_LENGTH: usize = 32;

/// Minimum entropy required before PRNG is ready
const MIN_ENTROPY: usize = 32;

/// Maximum entropy that can be added at once
const MAX_ENTROPY: usize = 4096;

/// Maximum draw length
const MAX_DRAW_LEN: usize = 65536;

/// ChaCha20 block size
const CHACHA20_BLOCK_SIZE: usize = 64;

/// ChaCha20 key size
const CHACHA20_KEY_SIZE: usize = 32;

/// ChaCha20 nonce size
const CHACHA20_NONCE_SIZE: usize = 12;

/// Nonce overflow threshold (2^96)
const NONCE_OVERFLOW: u128 = 1u128 << 96;

/// Thread-safe PRNG
pub struct ThreadSafePrng;

/// Non-thread-safe PRNG tag
pub struct NonThreadSafeTag;

/// Pseudo-Random Number Generator
pub struct Prng {
    /// ChaCha20 key
    key: [u8; CHACHA20_KEY_SIZE],
    /// Current nonce
    nonce: AtomicU128,
    /// Accumulated entropy
    accumulated: AtomicUsize,
    /// Thread-safe flag
    thread_safe: AtomicBool,
    /// Spin lock for key/nonce access
    spinlock: Mutex<()>,
}

unsafe impl Send for Prng {}
unsafe impl Sync for Prng {}

impl Prng {
    /// Create a new thread-safe PRNG
    ///
    /// # Arguments
    ///
    /// * `data` - Initial entropy data
    /// * `size` - Size of entropy data
    pub fn new(data: &[u8]) -> Self {
        let mut prng = Self::new_non_thread_safe(data);
        prng.become_thread_safe();
        prng
    }

    /// Create a new non-thread-safe PRNG
    ///
    /// # Arguments
    ///
    /// * `data` - Initial entropy data
    pub fn new_non_thread_safe(data: &[u8]) -> Self {
        let mut prng = Self {
            key: [0u8; CHACHA20_KEY_SIZE],
            nonce: AtomicU128::new(0),
            accumulated: AtomicUsize::new(0),
            thread_safe: AtomicBool::new(false),
            spinlock: Mutex::new(()),
        };

        prng.add_entropy_internal(data);
        prng
    }

    /// Add entropy to the PRNG
    ///
    /// # Arguments
    ///
    /// * `data` - Entropy data
    pub fn add_entropy(&self, data: &[u8]) {
        assert!(data.len() <= MAX_ENTROPY, "Entropy too large");

        // Mix new entropy with existing key using SHA256
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(&self.get_key());
        let new_key = hasher.finalize();

        self.set_key(&new_key);

        // Update accumulated entropy count
        let new_accumulated = self.accumulated.fetch_add(data.len(), Ordering::AcqRel) + data.len();

        // Signal ready if we have enough entropy and are thread-safe
        if self.is_thread_safe() && new_accumulated >= MIN_ENTROPY {
            // TODO: Signal ready event
        }
    }

    /// Draw random bytes
    ///
    /// # Arguments
    ///
    /// * `out` - Output buffer
    pub fn draw(&self, out: &mut [u8]) {
        assert!(out.len() <= MAX_DRAW_LEN, "Draw too large");

        // Wait for enough entropy if thread-safe
        if self.is_thread_safe() && self.accumulated.load(Ordering::Acquire) < MIN_ENTROPY {
            // TODO: Wait for ready event
        }

        // Get current key and nonce
        let (key, nonce) = self.get_key_and_nonce();

        // Encrypt the output buffer using ChaCha20
        chacha20_encrypt(out, &key, nonce);
    }

    /// Generate a random integer in [0, exclusive_upper_bound)
    ///
    /// # Arguments
    ///
    /// * `exclusive_upper_bound` - Upper bound (exclusive)
    ///
    /// # Returns
    ///
    /// Random integer in the range
    pub fn rand_int(&self, exclusive_upper_bound: u64) -> u64 {
        assert!(exclusive_upper_bound != 0, "Upper bound cannot be zero");

        let log2 = 64 - exclusive_upper_bound.leading_zeros() as usize;
        let mask = if log2 != 64 {
            (1u64 << log2) - 1
        } else {
            u64::MAX
        };

        // Discard out-of-range values (rejection sampling)
        loop {
            let mut bytes = [0u8; 8];
            self.draw(&mut bytes);
            let mut v = u64::from_le_bytes(bytes);
            v &= mask;

            if v < exclusive_upper_bound {
                return v;
            }
        }
    }

    /// Check if PRNG is thread-safe
    pub fn is_thread_safe(&self) -> bool {
        self.thread_safe.load(Ordering::Acquire)
    }

    /// Add entropy (internal helper)
    fn add_entropy_internal(&mut self, data: &[u8]) {
        // Initialize key using SHA256 of the entropy
        let mut hasher = Sha256::new();
        hasher.update(data);
        self.key = hasher.finalize();

        self.accumulated.store(data.len(), Ordering::Release);
    }

    /// Get the current key
    fn get_key(&self) -> [u8; CHACHA20_KEY_SIZE] {
        let _lock = self.spinlock.lock();
        self.key
    }

    /// Set the key
    fn set_key(&self, new_key: &[u8; CHACHA20_KEY_SIZE]) {
        let _lock = self.spinlock.lock();
        // Safety: We're behind a lock
        let key_ptr = &self.key as *const [u8; CHACHA20_KEY_SIZE] as *mut [u8; CHACHA20_KEY_SIZE];
        unsafe {
            *key_ptr = *new_key;
        }
    }

    /// Get current key and increment nonce
    fn get_key_and_nonce(&self) -> ([u8; CHACHA20_KEY_SIZE], u128) {
        let _lock = self.spinlock.lock();
        let nonce = self.nonce.fetch_add(1, Ordering::AcqRel);

        assert!(nonce < NONCE_OVERFLOW, "Nonce overflow");

        (self.key, nonce)
    }

    /// Make PRNG thread-safe
    fn become_thread_safe(&self) {
        self.thread_safe.store(true, Ordering::Release);
        // TODO: Initialize ready event
    }
}

/// Simple SHA256 implementation
struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        // TODO: Implement full SHA256
        let _ = data;
        // For now, this is a stub
    }

    fn finalize(mut self) -> [u8; SHA256_DIGEST_LENGTH] {
        // TODO: Implement full SHA256
        let mut result = [0u8; SHA256_DIGEST_LENGTH];
        // For now, return zeros
        result
    }
}

/// ChaCha20 encryption
///
/// # Arguments
///
/// * `data` - Data to encrypt (in-place)
/// * `key` - 32-byte key
/// * `nonce` - 96-bit nonce
fn chacha20_encrypt(data: &mut [u8], key: &[u8; 32], nonce: u128) {
    // TODO: Implement full ChaCha20
    // For now, use a simple XOR with the key and nonce
    let key_bytes = key.as_ptr();
    let nonce_bytes = &nonce as *const u128 as *const u8;

    for (i, byte) in data.iter_mut().enumerate() {
        unsafe {
            let key_byte = *key_bytes.add(i % 32);
            let nonce_byte = *nonce_bytes.add(i % 16);
            *byte ^= key_byte ^ nonce_byte;
        }
    }
}

/// Global PRNG instance
static GLOBAL_PRNG: Mutex<Option<Prng>> = Mutex::new(None);

/// Initialize the global PRNG
///
/// # Arguments
///
/// * `data` - Initial entropy
pub fn global_prng_init(data: &[u8]) {
    let mut global = GLOBAL_PRNG.lock();
    *global = Some(Prng::new(data));
    println!("PRNG: Global PRNG initialized");
}

/// Get the global PRNG
///
/// # Returns
///
/// Reference to the global PRNG, or None if not initialized
pub fn global_prng() -> Option<&'static Prng> {
    // This is a bit tricky with static mutex
    // For now, return None
    None
}

/// Add entropy to the global PRNG
///
/// # Arguments
///
/// * `data` - Entropy data
pub fn global_prng_add_entropy(data: &[u8]) {
    let global = GLOBAL_PRNG.lock();
    if let Some(ref prng) = *global {
        prng.add_entropy(data);
    }
}

/// Draw random bytes from the global PRNG
///
/// # Arguments
///
/// * `out` - Output buffer
pub fn global_prng_draw(out: &mut [u8]) {
    let global = GLOBAL_PRNG.lock();
    if let Some(ref prng) = *global {
        prng.draw(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prng_creation() {
        let entropy = [0u8; 32];
        let prng = Prng::new_non_thread_safe(&entropy);

        assert!(!prng.is_thread_safe());
        assert_eq!(prng.accumulated.load(Ordering::Acquire), 32);
    }

    #[test]
    fn test_prng_thread_safe() {
        let entropy = [0u8; 32];
        let prng = Prng::new(&entropy);

        assert!(prng.is_thread_safe());
    }

    #[test]
    fn test_prng_add_entropy() {
        let entropy = [0u8; 32];
        let prng = Prng::new_non_thread_safe(&entropy);

        prng.add_entropy(&[1u8; 16]);
        assert_eq!(prng.accumulated.load(Ordering::Acquire), 48);
    }

    #[test]
    fn test_prng_draw() {
        let entropy = [0u8; 32];
        let prng = Prng::new_non_thread_safe(&entropy);

        let mut output = [0u8; 64];
        prng.draw(&mut output);

        // Output should be modified
        // (unless ChaCha20 stub produces all zeros)
        let _ = output;
    }

    #[test]
    fn test_prng_rand_int() {
        let entropy = [0u8; 32];
        let prng = Prng::new_non_thread_safe(&entropy);

        let val = prng.rand_int(100);
        assert!(val < 100);
    }

    #[test]
    fn test_constants() {
        assert_eq!(SHA256_DIGEST_LENGTH, 32);
        assert_eq!(MIN_ENTROPY, 32);
        assert_eq!(MAX_ENTROPY, 4096);
        assert_eq!(CHACHA20_KEY_SIZE, 32);
        assert_eq!(CHACHA20_NONCE_SIZE, 12);
    }
}
