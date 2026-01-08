// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! CPU management module


use core::sync::atomic::{AtomicU32, Ordering};

/// CPU mask type
pub type CpuMask = u64;

/// CPU number type
pub type CpuNum = u32;

/// Number of assigned IST entries
pub const NUM_ASSIGNED_IST_ENTRIES: usize = 6;
