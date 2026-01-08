// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM64 EL2 State Module (Stub)
//!
//! Minimal stub for EL2 state management.


/// EL2 state structure
#[repr(C)]
pub struct El2State {
    pub enabled: bool,
}

/// Initialize EL2 state
pub fn init() -> El2State {
    El2State { enabled: false }
}
