// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Alignment utilities


use core::fmt;

/// Aligned marker trait
pub trait Aligned {}

/// CPU alignment
pub struct CpuAlign;

impl Aligned for CpuAlign {}
