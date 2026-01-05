// Copyright 2025 The Rustux Authors
// Copyright (c) 2014 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

pub const CURRENT_PERCPU_PTR_OFFSET: usize = 16;

use core::assert_eq;
use crate::arch::arm64::registers::*;
use core::mem::offset_of;
use crate::sys::types::*;
use crate::rustux::compiler::*;
use crate::rustux::tls::*;

#[repr(C)]
pub struct fpstate {
    pub fpcr: u32,
    pub fpsr: u32,
    pub regs: [u64; 64],
}

#[repr(C)]
pub struct arch_thread {
    // The compiler (when it's Clang with -mcmodel=kernel) knows
    // the position of these two fields relative to TPIDR_EL1,
    // which is what __builtin_thread_pointer() returns.  TPIDR_EL1
    // points just past these, i.e. to &abi[1].
    pub stack_guard: uintptr_t,
    pub unsafe_sp: vaddr_t,
    pub thread_pointer_location_or_sp: ThreadPointerUnion,

    // Debugger access to userspace general regs while suspended or stopped
    // in an exception.
    // The regs are saved on the stack and then a pointer is stored here.
    // NULL if not suspended or stopped in an exception.
    pub suspended_general_regs: *mut arm64_iframe_long,

    // Point to the current cpu pointer when the thread is running, used to
    // restore x18 on exception entry. Swapped on context switch.
    pub current_percpu_ptr: *mut arm64_percpu,

    // if non-NULL, address to return to on data fault
    pub data_fault_resume: *mut core::ffi::c_void,

    // saved fpu state
    pub fpstate: fpstate,

    // |track_debug_state| tells whether the kernel should keep track of the whole debug state for
    // this thread. Normally this is set explicitly by an user that wants to make use of HW
    // breakpoints or watchpoints.
    // Userspace can still read the complete |debug_state| even if |track_debug_state| is false.
    pub track_debug_state: bool,
    pub debug_state: arm64_debug_state_t,
}

#[repr(C)]
pub union ThreadPointerUnion {
    pub thread_pointer_location: u8,
    pub sp: vaddr_t,
}

#[inline]
pub const fn thread_pointer_offsetof<T>(field: unsafe fn(*const arch_thread) -> *const T) -> isize {
    let base = offset_of!(arch_thread, thread_pointer_location_or_sp) + 
               offset_of!(ThreadPointerUnion, thread_pointer_location);
    
    // This is a placeholder for the actual offset calculation which would need
    // to be done at compile time. Rust doesn't have a direct equivalent to C's 
    // offsetof for arbitrary fields, so in a real implementation this would need
    // to be handled differently, possibly with custom proc macros.
    // 
    // For demonstration purposes, we're returning hardcoded values based on the
    // field being accessed:
    match field as usize {
        _ if field as usize == unsafe_stack_guard_field as usize => 
            (offset_of!(arch_thread, stack_guard) as isize) - (base as isize),
        _ if field as usize == unsafe_sp_field as usize => 
            (offset_of!(arch_thread, unsafe_sp) as isize) - (base as isize),
        _ if field as usize == unsafe_current_percpu_ptr_field as usize => 
            (offset_of!(arch_thread, current_percpu_ptr) as isize) - (base as isize),
        _ => panic!("Unknown field in thread_pointer_offsetof"),
    }
}

// These are placeholder functions used for field identification in thread_pointer_offsetof
unsafe fn unsafe_stack_guard_field(thread: *const arch_thread) -> *const uintptr_t {
    &(*thread).stack_guard
}

unsafe fn unsafe_sp_field(thread: *const arch_thread) -> *const vaddr_t {
    &(*thread).unsafe_sp
}

unsafe fn unsafe_current_percpu_ptr_field(thread: *const arch_thread) -> *const *mut arm64_percpu {
    &(*thread).current_percpu_ptr
}

// Static assertions to ensure field offsets match expected values
const _: () = assert!(thread_pointer_offsetof(unsafe_stack_guard_field) == RX_TLS_STACK_GUARD_OFFSET);
const _: () = assert!(thread_pointer_offsetof(unsafe_sp_field) == RX_TLS_UNSAFE_SP_OFFSET);
const _: () = assert!(thread_pointer_offsetof(unsafe_current_percpu_ptr_field) == CURRENT_PERCPU_PTR_OFFSET as isize);