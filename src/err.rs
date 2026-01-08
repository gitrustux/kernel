// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Error Codes
//!
//! This module provides status codes and error types used throughout
//! the Rustux kernel.

#![no_std]

// Use the rx_status_t from rustux::types
pub use crate::rustux::types::rx_status_t;

/// Success status code
pub const RX_OK: rx_status_t = 0;

/// Invalid arguments error
pub const RX_ERR_INVALID_ARGS: rx_status_t = -10;

/// No memory error
pub const RX_ERR_NO_MEMORY: rx_status_t = -12;

/// Out of range error
pub const RX_ERR_OUT_OF_RANGE: rx_status_t = -33;

/// Internal error
pub const RX_ERR_INTERNAL: rx_status_t = -114;

/// Not found error
pub const RX_ERR_NOT_FOUND: rx_status_t = -3;

/// Not implemented error
pub const RX_ERR_NOT_IMPLEMENTED: rx_status_t = -2;

/// Busy error
pub const RX_ERR_BUSY: rx_status_t = -1;

/// Timeout error
pub const RX_ERR_TIMED_OUT: rx_status_t = -5;

/// Permission denied error
pub const RX_ERR_PERMISSION_DENIED: rx_status_t = -13;
