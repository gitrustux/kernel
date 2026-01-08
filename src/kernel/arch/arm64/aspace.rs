// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 Address Space Module (Stub)
//!
//! Minimal stub for address space management.

#![no_std]

/// Address space structure
pub struct AddressSpace {
    pub virt_base: u64,
    pub virt_size: u64,
}

/// Initialize address space
pub fn init() -> AddressSpace {
    AddressSpace {
        virt_base: 0,
        virt_size: 0,
    }
}
