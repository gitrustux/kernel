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

/// ============================================================================
/// x86-64 Page Table Entry Flags
/// ============================================================================

/// Present flag - page is present in memory
pub const X86_MMU_PG_P: u64 = 1 << 0;

/// Read/Write flag - page is writable
pub const X86_MMU_PG_RW: u64 = 1 << 1;

/// User/Supervisor flag - page is accessible from user mode
pub const X86_MMU_PG_U: u64 = 1 << 2;

/// Page Write Through flag - write-through caching
pub const X86_MMU_PG_PWT: u64 = 1 << 3;

/// Page Cache Disable flag - disable caching
pub const X86_MMU_PG_PCD: u64 = 1 << 4;

/// Accessed flag - page has been accessed
pub const X86_MMU_PG_A: u64 = 1 << 5;

/// Dirty flag - page has been written to
pub const X86_MMU_PG_D: u64 = 1 << 6;

/// Page Size flag - large page (2MB/1GB)
pub const X86_MMU_PG_PS: u64 = 1 << 7;

/// Global flag - page is global (not flushed on CR3 write)
pub const X86_MMU_PG_G: u64 = 1 << 8;

/// No Execute flag - page cannot be executed from (must be set in EFER.NXE)
pub const X86_MMU_PG_NX: u64 = 1u64 << 63;