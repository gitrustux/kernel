// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Trace and Logging Support
//!
//! This module provides trace-specific macros that use the logging
//! functionality from the debug module.

#![no_std]

/// Function entry trace macro
#[macro_export]
macro_rules! LTRACEF {
    ($($arg:tt)*) => {
        #[cfg(feature = "kernel-debug")]
        {
            crate::log_debug!($($arg)*);
        }
    };
}

/// Function entry trace with return value
#[macro_export]
macro_rules! LTRACEF_RET {
    ($($arg:tt)*) => {
        #[cfg(feature = "kernel-debug")]
        {
            crate::log_debug!($($arg)*);
        }
    };
}

/// Function entry macro
#[macro_export]
macro_rules! TRACE_ENTRY {
    ($($arg:tt)*) => {
        #[cfg(feature = "kernel-debug")]
        {
            crate::log_debug!($($arg)*);
        }
    };
}

/// Function exit macro
#[macro_export]
macro_rules! TRACE_EXIT {
    ($($arg:tt)*) => {
        #[cfg(feature = "kernel-debug")]
        {
            crate::log_debug!($($arg)*);
        }
    };
}

// Re-export LTRACEF
pub use LTRACEF;
pub use LTRACEF_RET;
pub use TRACE_ENTRY;
pub use TRACE_EXIT;