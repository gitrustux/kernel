// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM GIC (Generic Interrupt Controller)
//!
//! This module provides support for ARM's Generic Interrupt Controller.
//! Currently only GICv3 is implemented.


pub mod v3;

// Re-exports
pub use v3::*;

// ============================================================================
// Common Interrupt Configuration Types
// ============================================================================

/// Interrupt trigger mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptTriggerMode {
    /// Edge-triggered interrupt
    Edge = 0,
    /// Level-triggered interrupt
    Level = 1,
}

/// Interrupt polarity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptPolarity {
    /// Active-high polarity
    ActiveHigh = 0,
    /// Active-low polarity
    ActiveLow = 1,
}

/// SGI (Software Generated Interrupt) target filter flags
pub const ARM_GIC_SGI_FLAG_TARGET_FILTER_MASK: u32 = 0x3;
pub const ARM_GIC_SGI_FLAG_TARGET_LIST: u32 = 0x0;
pub const ARM_GIC_SGI_FLAG_TARGET_ALL_OTHERS: u32 = 0x1;
pub const ARM_GIC_SGI_FLAG_TARGET_SELF: u32 = 0x2;
