// Copyright 2025 Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64::*;
use crate::arch::arm64::mp::*;
use crate::debug::*;
use crate::kernel::thread::*;
use core::ptr;

const LOCAL_TRACE: bool = false;

/// Register state layout used by arm64_context_switch().
#[repr(C, align(16))]
struct ContextSwitchFrame {
    tpidr_el0: u64,
    tpidrro_el0: u64,
    r19: u64,
    r20: u64,
    r21: u64,
    r22: u64,
    r23: u64,
    r24: u64,
    r25: u64,
    r26: u64,
    r27: u64,
    r28: u64,
    r29: u64,
    lr: u64,
}

// assert that the context switch frame is a multiple of 16 to maintain
// stack alignment requirements per ABI
const_assert_eq!(core::mem::size_of::<ContextSwitchFrame>() % 16, 0);

extern "C" {
    fn arm64_context_switch(old_sp: *mut addr_t, new_sp: addr_t);
    fn arm64_read_percpu_ptr() -> *mut u8;
    fn arm64_fpu_context_switch(oldthread: *const Thread, newthread: *const Thread);
    fn arm64_write_hw_debug_regs(state: *const Arm64DebugState);
    fn arm64_enable_debug_state();
    fn arm64_disable_debug_state();
}

/// Initialize architecture-specific thread state
///
/// # Arguments
///
/// * `t` - Thread to initialize
/// * `entry_point` - Entry point address for the thread
pub fn arch_thread_initialize(t: &mut Thread, entry_point: vaddr_t) {
    // zero out the entire arch state
    t.arch = ArchThread::default();

    // create a default stack frame on the stack
    let mut stack_top = t.stack.top;

    // make sure the top of the stack is 16 byte aligned for EABI compliance
    stack_top = round_down(stack_top, 16);
    t.stack.top = stack_top;

    let frame = (stack_top as *mut ContextSwitchFrame).offset(-1);

    // fill in the entry point
    unsafe {
        (*frame).lr = entry_point;
    }

    // This is really a global (boot-time) constant value.
    // But it's stored in each thread struct to satisfy the
    // compiler ABI (TPIDR_EL1 + RX_TLS_STACK_GUARD_OFFSET).
    t.arch.stack_guard = get_current_thread().arch.stack_guard;

    // set the stack pointer
    t.arch.sp = frame as vaddr_t;
    
    #[cfg(feature = "safe_stack")]
    {
        t.arch.unsafe_sp = round_down(t.stack.unsafe_base + t.stack.size, 16);
    }

    // Initialize the debug state to a valid initial state.
    for i in 0..ARM64_MAX_HW_BREAKPOINTS {
        t.arch.debug_state.hw_bps[i].dbgbcr = (0b10u32 << ARM64_DBGBCR_PMC_SHIFT) | ARM64_DBGBCR_BAS;
        t.arch.debug_state.hw_bps[i].dbgbvr = 0;
    }
}

/// Construct the first thread for the system
///
/// # Arguments
///
/// * `t` - Thread to construct
#[no_mangle]
#[cfg_attr(feature = "safe_stack", no_safe_stack)]
pub extern "C" fn arch_thread_construct_first(t: &mut Thread) {
    // Propagate the values from the fake arch_thread that the thread
    // pointer points to now (set up in start.S) into the real thread
    // structure being set up now.
    let fake = get_current_thread();
    t.arch.stack_guard = fake.arch.stack_guard;
    t.arch.unsafe_sp = fake.arch.unsafe_sp;

    // make sure the thread saves a copy of the current cpu pointer
    t.arch.current_percpu_ptr = unsafe { arm64_read_percpu_ptr() };

    // Force the thread pointer immediately to the real struct. This way
    // our callers don't have to avoid safe-stack code or risk losing track
    // of the unsafe_sp value. The caller's unsafe_sp value is visible at
    // TPIDR_EL1 + RX_TLS_UNSAFE_SP_OFFSET as expected, though TPIDR_EL1
    // happens to have changed. (We're assuming that the compiler doesn't
    // decide to cache the TPIDR_EL1 value across this function call, which
    // would be pointless since it's just one instruction to fetch it afresh.)
    set_current_thread(t);
}

