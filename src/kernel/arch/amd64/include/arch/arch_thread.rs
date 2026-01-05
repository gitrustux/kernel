// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Architecture-specific thread context for x86_64
//!
//! This module defines the architecture-specific portion of thread context
//! for x86_64 processors, including register state, FPU state, and debug state.

use crate::kernel::arch::amd64::registers::{X86SyscallGeneralRegs, X86DebugState, X86_MAX_EXTENDED_REGISTER_SIZE};
use crate::kernel::arch::amd64::registers::msr;
use crate::kernel::arch::amd64::mmu;
use crate::println;
use crate::rustux::types::*;
use core::ptr::NonNull;
use core::mem::MaybeUninit;

/// Wrapper type for 64-byte aligned buffer
#[repr(align(64))]
#[derive(Clone, Copy)]
pub struct AlignedBuffer {
    pub data: [u8; X86_MAX_EXTENDED_REGISTER_SIZE + 64],
}

/// Source type for suspended general registers
pub const X86_GENERAL_REGS_NONE: u32 = 0;
/// General registers from syscall state
pub const X86_GENERAL_REGS_SYSCALL: u32 = 1;
/// General registers from interrupt frame
pub const X86_GENERAL_REGS_IFRAME: u32 = 2;

/// Interrupt frame structure
/// This is defined elsewhere in the kernel but referenced here
#[repr(C)]
pub struct X86Iframe {
    _data: [u8; 0],
}

/// Architecture-specific thread state for x86_64
#[repr(C)]
pub struct ArchThread {
    /// Stack pointer
    pub sp: VAddr,
    
    /// Unsafe stack pointer (only when safe stack is enabled)
    #[cfg(feature = "safe_stack")]
    pub unsafe_sp: VAddr,
    
    /// FS segment base address
    pub fs_base: VAddr,
    
    /// GS segment base address
    pub gs_base: VAddr,
    
    /// Which entry of suspended_general_regs to use
    /// One of X86_GENERAL_REGS_*
    pub general_regs_source: u32,
    
    /// Debugger access to userspace general regs while suspended or stopped
    /// in an exception. The regs are saved on the stack and then a pointer is stored here.
    /// NULL if not suspended or stopped in an exception.
    pub suspended_general_regs: SuspendedGeneralRegs,
    
    /// Buffer to save FPU and extended register (e.g., PT) state
    pub extended_register_state: *mut core::ffi::c_void,

    /// Buffer for extended register state (aligned to 64 bytes)
    pub extended_register_buffer: AlignedBuffer,

    /// If non-NULL, address to return to on page fault
    pub page_fault_resume: *mut core::ffi::c_void,
    
    /// Whether the kernel should keep track of the whole debug state for this thread
    ///
    /// Normally this is set explicitly by a user that wants to make use of HW
    /// breakpoints or watchpoints. The debug_state will still keep track of the
    /// status of the exceptions (DR6), as there are HW exceptions that are triggered
    /// without explicit debug state setting (e.g., single step).
    ///
    /// Userspace can still read the complete debug_state even if track_debug_state is false.
    /// As normally the CPU only changes DR6, the debug_state will be up to date anyway.
    pub track_debug_state: bool,
    
    /// Debug register state
    pub debug_state: X86DebugState,
}

/// Union of possible general register sources
#[repr(C)]
pub union SuspendedGeneralRegs {
    /// Generic pointer to registers
    pub gregs: *mut core::ffi::c_void,
    /// Pointer to syscall general registers
    pub syscall: *mut X86SyscallGeneralRegs,
    /// Pointer to interrupt frame
    pub iframe: *mut X86Iframe,
}

impl SuspendedGeneralRegs {
    /// Create a new, empty suspended general registers union
    pub const fn new() -> Self {
        Self { gregs: core::ptr::null_mut() }
    }
}

impl ArchThread {
    /// Create a new, zeroed arch thread structure
    pub fn new() -> Self {
        Self {
            sp: 0,
            #[cfg(feature = "safe_stack")]
            unsafe_sp: 0,
            fs_base: 0,
            gs_base: 0,
            general_regs_source: X86_GENERAL_REGS_NONE,
            suspended_general_regs: SuspendedGeneralRegs::new(),
            extended_register_state: core::ptr::null_mut(),
            extended_register_buffer: AlignedBuffer { data: [0; X86_MAX_EXTENDED_REGISTER_SIZE + 64] },
            page_fault_resume: core::ptr::null_mut(),
            track_debug_state: false,
            debug_state: X86DebugState {
                dr0: 0,
                dr1: 0,
                dr2: 0,
                dr3: 0,
                dr6: 0,
                dr7: 0,
            },
        }
    }

