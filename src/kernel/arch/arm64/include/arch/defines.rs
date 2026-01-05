// Copyright 2025 The Rustux Authors
// Copyright (c) 2008 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture-specific definitions for ARM64

// Page size shifts for different page sizes
pub const SHIFT_4K: usize = 12;
pub const SHIFT_16K: usize = 14;
pub const SHIFT_64K: usize = 16;

// ARM specific page size configuration
// The actual page size shift is determined by build configuration
#[cfg(feature = "arm64_large_pagesize_64k")]
pub const PAGE_SIZE_SHIFT: usize = SHIFT_64K;

#[cfg(feature = "arm64_large_pagesize_16k")]
pub const PAGE_SIZE_SHIFT: usize = SHIFT_16K;

#[cfg(not(any(feature = "arm64_large_pagesize_64k", feature = "arm64_large_pagesize_16k")))]
pub const PAGE_SIZE_SHIFT: usize = SHIFT_4K;

pub const USER_PAGE_SIZE_SHIFT: usize = SHIFT_4K;

pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_SHIFT;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

pub const USER_PAGE_SIZE: usize = 1 << USER_PAGE_SIZE_SHIFT;
pub const USER_PAGE_MASK: usize = USER_PAGE_SIZE - 1;

/// The maximum cache line seen on any known ARM hardware
pub const MAX_CACHE_LINE: usize = 128;

/// Bit manipulation macro: creates a bitmask with `count` bits starting at position `base` with value `val`
#[inline(always)]
pub const fn bm(base: usize, count: usize, val: u64) -> u64 {
    ((val) & ((1u64 << (count)) - 1)) << (base)
}

// ARM64 MMFR0 (Memory Model Feature Register 0) bit definitions
pub const ARM64_MMFR0_ASIDBITS_16: u64 = bm(4, 4, 2);
pub const ARM64_MMFR0_ASIDBITS_8: u64 = bm(4, 4, 0);
pub const ARM64_MMFR0_ASIDBITS_MASK: u64 = bm(4, 4, 15);

/// Default stack size for ARM64 architecture
pub const ARCH_DEFAULT_STACK_SIZE: usize = 8192;

/// Map 512GB at the base of the kernel. This is the max that can be mapped with a
/// single level 1 page table using 1GB pages.
pub const ARCH_PHYSMAP_SIZE: u64 = 1u64 << 39;