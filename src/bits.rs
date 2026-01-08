// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Bit Manipulation Utilities
//!
//! This module provides bit-level manipulation functions used throughout
//! the kernel for working with hardware registers and bitfields.

#![no_std]

/// Extract a bitfield from a value
///
/// # Arguments
///
/// * `value` - The value to extract from
/// * `high` - High bit position (inclusive)
/// * `low` - Low bit position (inclusive)
///
/// # Examples
///
/// ```
/// let value = 0b1234_5678u32;
/// let field = BITS_SHIFT(value, 31, 28); // Extracts bits 31-28
/// ```
#[inline]
pub fn bits_shift<T: Into<u64>>(value: T, high: u32, low: u32) -> u64 {
    let v = value.into();
    (v >> low) & ((1 << (high - low + 1)) - 1)
}

/// Extract bits from a value (alias for compatibility)
#[inline]
pub fn BITS_SHIFT<T: Into<u64>>(value: T, high: u32, low: u32) -> u64 {
    bits_shift(value, high, low)
}

/// Extract bits from a value (alternative alias)
#[inline]
pub fn BITS<T: Into<u64>>(value: T, high: u32, low: u32) -> u64 {
    bits_shift(value, high, low)
}

/// Extract a single bit from a value
#[inline]
pub fn BIT<T: Into<u64>>(value: T, bit: u32) -> u64 {
    let v = value.into();
    (v >> bit) & 1
}

/// Set a bit in a value
#[inline]
pub fn set_bit<T: Into<u64> + From<u64>>(value: T, bit: u32) -> T {
    let v = value.into();
    let result = v | (1u64 << bit);
    T::from(result)
}

/// Clear a bit in a value
#[inline]
pub fn clear_bit<T: Into<u64> + From<u64>>(value: T, bit: u32) -> T {
    let v = value.into();
    let result = v & !(1u64 << bit);
    T::from(result)
}

/// Check if a bit is set
#[inline]
pub fn is_bit_set<T: Into<u64>>(value: T, bit: u32) -> bool {
    let v = value.into();
    (v & (1u64 << bit)) != 0
}

/// Count leading zeros
#[inline]
pub fn clz(value: u64) -> u32 {
    value.leading_zeros()
}

/// Count trailing zeros
#[inline]
pub fn ctz(value: u64) -> u32 {
    value.trailing_zeros()
}

/// Population count (count set bits)
#[inline]
pub fn popcount(value: u64) -> u32 {
    value.count_ones()
}

/// Find first set bit (1-based index, 0 if none set)
#[inline]
pub const fn ffs(value: u64) -> u32 {
    if value == 0 {
        0
    } else {
        value.trailing_zeros() + 1
    }
}

/// Round up to next power of 2
#[inline]
pub const fn round_up_pow2(value: u64) -> u64 {
    if value == 0 {
        1
    } else {
        1u64 << (64 - value.leading_zeros())
    }
}

/// Align up to given alignment (must be power of 2)
#[inline]
pub const fn align_up(value: u64, alignment: u64) -> u64 {
    (value + alignment - 1) & !(alignment - 1)
}

/// Align down to given alignment (must be power of 2)
#[inline]
pub const fn align_down(value: u64, alignment: u64) -> u64 {
    value & !(alignment - 1)
}