    /// Set the suspended general registers for a thread
    ///
    /// # Arguments
    ///
    /// * `source` - Source of the registers (X86_GENERAL_REGS_*)
    /// * `gregs` - Pointer to the register state
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The thread doesn't already have suspended registers set
    /// - The provided gregs pointer is valid and of the correct type for the given source
    pub unsafe fn x86_set_suspended_general_regs(&mut self, source: u32, gregs: *mut core::ffi::c_void) {
        debug_assert!(self.suspended_general_regs.gregs.is_null());
        debug_assert!(!gregs.is_null());
        debug_assert!(source != X86_GENERAL_REGS_NONE);
        
        self.general_regs_source = source;
        self.suspended_general_regs.gregs = gregs;
    }

    /// Reset the suspended general registers for a thread
    pub fn x86_reset_suspended_general_regs(&mut self) {
        self.general_regs_source = X86_GENERAL_REGS_NONE;
        unsafe {
            self.suspended_general_regs.gregs = core::ptr::null_mut();
        }
    }
}

// Provide default implementation
impl Default for ArchThread {
    fn default() -> Self {
        Self::new()
    }
}

/// Context switch frame for x86_64
///
/// This structure is pushed on the stack when a thread is suspended
/// and contains all the state that needs to be saved/restored.
#[repr(C)]
pub struct X86ContextSwitchFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rip: u64,
}

/// Initialize the architecture-specific portion of a thread
///
/// # Arguments
///
/// * `thread` - The thread to initialize
/// * `entry_point` - The entry point address where the thread should start execution
///
/// # Safety
///
/// Caller must ensure the thread structure is valid and the entry point is a valid address
pub unsafe fn arch_thread_initialize(thread: &mut crate::kernel::thread::Thread, entry_point: VAddr) {
    use crate::kernel::thread::Thread;

    // Get the top of the stack
    let mut stack_top = thread.stack_top();

    // Make sure the top of the stack is 16 byte aligned for ABI compliance
    stack_top &= !0xf;  // ROUNDDOWN(stack_top, 16)

    // Make sure we start the frame 8 byte unaligned (relative to 16 byte alignment)
    // because of the way the context switch will pop the return address
    stack_top -= 8;

    // Zero the 8 bytes for the return address
    *(stack_top as *mut u64) = 0;

    // Get the context switch frame
    let mut frame_ptr = (stack_top as *mut X86ContextSwitchFrame).offset(-1);

    // Zero out the frame
    core::ptr::write_bytes(frame_ptr, 0, 1);

    // Set the entry point
    (*frame_ptr).rip = entry_point;

    // Initialize extended register state (FPU/SSE/AVX)
    let arch_state = &mut thread.arch;
    // Allocate buffer for extended register state
    let buf = (arch_state.extended_register_state as VAddr + 63) & !63;  // ROUNDUP to 64
    arch_state.extended_register_state = buf as *mut core::ffi::c_void;

    // Initialize the extended register state to a clean state
    x86_extended_register_init_state(arch_state.extended_register_state);

    // Set the stack pointer
    arch_state.sp = frame_ptr as VAddr;

    // Initialize fs_base and gs_base to 0
    arch_state.fs_base = 0;
    arch_state.gs_base = 0;

    // Initialize debug registers to a valid initial state
    arch_state.track_debug_state = false;
    arch_state.debug_state.dr0 = 0;
    arch_state.debug_state.dr1 = 0;
    arch_state.debug_state.dr2 = 0;
    arch_state.debug_state.dr3 = 0;
    arch_state.debug_state.dr6 = !0xFFFF0FF0u64;  // ~X86_DR6_USER_MASK
    arch_state.debug_state.dr7 = !0x400u64;      // ~X86_DR7_USER_MASK
}

/// Construct the first (bootstrap) thread
///
/// # Arguments
///
/// * `thread` - The first thread to construct
pub fn arch_thread_construct_first(thread: &mut crate::kernel::thread::Thread) {
    // The first thread doesn't need any special initialization on x86_64
    // as the context switch code will handle it
}

/// Dump thread state for debugging
///
/// # Arguments
///
/// * `thread` - The thread to dump
pub fn arch_dump_thread(thread: &crate::kernel::thread::Thread) {
    use crate::kernel::thread::ThreadState;

    if *thread.state.lock() != ThreadState::Running {
        println!("\tarch: sp {:#x}", thread.arch.sp);
    }
}

/// Get the blocked frame pointer for debugging
///
/// # Arguments
///
/// * `thread` - The thread to get the frame pointer from
///
/// # Returns
///
/// The frame pointer, or NULL if frame pointers are disabled
pub fn arch_thread_get_blocked_fp(thread: &crate::kernel::thread::Thread) -> *mut core::ffi::c_void {
    #[cfg(feature = "frame_pointers")]
    {
        let frame = thread.arch.sp as *const X86ContextSwitchFrame;
        (*frame).rbp as *mut core::ffi::c_void
    }
    #[cfg(not(feature = "frame_pointers"))]
    {
        core::ptr::null_mut()
    }
}

