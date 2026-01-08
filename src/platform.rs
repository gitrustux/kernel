// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Platform Compatibility Module
//!
//! This module provides platform-specific initialization and functions.
//! It wraps the existing C++ platform code with a Rust interface.


// ============================================================================
// Halt Constants
// ============================================================================

/// Halt reason: software panic
pub const HALT_REASON_SW_PANIC: u32 = 0x1;

/// Halt reason: user request
pub const HALT_REASON_USER_REQUEST: u32 = 0x2;

/// Halt action: halt the system
pub const HALT_ACTION_HALT: u32 = 0x1;

/// Halt action: reboot the system
pub const HALT_ACTION_REBOOT: u32 = 0x2;

/// Halt action: shutdown the system
pub const HALT_ACTION_SHUTDOWN: u32 = 0x3;

// ============================================================================
// Platform Functions
// ============================================================================

/// Initialize platform-specific MMU mappings
pub fn platform_init_mmu_mappings() {
    // Platform-specific MMU mappings are handled by architecture code
}

/// Platform halt
pub fn platform_halt(_reason: u32, _action: u32) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Platform panic start
pub fn platform_panic_start() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Get platform IRQ information
pub fn platform_irq() -> u32 {
    0 // No IRQ by default
}

pub mod init {
    use crate::rustux::types::*;

    /// Initialize platform-specific MMU mappings
    ///
    /// This is called early in boot to set up any platform-specific
    /// memory mappings before the main MMU initialization.
    pub fn init_mmu_mappings() {
        // Platform-specific MMU mappings are handled by architecture code
        // This is a placeholder for future platform-specific needs
    }

    /// Early platform initialization
    ///
    /// Called very early in boot, before most kernel services are available.
    pub fn early_init() {
        // Early platform initialization happens in architecture code
        // This is a placeholder for future platform-specific needs
    }

    /// Main platform initialization
    ///
    /// Called after basic kernel services are up.
    pub fn init() {
        // Platform initialization happens in architecture code
        // This is a placeholder for future platform-specific needs
    }

    /// Quiesce platform activity
    ///
    /// Called before system suspend or shutdown.
    pub fn quiesce() {
        // TODO: Implement platform quiesce
    }

    /// Platform panic start
    ///
    /// Called when the kernel panics.
    pub fn panic_start() -> ! {
        // TODO: Implement platform-specific panic handling
        loop {
            core::hint::spin_loop();
        }
    }

    /// Get platform ramdisk (if any)
    ///
    /// Returns a pointer to the ramdisk and its size.
    pub fn get_ramdisk() -> (PAddr, usize) {
        (0, 0) // No ramdisk by default
    }
}
