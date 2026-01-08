// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Thread Local Storage (TLS) module

#![no_std]

/// TLS data placeholder
#[repr(C)]
pub struct TlsData {
    pub data: [u8; 0],
}

/// Stack guard offset in TLS
pub const ZX_TLS_STACK_GUARD_OFFSET: usize = 0x10;

/// Unsafe stack pointer offset in TLS
pub const ZX_TLS_UNSAFE_SP_OFFSET: usize = 0x18;

/// Legacy aliases for compatibility
pub const RX_TLS_STACK_GUARD_OFFSET: usize = ZX_TLS_STACK_GUARD_OFFSET;
pub const RX_TLS_UNSAFE_SP_OFFSET: usize = ZX_TLS_UNSAFE_SP_OFFSET;

/// Get current TLS data
pub fn tls_get() -> *mut TlsData {
    core::ptr::null_mut()
}