/// Switch from old thread to new thread
///
/// # Arguments
///
/// * `old_thread` - The thread being switched from
/// * `new_thread` - The thread being switched to
///
/// # Safety
///
/// This function must only be called from within the scheduler with valid thread pointers.
/// Interrupts should be disabled.
pub unsafe fn arch_context_switch(
    old_thread: &mut crate::kernel::thread::Thread,
    new_thread: &mut crate::kernel::thread::Thread,
) {
    use crate::kernel::arch::amd64::registers::*;

    // Save extended register state (FPU/SSE/AVX)
    x86_extended_register_context_switch(old_thread, new_thread);

    // Handle debug register state
    x86_debug_state_context_switch(old_thread, new_thread);

    // Set the TSS SP0 value to point at the top of the new thread's stack
    mmu::x86_set_tss_sp(new_thread.stack_top());

    // Save the user fs_base register value
    let fs_base = if crate::kernel::arch::amd64::feature::g_x86_feature_fsgsbase() {
        _readfsbase_u64()
    } else {
        mmu::x86_read_msr(X86_MSR_IA32_FS_BASE)
    };
    old_thread.arch.fs_base = fs_base;

    // Reset segment selectors to prevent values from leaking between processes
    // Segment selectors get clobbered when returning from interrupts, so we reset them here
    mmu::x86_set_ds(0);
    mmu::x86_set_es(0);
    mmu::x86_set_fs(0);

    // Handle GS specially - it needs to maintain the kernel GS base
    if mmu::x86_get_gs() != 0 {
        assert!(crate::kernel::arch::amd64::arch::arch_ints_disabled());
        let gs_base = crate::kernel::arch::amd64::mp::x86_get_percpu() as VAddr;
        mmu::x86_set_gs(0);
        mmu::x86_write_msr(msr::IA32_GS_BASE, gs_base as u64);
    }

    // Restore fs_base and save/restore user gs_base
    // Note: user and kernel gs_base are swapped (user is in KERNEL_GS_BASE)
    if crate::kernel::arch::amd64::feature::g_x86_feature_fsgsbase() {
        // Use the faster {rd,wr}gsbase instructions with swapgs
        let mut old_gs_base: u64 = 0;
        core::arch::asm!(
            "swapgs",
            "rdgsbase {0}",
            "wrgsbase {1}",
            "swapgs",
            inlateout(reg) old_gs_base,
            in(reg) new_thread.arch.gs_base
        );
        old_thread.arch.gs_base = old_gs_base;

        _writefsbase_u64(new_thread.arch.fs_base);
    } else {
        // Fall back to MSR access
        old_thread.arch.gs_base = mmu::x86_read_msr(msr::IA32_KERNEL_GS_BASE);
        mmu::x86_write_msr(msr::IA32_FS_BASE, new_thread.arch.fs_base as u64);
        mmu::x86_write_msr(msr::IA32_KERNEL_GS_BASE, new_thread.arch.gs_base as u64);
    }

    // Switch to the new thread's stack
    unsafe {
        x86_64_context_switch(
            &mut (old_thread.arch.sp as *const u64),
            new_thread.arch.sp as *const u64,
        );
    }
}

/// Handle debug register state during context switch
///
/// # Arguments
///
/// * `old_thread` - The thread being switched from
/// * `new_thread` - The thread being switched to
///
/// # Safety
///
/// This function must be called with valid thread pointers
unsafe fn x86_debug_state_context_switch(
    old_thread: &mut crate::kernel::thread::Thread,
    new_thread: &mut crate::kernel::thread::Thread,
) {
    use crate::kernel::arch::amd64::registers::*;

    // If the new thread has debug state, install it
    if new_thread.arch.track_debug_state {
        // Note: x86 doesn't have a global enable/disable switch for debug registers.
        // Debug registers are enabled through DR7. These are selected by userspace
        // (and filtered by the kernel) in the thread_write_state syscall.
        // Writing the thread debug state into the CPU is enough to activate it.
        x86_write_hw_debug_regs(&new_thread.arch.debug_state);
        return;
    }

    // If the old thread had debug state and the new one doesn't, disable it
    if old_thread.arch.track_debug_state {
        x86_disable_debug_state();
    }
}

// External assembly functions
extern "C" {
    fn x86_extended_register_init_state(state: *mut core::ffi::c_void);
    fn x86_extended_register_context_switch(
        old_thread: &mut crate::kernel::thread::Thread,
        new_thread: &mut crate::kernel::thread::Thread,
    );
    fn x86_write_hw_debug_regs(state: &X86DebugState);
    fn x86_disable_debug_state();
    fn x86_64_context_switch(oldsp: *mut *const u64, newsp: *const u64);
    fn _readfsbase_u64() -> u64;
    fn _writefsbase_u64(base: u64);
}

// MSR constants
const X86_MSR_IA32_FS_BASE: u32 = 0xC000_0100;
const X86_MSR_IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;