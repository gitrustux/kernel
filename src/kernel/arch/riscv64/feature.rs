// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V CPU feature detection
//!
//! Provides runtime detection of RISC-V ISA extensions and features.

use crate::bits;
use crate::arch::riscv64::registers;

/// RISC-V ISA extensions
pub mod extensions {
    pub const RISCV_ISA_EXT_A: u64 = 1 << 0;  // Atomic instructions
    pub const RISCV_ISA_EXT_B: u64 = 1 << 1;  // Bit-manipulation
    pub const RISCV_ISA_EXT_C: u64 = 1 << 2;  // Compressed instructions
    pub const RISCV_ISA_EXT_D: u64 = 1 << 3;  // Double-precision floating-point
    pub const RISCV_ISA_EXT_F: u64 = 1 << 4;  // Single-precision floating-point
    pub const RISCV_ISA_EXT_I: u64 = 1 << 5;  // Integer base ISA
    pub const RISCV_ISA_EXT_M: u64 = 1 << 6;  // Integer multiply/divide
    pub const RISCV_ISA_EXT_V: u64 = 1 << 7;  // Vector operations
    pub const RISCV_ISA_EXT_ZICBOM: u64 = 1 << 8;  // Cache-block management
    pub const RISCV_ISA_EXT_ZICBOP: u64 = 1 << 9;  // Cache-block prefetch
    pub const RISCV_ISA_EXT_ZICBOZ: u64 = 1 << 10; // Cache-block zero
    pub const RISCV_ISA_EXT_ZIHINTPAUSE: u64 = 1 << 11; // Pause hint
    pub const RISCV_ISA_EXT_SSTC: u64 = 1 << 12; // Supervisor-mode timer CSRs
}

/// Global feature flags
pub static mut riscv_features: u32 = 0;
pub static mut riscv_dcache_size: u32 = 64;  // Default 64-byte cache line
pub static mut riscv_icache_size: u32 = 64;  // Default 64-byte cache line

/// Detect CPU features at boot time
pub fn riscv_feature_early_detect() {
    // TODO: Implement proper feature detection
    // This typically requires:
    // 1. Reading device tree for CPU information
    // 2. Probing CSR registers for extension presence
    // 3. Detecting cache geometry from cbom/cboz extensions

    // For now, assume RV64GC (IMAFD) + C
    unsafe {
        riscv_features = (extensions::RISCV_ISA_EXT_I |
                          extensions::RISCV_ISA_EXT_M |
                          extensions::RISCV_ISA_EXT_A |
                          extensions::RISCV_ISA_EXT_F |
                          extensions::RISCV_ISA_EXT_D |
                          extensions::RISCV_ISA_EXT_C) as u32;
    }
}

/// Get CPU features as a u64 bitmask
pub fn riscv_get_features() -> u64 {
    unsafe { riscv_features as u64 }
}

/// Check if an extension is available
pub fn has_extension(ext: u64) -> bool {
    unsafe { (riscv_features as u64) & ext != 0 }
}
