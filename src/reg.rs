// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Register Access Module (Stub)
//!
//! Minimal stub for register access functionality.


/// Read a 32-bit register
#[inline]
pub fn read_reg32(addr: usize) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

/// Write a 32-bit register
#[inline]
pub fn write_reg32(addr: usize, val: u32) {
    unsafe { core::ptr::write_volatile(addr as *mut u32, val) }
}

/// Read a 64-bit register
#[inline]
pub fn read_reg64(addr: usize) -> u64 {
    unsafe { core::ptr::read_volatile(addr as *const u64) }
}

/// Write a 64-bit register
#[inline]
pub fn write_reg64(addr: usize, val: u64) {
    unsafe { core::ptr::write_volatile(addr as *mut u64, val) }
}
