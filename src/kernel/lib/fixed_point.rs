// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Fixed-Point Arithmetic
//!
//! This module provides fixed-point arithmetic utilities for the kernel.
//! Fixed-point numbers are useful for graphics, audio, and other operations
//! where floating-point is too slow or unavailable.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::{Add, Div, Mul, Sub};

use crate::rustux::types::*;

/// 16.16 fixed-point number (16 bits integer, 16 bits fractional)
pub type Fp16_16 = i32;

/// 24.8 fixed-point number (24 bits integer, 8 bits fractional)
pub type Fp24_8 = i32;

/// 8.24 fixed-point number (8 bits integer, 24 bits fractional)
pub type Fp8_24 = i32;

/// Convert integer to 16.16 fixed-point
#[inline]
pub const fn int_to_fp16_16(i: i32) -> Fp16_16 {
    i << 16
}

/// Convert 16.16 fixed-point to integer (truncate)
#[inline]
pub const fn fp16_16_to_int(f: Fp16_16) -> i32 {
    f >> 16
}

/// Convert 16.16 fixed-point to integer (round)
#[inline]
pub fn fp16_16_to_int_round(f: Fp16_16) -> i32 {
    (f + (1 << 15)) >> 16
}

/// Convert floating-point to 16.16 fixed-point
#[inline]
pub fn float_to_fp16_16(f: f32) -> Fp16_16 {
    (f * 65536.0) as i32
}

/// Convert 16.16 fixed-point to floating-point
#[inline]
pub fn fp16_16_to_float(f: Fp16_16) -> f32 {
    f as f32 / 65536.0
}

/// Multiply two 16.16 fixed-point numbers
#[inline]
pub fn fp16_16_mul(a: Fp16_16, b: Fp16_16) -> Fp16_16 {
    ((a as i64) * (b as i64) >> 16) as i32
}

/// Divide two 16.16 fixed-point numbers
#[inline]
pub fn fp16_16_div(a: Fp16_16, b: Fp16_16) -> Fp16_16 {
    ((a as i64) << 16 / b as i64) as i32
}

/// Add two 16.16 fixed-point numbers
#[inline]
pub const fn fp16_16_add(a: Fp16_16, b: Fp16_16) -> Fp16_16 {
    a + b
}

/// Subtract two 16.16 fixed-point numbers
#[inline]
pub const fn fp16_16_sub(a: Fp16_16, b: Fp16_16) -> Fp16_16 {
    a - b
}

/// Convert integer to 24.8 fixed-point
#[inline]
pub const fn int_to_fp24_8(i: i32) -> Fp24_8 {
    i << 8
}

/// Convert 24.8 fixed-point to integer (truncate)
#[inline]
pub const fn fp24_8_to_int(f: Fp24_8) -> i32 {
    f >> 8
}

/// Convert 24.8 fixed-point to integer (round)
#[inline]
pub fn fp24_8_to_int_round(f: Fp24_8) -> i32 {
    (f + (1 << 7)) >> 8
}

/// Multiply two 24.8 fixed-point numbers
#[inline]
pub fn fp24_8_mul(a: Fp24_8, b: Fp24_8) -> Fp24_8 {
    ((a as i64) * (b as i64) >> 8) as i32
}

/// Divide two 24.8 fixed-point numbers
#[inline]
pub fn fp24_8_div(a: Fp24_8, b: Fp24_8) -> Fp24_8 {
    ((a as i64) << 8 / b as i64) as i32
}

/// Convert integer to 8.24 fixed-point
#[inline]
pub const fn int_to_fp8_24(i: i32) -> Fp8_24 {
    i << 24
}

/// Convert 8.24 fixed-point to integer (truncate)
#[inline]
pub const fn fp8_24_to_int(f: Fp8_24) -> i32 {
    f >> 24
}

/// Convert 8.24 fixed-point to integer (round)
#[inline]
pub fn fp8_24_to_int_round(f: Fp8_24) -> i32 {
    (f + (1 << 23)) >> 24
}

/// Multiply two 8.24 fixed-point numbers
#[inline]
pub fn fp8_24_mul(a: Fp8_24, b: Fp8_24) -> Fp8_24 {
    ((a as i64) * (b as i64) >> 24) as i32
}

/// Divide two 8.24 fixed-point numbers
#[inline]
pub fn fp8_24_div(a: Fp8_24, b: Fp8_24) -> Fp8_24 {
    ((a as i64) << 24 / b as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fp16_16_conversion() {
        let f = int_to_fp16_16(10);
        assert_eq!(fp16_16_to_int(f), 10);
        assert_eq!(fp16_16_to_int_round(f), 10);
    }

    #[test]
    fn test_fp16_16_arithmetic() {
        let a = int_to_fp16_16(10);
        let b = int_to_fp16_16(5);

        let sum = fp16_16_add(a, b);
        assert_eq!(fp16_16_to_int(sum), 15);

        let diff = fp16_16_sub(a, b);
        assert_eq!(fp16_16_to_int(diff), 5);

        let product = fp16_16_mul(a, b);
        assert_eq!(fp16_16_to_int_round(product), 50);

        let quotient = fp16_16_div(a, b);
        assert_eq!(fp16_16_to_int_round(quotient), 2);
    }

    #[test]
    fn test_fp24_8_conversion() {
        let f = int_to_fp24_8(10);
        assert_eq!(fp24_8_to_int(f), 10);
        assert_eq!(fp24_8_to_int_round(f), 10);
    }

    #[test]
    fn test_fp8_24_conversion() {
        let f = int_to_fp8_24(10);
        assert_eq!(fp8_24_to_int(f), 10);
        assert_eq!(fp8_24_to_int_round(f), 10);
    }

    #[test]
    fn test_float_conversion() {
        let f = 1.5f32;
        let fp = float_to_fp16_16(f);
        assert_eq!(fp, int_to_fp16_16(1) + (1 << 15)); // 1.5 = 1 + 0.5

        let back = fp16_16_to_float(fp);
        assert!((back - f).abs() < 0.0001);
    }
}
