//! Copyright 2025 The Rustux authors
//!
//! Use of this source code is governed by a MIT-style
//! license that can be found in the LICENSE file or at
//! https://opensource.org/licenses/MIT

/// Flags passed back from arm64_irq() to the calling assembler.
pub const ARM64_IRQ_EXIT_THREAD_SIGNALED: u32 = 1;
pub const ARM64_IRQ_EXIT_RESCHEDULE: u32 = 2;