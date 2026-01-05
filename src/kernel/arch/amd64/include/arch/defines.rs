// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture-specific constants for x86_64
//!
//! This module contains architecture-specific constants related to
//! memory management, caching, and system configuration for x86_64.

/// Page size in bytes (4 KiB)
pub const PAGE_SIZE: usize = 4096;

/// Bit shift for page size (log2 of page size)
pub const PAGE_SIZE_SHIFT: usize = 12;

/// Bitmask for addresses within a page
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

/// Maximum CPU cache line size in bytes
pub const MAX_CACHE_LINE: usize = 64;

/// Default stack size for architecture-specific threads
pub const ARCH_DEFAULT_STACK_SIZE: usize = 8192;

/// Size of the physical memory mapping window (64 GiB)
pub const ARCH_PHYSMAP_SIZE: usize = 0x1000000000;