/// Switch context to a different thread
///
/// # Arguments
///
/// * `oldthread` - Thread to switch from
/// * `newthread` - Thread to switch to
#[no_mangle]
#[cfg_attr(feature = "safe_stack", no_safe_stack)]
pub extern "C" fn arch_context_switch(oldthread: &mut Thread, newthread: &mut Thread) {
    if LOCAL_TRACE {
        ltrace!("old %p (%s), new %p (%s)\n", 
                oldthread as *mut Thread, 
                oldthread.name.as_ptr(), 
                newthread as *mut Thread, 
                newthread.name.as_ptr());
    }
    
    unsafe {
        __dsb(ARM_MB_SY); /* broadcast tlb operations in case the thread moves to another cpu */
    }

    /* set the current cpu pointer in the new thread's structure so it can be
     * restored on exception entry.
     */
    unsafe {
        newthread.arch.current_percpu_ptr = arm64_read_percpu_ptr();

        arm64_fpu_context_switch(oldthread as *const Thread, newthread as *const Thread);
        arm64_debug_state_context_switch(oldthread, newthread);
        arm64_context_switch(&mut oldthread.arch.sp, newthread.arch.sp);
    }
}

/// Dump thread information for debugging
///
/// # Arguments
///
/// * `t` - Thread to dump information for
pub fn arch_dump_thread(t: &Thread) {
    if t.state != ThreadState::Running {
        dprintf!(INFO, "\tarch: ");
        dprintf!(INFO, "sp 0x%lx\n", t.arch.sp);
    }
}

/// Get the frame pointer for a blocked thread
///
/// # Arguments
///
/// * `t` - Thread to get frame pointer for
///
/// # Returns
///
/// * Pointer to the frame, or null if not available
pub fn arch_thread_get_blocked_fp(t: &Thread) -> *mut u8 {
    if !cfg!(feature = "frame_pointers") {
        return ptr::null_mut();
    }

    let frame = t.arch.sp as *const ContextSwitchFrame;
    unsafe { (*frame).r29 as *mut u8 }
}

/// Handle debug state during context switch
///
/// # Arguments
///
/// * `old_thread` - Thread being switched from
/// * `new_thread` - Thread being switched to
pub fn arm64_debug_state_context_switch(old_thread: &Thread, new_thread: &Thread) {
    // If the new thread has debug state, then install it, replacing the current contents.
    if unlikely(new_thread.arch.track_debug_state) {
        unsafe {
            arm64_write_hw_debug_regs(&new_thread.arch.debug_state);
            arm64_enable_debug_state();
        }
        return;
    }

    // If the old thread had debug state running and the new one doesn't use it, disable the
    // debug capabilities. We don't need to clear the state because if a new thread being
    // scheduled needs them, then it will overwrite the state.
    if unlikely(old_thread.arch.track_debug_state) {
        unsafe {
            arm64_disable_debug_state();
        }
    }
}

// Helper function to round down to the nearest multiple
fn round_down(val: vaddr_t, multiple: vaddr_t) -> vaddr_t {
    val & !(multiple - 1)
}

// Helper macro for compile-time assertion
macro_rules! const_assert_eq {
    ($left:expr, $right:expr) => {
        const _: [(); 0 - !{
            const ASSERT: bool = $left == $right;
            ASSERT
        } as usize] = [];
    };
}

// Helper macro for unlikely optimization hint
fn unlikely(b: bool) -> bool {
    if cfg!(feature = "optimize") {
        use core::intrinsics::unlikely as intrinsic_unlikely;
        unsafe { intrinsic_unlikely(b) }
    } else {
        b
    }
}

// ARM64-specific thread state
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct ArchThread {
    pub sp: vaddr_t,
    #[cfg(feature = "safe_stack")]
    pub unsafe_sp: vaddr_t,
    pub stack_guard: u64,
    pub current_percpu_ptr: *mut u8,
    pub track_debug_state: bool,
    pub debug_state: Arm64DebugState,
}

// Constants
const ARM64_MAX_HW_BREAKPOINTS: usize = 16;
const ARM64_DBGBCR_PMC_SHIFT: u32 = 1; // Position in register
const ARM64_DBGBCR_BAS: u32 = 0xFF; // Byte Address Select mask
const ARM_MB_SY: u32 = 15; // Full system memory barrier

// Define ThreadState enum
#[repr(u32)]
pub enum ThreadState {
    Running = 0,
    // Other thread states would be defined here
}

// Define types that would be imported from other modules
type vaddr_t = u64;
type addr_t = u64;
const INFO: i32 = 0; // Log level for info messages

// Define the hardware breakpoint structure
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Arm64HwBreakpoint {
    pub dbgbcr: u32,
    pub dbgbvr: u64,
}

// Define the debug state structure
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Arm64DebugState {
    pub hw_bps: [Arm64HwBreakpoint; ARM64_MAX_HW_BREAKPOINTS],
}

// External function declarations for functions implemented elsewhere
extern "C" {
    fn get_current_thread() -> &'static mut Thread;
    fn set_current_thread(thread: &Thread);
    fn __dsb(mb_type: u32);
}