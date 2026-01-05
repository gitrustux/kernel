// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::rustux::types::*;
use crate::sys::types::*;
use crate::rustux::compiler::*;

/// Adds a new peripheral range
pub unsafe fn add_periph_range(base_phys: paddr_t, length: size_t) -> rx_status_t;

/// Called after virtual memory is started to reserve peripheral ranges
/// in the kernel's address space
pub unsafe fn reserve_periph_ranges();

/// Translates peripheral physical address to virtual address in the big kernel map
pub unsafe fn periph_paddr_to_vaddr(paddr: paddr_t) -> vaddr_t;