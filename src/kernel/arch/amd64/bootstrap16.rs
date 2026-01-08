// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! 16-bit bootstrap code for SMP startup
//!
//! This module contains the 16-bit code used for booting secondary CPUs.


/// Bootstrap code for secondary CPUs
///
/// This is the entry point for secondary CPU cores.
#[no_mangle]
pub unsafe extern "C" fn bootstrap16() -> ! {
    // TODO: Implement 16-bit bootstrap code
    loop {
        core::hint::spin_loop();
    }
}
