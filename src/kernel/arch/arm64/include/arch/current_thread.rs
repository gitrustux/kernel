// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use core::mem::offset_of;
use crate::kernel::thread::Thread;

/// Get the current thread from the CPU local thread context pointer
///
/// This uses the TPIDR_EL1 register which holds a pointer to the current thread
/// when running in kernel mode on ARM64.
#[inline(always)]
pub unsafe fn get_current_thread() -> *mut Thread {
    let tp: *mut u8;
    
    // Clang with --target=aarch64-fuchsia -mcmodel=kernel reads
    // TPIDR_EL1 for __builtin_thread_pointer (instead of the usual
    // TPIDR_EL0 for user mode). Using the intrinsic instead of asm
    // lets the compiler understand what it's doing a little better,
    // which conceivably could let it optimize better.
    #[cfg(target_feature = "aarch64")]
    {
        // In Rust we use inline assembly to read the register
        core::arch::asm!(
            "mrs {}, tpidr_el1",
            out(reg) tp,
            options(nomem, nostack, preserves_flags)
        );
    }
    
    #[cfg(not(target_feature = "aarch64"))]
    {
        // Fallback for non-ARM64 builds or testing
        tp = core::ptr::null_mut();
    }
    
    // Adjust the pointer to get to the start of the thread struct
    (tp as *mut Thread).offset(-(offset_of!(Thread, arch.thread_pointer_location) as isize))
}

/// Set the current thread in the CPU local thread context pointer
///
/// This writes to the TPIDR_EL1 register to store a pointer to the current thread
/// when running in kernel mode on ARM64.
#[inline(always)]
pub unsafe fn set_current_thread(t: *mut Thread) {
    // Calculate the location of thread_pointer_location within the thread struct
    let tp_loc = core::ptr::addr_of_mut!((*t).arch.thread_pointer_location) as *mut u8;

    #[cfg(target_feature = "aarch64")]
    {
        // Write the pointer to TPIDR_EL1
        core::arch::asm!(
            "msr tpidr_el1, {}",
            "isb sy",
            in(reg) tp_loc,
            options(nomem, nostack, preserves_flags)
        );
    }
    
    #[cfg(not(target_feature = "aarch64"))]
    {
        // Fallback for non-ARM64 builds or testing
        // This is a no-op in non-ARM64 environments
    }
}