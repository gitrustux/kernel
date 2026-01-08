// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Thread Management
//!
//! This module provides thread management for the Rustux kernel.
//! Threads represent schedulable units of execution that belong to processes.
//!
//! # Design
//!
//! - Each thread has a unique thread ID (TID)
//! - Threads have states (New, Ready, Running, Blocked, Dying, Dead)
//! - Per-architecture thread context for register save/restore
//! - Each thread has a kernel stack with guard page
//! - Threads are scheduled by the scheduler
//!
//! # Thread States
//!
//! ```text
//! New -> Ready -> Running -> Blocked -> Ready -> Running
//!                 |           |                      |
//!                 v           v                      v
//!               Dying -------> Dead <-----------------
//! ```
//!
//! # Usage
//!
//! ```rust
//! // Create a new thread
//! let thread = Thread::new(process, entry_point, arg, priority)?;
//!
//! // Start the thread
//! thread.start()?;
//!
//! // Yield to scheduler
//! Thread::yield_current();
//! ```

#![no_std]

use crate::kernel::vm::stacks::*;
use crate::kernel::vm::aspace::*;
use crate::kernel::vm::{VmError, Result};
use crate::kernel::arch::arch_traits::*;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use crate::kernel::sync::spin::SpinMutex;
use crate::kernel::sync::Mutex;
use alloc::vec::Vec;
use alloc::string::String;
use crate::rustux::types::*;

// Import logging macros
use crate::{log_debug, log_info, log_trace};

/// ============================================================================
/// Thread ID
/// ============================================================================

/// Thread ID type
pub type ThreadId = u64;

/// Invalid thread ID
pub const TID_INVALID: ThreadId = 0;

/// Global thread ID allocator
static TID_ALLOCATOR: TidAllocator = TidAllocator::new();

/// Thread ID allocator
struct TidAllocator {
    next: AtomicU64,
}

impl TidAllocator {
    const fn new() -> Self {
        Self {
            next: AtomicU64::new(1), // TID 0 is reserved/invalid
        }
    }

    fn allocate(&self) -> ThreadId {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

/// ============================================================================
/// Thread State
/// ============================================================================

/// Thread state
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    /// Thread has been created but not yet started
    New = 0,

    /// Thread is ready to run (in run queue)
    Ready = 1,

    /// Thread is currently running on a CPU
    Running = 2,

    /// Thread is blocked (waiting for I/O, mutex, etc.)
    Blocked = 3,

    /// Thread is dying (being terminated)
    Dying = 4,

    /// Thread is dead (terminated and cleaned up)
    Dead = 5,
}

impl ThreadState {
    /// Check if thread can be scheduled
    pub const fn is_schedulable(self) -> bool {
        matches!(self, Self::Ready | Self::Running)
    }

    /// Check if thread is alive
    pub const fn is_alive(self) -> bool {
        matches!(self, Self::New | Self::Ready | Self::Running | Self::Blocked | Self::Dying)
    }

    /// Check if thread has exited
    pub const fn has_exited(self) -> bool {
        matches!(self, Self::Dying | Self::Dead)
    }
}

/// Reason for thread being blocked
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// Not blocked
    None = 0,

    /// Blocked on I/O
    Io = 1,

    /// Blocked on a mutex/lock
    Lock = 2,

    /// Blocked on a channel read
    ChannelRead = 3,

    /// Blocked on a channel write
    ChannelWrite = 4,

    /// Blocked on sleep/timer
    Sleep = 5,

    /// Blocked on join
    Join = 6,
}

/// ============================================================================
/// Thread Priority
/// ============================================================================

/// Thread priority (0 = lowest, 255 = highest)
pub type ThreadPriority = u8;

/// Default thread priority
pub const PRIORITY_DEFAULT: ThreadPriority = 128;

/// Idle thread priority (lowest)
pub const PRIORITY_IDLE: ThreadPriority = 0;

/// Real-time thread priority (highest)
pub const PRIORITY_REALTIME: ThreadPriority = 255;

/// ============================================================================
/// CPU Affinity
/// ============================================================================

/// CPU affinity mask
pub type CpuMask = u64;

/// Run on any CPU
pub const CPU_MASK_ALL: CpuMask = 0xFFFF_FFFF_FFFF_FFFF;

/// Run on CPU 0 only
pub const CPU_MASK_CPU0: CpuMask = 0x1;

/// ============================================================================
/// Thread Structure
/// ============================================================================

/// Thread structure
///
/// Represents a schedulable thread of execution.
pub struct Thread {
    /// Thread ID
    pub tid: ThreadId,

    /// Thread state
    pub state: Mutex<ThreadState>,

    /// Thread priority
    pub priority: ThreadPriority,

    /// CPU affinity mask
    pub cpu_affinity: CpuMask,

    /// Block reason (if blocked)
    pub block_reason: Mutex<BlockReason>,

    /// Kernel stack
    pub stack: Mutex<Option<KernelStack>>,

    /// Process ID this thread belongs to
    pub pid: Mutex<Option<u64>>,

    /// Parent thread ID (for thread creation)
    pub parent_tid: Mutex<Option<ThreadId>>,

    /// Return code (when thread exits)
    pub return_code: Mutex<Option<rx_status_t>>,

    /// Architecture-specific context
    pub arch_context: Mutex<Option<ArchContext>>,

    /// Architecture-specific data (for debugger access)
    pub arch: ArchData,

    /// Thread name (for debugging)
    pub name: Mutex<Option<&'static str>>,

    /// Joinable flag
    pub joinable: AtomicBool,

    /// Join wait queue (threads waiting for this thread to finish)
    pub join_waiters: Mutex<Vec<ThreadId>>,

    /// Reference count
    pub ref_count: AtomicU64,

    /// Entry point (virtual address)
    pub entry_point: VAddr,

    /// Argument to pass to entry point
    pub entry_arg: usize,
}

/// Architecture-specific thread context
///
/// This wraps the per-architecture context representation.
#[repr(C)]
#[derive(Debug)]
pub struct ArchContext {
    /// Architecture-specific context data
    #[cfg(target_arch = "aarch64")]
    pub inner: crate::kernel::arch::arm64::RiscvIframe,

    #[cfg(target_arch = "x86_64")]
    pub inner: crate::kernel::arch::amd64::X86ThreadStateGeneralRegs,

    #[cfg(target_arch = "riscv64")]
    pub inner: crate::kernel::arch::riscv64::RiscvIframe,
}

/// Architecture-specific data for debugger access
///
/// This contains pointers to suspended register state.
#[repr(C)]
#[derive(Debug)]
pub struct ArchData {
    /// Pointer to suspended general registers
    #[cfg(target_arch = "aarch64")]
    pub suspended_general_regs: *const crate::kernel::arch::arm64::RiscvIframe,

    /// Stack pointer (for aarch64)
    #[cfg(target_arch = "aarch64")]
    pub sp: VAddr,

    /// Stack guard value (for aarch64)
    #[cfg(target_arch = "aarch64")]
    pub stack_guard: u64,

    /// Unsafe stack pointer (for aarch64 with safe_stack feature)
    #[cfg(all(target_arch = "aarch64", feature = "safe_stack"))]
    pub unsafe_sp: VAddr,

    /// Current per-CPU pointer (for aarch64)
    #[cfg(target_arch = "aarch64")]
    pub current_percpu_ptr: *mut u8,

    /// Track debug state flag (for aarch64)
    #[cfg(target_arch = "aarch64")]
    pub track_debug_state: bool,

    /// Debug state (for aarch64)
    #[cfg(target_arch = "aarch64")]
    pub debug_state: crate::kernel::arch::arm64::thread::Arm64DebugState,

    /// FP state pointer (for aarch64)
    #[cfg(target_arch = "aarch64")]
    pub fpstate: *mut core::ffi::c_void,

    /// Pointer to suspended general registers
    #[cfg(target_arch = "x86_64")]
    pub suspended_general_regs: *const crate::kernel::arch::amd64::X86ThreadStateGeneralRegs,

    /// Stack pointer (for x86_64)
    #[cfg(target_arch = "x86_64")]
    pub sp: VAddr,

    /// FS base register (for x86_64)
    #[cfg(target_arch = "x86_64")]
    pub fs_base: VAddr,

    /// GS base register (for x86_64)
    #[cfg(target_arch = "x86_64")]
    pub gs_base: VAddr,

    /// Debug state (for x86_64)
    #[cfg(target_arch = "x86_64")]
    pub debug_state: crate::kernel::arch::amd64::registers::X86DebugState,

    /// Track debug state flag (for x86_64)
    #[cfg(target_arch = "x86_64")]
    pub track_debug_state: bool,

    /// Extended register state (FXSAVE area) for x86_64
    #[cfg(target_arch = "x86_64")]
    pub extended_register_state: *mut core::ffi::c_void,

    /// Pointer to suspended general registers
    #[cfg(target_arch = "riscv64")]
    pub suspended_general_regs: *const crate::kernel::arch::riscv64::RiscvIframe,
}

impl ArchData {
    /// Create a new (empty) ArchData
    pub const fn new() -> Self {
        Self {
            #[cfg(target_arch = "aarch64")]
            suspended_general_regs: core::ptr::null(),
            #[cfg(target_arch = "aarch64")]
            sp: 0,
            #[cfg(target_arch = "aarch64")]
            stack_guard: 0,
            #[cfg(all(target_arch = "aarch64", feature = "safe_stack"))]
            unsafe_sp: 0,
            #[cfg(target_arch = "aarch64")]
            current_percpu_ptr: core::ptr::null_mut(),
            #[cfg(target_arch = "aarch64")]
            track_debug_state: false,
            #[cfg(target_arch = "aarch64")]
            debug_state: crate::kernel::arch::arm64::thread::Arm64DebugState::default(),
            #[cfg(target_arch = "aarch64")]
            fpstate: core::ptr::null_mut(),
            #[cfg(target_arch = "x86_64")]
            suspended_general_regs: core::ptr::null(),
            #[cfg(target_arch = "x86_64")]
            sp: 0,
            #[cfg(target_arch = "x86_64")]
            fs_base: 0,
            #[cfg(target_arch = "x86_64")]
            gs_base: 0,
            #[cfg(target_arch = "x86_64")]
            debug_state: unsafe { core::mem::zeroed() },
            #[cfg(target_arch = "x86_64")]
            track_debug_state: false,
            #[cfg(target_arch = "x86_64")]
            extended_register_state: core::ptr::null_mut(),
            #[cfg(target_arch = "riscv64")]
            suspended_general_regs: core::ptr::null(),
        }
    }
}

impl Thread {
    /// Create a new thread
    ///
    /// # Arguments
    ///
    /// * `entry_point` - Virtual address where thread should start execution
    /// * `arg` - Argument passed to thread entry point
    /// * `stack_top` - Top of kernel stack for this thread
    /// * `priority` - Thread scheduling priority
    pub fn new(
        entry_point: VAddr,
        arg: usize,
        stack_top: VAddr,
        priority: ThreadPriority,
    ) -> Result<Self> {
        let tid = TID_ALLOCATOR.allocate();

        let thread = Self {
            tid,
            state: Mutex::new(ThreadState::New),
            priority,
            cpu_affinity: CPU_MASK_ALL,
            block_reason: Mutex::new(BlockReason::None),
            stack: Mutex::new(None),
            pid: Mutex::new(None),
            parent_tid: Mutex::new(None),
            return_code: Mutex::new(None),
            arch_context: Mutex::new(None),
            arch: ArchData::new(),
            name: Mutex::new(None),
            joinable: AtomicBool::new(false),
            join_waiters: Mutex::new(Vec::new()),
            ref_count: AtomicU64::new(1),
            entry_point,
            entry_arg: arg,
        };

        // Initialize architecture-specific context
        {
            let mut ctx_guard = thread.arch_context.lock();
            *ctx_guard = Some(ArchContext {
                #[cfg(target_arch = "aarch64")]
                inner: unsafe { core::mem::zeroed() },

                #[cfg(target_arch = "x86_64")]
                inner: unsafe { core::mem::zeroed() },

                #[cfg(target_arch = "riscv64")]
                inner: unsafe { core::mem::zeroed() },
            });
        }

        // Initialize context with entry point and stack
        // This would call into arch-specific code
        // For now, leave it zero-initialized

        log_debug!(
            "Created thread: tid={} entry={:#x} priority={}",
            tid,
            entry_point,
            priority
        );

        Ok(thread)
    }

    /// Create a new kernel thread
    pub fn new_kernel(
        entry_point: extern "C" fn(usize) -> !,
        arg: usize,
        priority: ThreadPriority,
    ) -> Result<Self> {
        // Allocate kernel stack
        let stack = alloc_kernel_stack(0)?; // Owner ID 0 = kernel

        Self::new(
            entry_point as VAddr,
            arg,
            stack.top,
            priority,
        )
    }

    /// Get the thread ID
    pub fn tid(&self) -> ThreadId {
        self.tid
    }

    /// Get the thread state
    pub fn state(&self) -> ThreadState {
        *self.state.lock()
    }

    /// Set the thread state
    pub fn set_state(&self, new_state: ThreadState) {
        *self.state.lock() = new_state;
    }

    /// Get the thread priority
    pub fn priority(&self) -> ThreadPriority {
        self.priority
    }

    /// Set the thread priority
    pub fn set_priority(&mut self, priority: ThreadPriority) {
        // Would need to update scheduler runqueue
        self.priority = priority;
    }

    /// Associate a process with this thread
    pub fn set_process(&self, pid: u64) {
        *self.pid.lock() = Some(pid);
    }

    /// Get the process ID
    pub fn pid(&self) -> Option<u64> {
        *self.pid.lock()
    }

    /// Get the kernel stack
    pub fn stack(&self) -> Option<KernelStack> {
        self.stack.lock().clone()
    }

    /// Set the kernel stack
    pub fn set_stack(&self, stack: KernelStack) {
        *self.stack.lock() = Some(stack);
    }

    /// Get the top of the kernel stack
    pub fn stack_top(&self) -> VAddr {
        self.stack.lock().as_ref().map(|s| s.top).unwrap_or(0)
    }

    /// Start the thread
    ///
    /// Transitions thread from New to Ready state.
    pub fn start(&self) -> Result {
        let mut state = self.state.lock();

        if *state != ThreadState::New {
            return Err(VmError::BadState);
        }

        *state = ThreadState::Ready;
        drop(state);

        // Add to scheduler run queue
        // scheduler::runqueue_add(self.tid);

        log_debug!("Started thread: tid={}", self.tid);

        Ok(())
    }

    /// Exit the current thread
    ///
    /// # Arguments
    ///
    /// * `code` - Return code
    pub fn exit(&self, code: rx_status_t) {
        // Set return code
        *self.return_code.lock() = Some(code);

        // Transition to dying state
        self.set_state(ThreadState::Dying);

        // Wake up any joiners
        let waiters = self.join_waiters.lock();
        for waiter in waiters.iter() {
            // scheduler::wake(*waiter);
        }

        log_debug!("Thread exiting: tid={} code={}", self.tid, code);
    }

    /// Block the current thread
    ///
    /// # Arguments
    ///
    /// * `reason` - Reason for blocking
    pub fn block(&self, reason: BlockReason) {
        *self.block_reason.lock() = reason;
        self.set_state(ThreadState::Blocked);

        // Remove from scheduler run queue
        // scheduler::runqueue_remove(self.tid);
    }

    /// Wake up a blocked thread
    pub fn wake(&self) {
        if self.state() == ThreadState::Blocked {
            *self.block_reason.lock() = BlockReason::None;
            self.set_state(ThreadState::Ready);

            // Add to scheduler run queue
            // scheduler::runqueue_add(self.tid);
        }
    }

    /// Yield the current thread
    ///
    /// Voluntarily give up the CPU to other threads.
    pub fn yield_current() {
        // Get current thread
        // let current = scheduler::current_thread();
        // scheduler::yield_current();
        log_trace!("Thread yielding");
    }

    /// Set the thread name
    pub fn set_name(&self, name: &'static str) {
        *self.name.lock() = Some(name);
    }

    /// Get the thread name
    pub fn name(&self) -> Option<&'static str> {
        *self.name.lock()
    }

    /// Increment reference count
    pub fn ref_inc(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference
    pub fn ref_dec(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Relaxed) == 1
    }

    /// Get the current reference count
    pub fn ref_count(&self) -> u64 {
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Create a dummy thread for FPU state operations
    ///
    /// This is used when saving/restoring FPU state without an actual thread context.
    /// Returns a mutable reference to a static dummy thread.
    pub fn dummy_thread() -> &'static mut Self {
        // Use a static dummy thread for FPU operations
        // This is unsafe but necessary for low-level FPU state management
        static mut DUMMY_THREAD: Thread = Thread {
            tid: 0,
            state: Mutex::new(ThreadState::Ready),
            priority: PRIORITY_DEFAULT,
            cpu_affinity: CPU_MASK_ALL,
            block_reason: Mutex::new(BlockReason::None),
            stack: Mutex::new(None),
            pid: Mutex::new(None),
            parent_tid: Mutex::new(None),
            return_code: Mutex::new(None),
            arch_context: Mutex::new(None),
            arch: ArchData::new(),
            name: Mutex::new(Some("<dummy>")),
            joinable: AtomicBool::new(false),
            join_waiters: Mutex::new(Vec::new()),
            ref_count: AtomicU64::new(1),
            entry_point: 0,
            entry_arg: 0,
        };

        unsafe { &mut DUMMY_THREAD }
    }
}

/// ============================================================================
/// Thread Registry
/// ============================================================================

/// Maximum number of threads in the system
const MAX_THREADS: usize = 65536;

/// Thread registry for lookup by ID
struct ThreadRegistry {
    /// Thread entries indexed by TID
    entries: Mutex<BTreeMap<ThreadId, Arc<Thread>>>,

    /// Number of active threads
    count: AtomicUsize,
}

impl ThreadRegistry {
    const fn new() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
            count: AtomicUsize::new(0),
        }
    }

    fn insert(&self, thread: Arc<Thread>) {
        let mut entries = self.entries.lock();
        entries.insert(thread.tid, thread);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn get(&self, tid: ThreadId) -> Option<Arc<Thread>> {
        let entries = self.entries.lock();
        entries.get(&tid).cloned()
    }

    fn remove(&self, tid: ThreadId) {
        let mut entries = self.entries.lock();
        if entries.remove(&tid).is_some() {
            self.count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

// SAFETY: ThreadRegistry uses atomic operations and contains Arc which is thread-safe
unsafe impl Send for ThreadRegistry {}
unsafe impl Sync for ThreadRegistry {}

/// Global thread registry
static THREAD_REGISTRY: ThreadRegistry = ThreadRegistry::new();

/// ============================================================================
/// Thread Blocking Primitives
/// ============================================================================

/// Block reason for object waiting
#[derive(Debug, Clone, Copy)]
pub enum WaitBlockReason {
    /// Waiting on an object handle
    ObjectWait {
        /// Handle being waited on
        handle: u32,
        /// Signal mask
        signals: u64,
    },
}

/// Get the current thread ID
///
/// This uses architecture-specific methods to get the current thread.
pub fn current_thread_id() -> ThreadId {
    unsafe {
        #[cfg(target_arch = "aarch64")]
        {
            let thread_ptr = crate::kernel::arch::arm64::get_current_thread();
            if thread_ptr.is_null() {
                return TID_INVALID;
            }
            (*thread_ptr).tid
        }

        #[cfg(target_arch = "x86_64")]
        {
            // For x86_64, we would use per-CPU data to get the current thread
            // For now, return a stub value
            // TODO: Implement proper thread retrieval from per-CPU data
            TID_INVALID
        }

        #[cfg(target_arch = "riscv64")]
        {
            // For RISC-V, we would use tp register
            // TODO: Implement proper thread retrieval
            TID_INVALID
        }
    }
}

/// Get the current thread
///
/// Returns a reference to the current thread.
pub fn get_current_thread() -> Option<Arc<Thread>> {
    let tid = current_thread_id();
    if tid == TID_INVALID {
        return None;
    }
    THREAD_REGISTRY.get(tid)
}

/// Get a thread by ID
pub fn get_thread_by_id(tid: ThreadId) -> Option<Arc<Thread>> {
    THREAD_REGISTRY.get(tid)
}

/// Register a thread in the global registry
pub fn register_thread(thread: Arc<Thread>) {
    THREAD_REGISTRY.insert(thread);
}

/// Unregister a thread from the global registry
pub fn unregister_thread(tid: ThreadId) {
    THREAD_REGISTRY.remove(tid);
}

/// Get the number of active threads
pub fn thread_count() -> usize {
    THREAD_REGISTRY.count()
}

/// Block the current thread
///
/// # Arguments
///
/// * `reason` - Block reason
///
/// # Note
///
/// This function blocks the current thread. It should be called when
/// the thread needs to wait for an event (I/O completion, signal, etc.).
pub fn block_current_thread(reason: BlockReason) {
    if let Some(thread) = get_current_thread() {
        thread.block(reason);
        // TODO: Invoke scheduler to switch to another thread
        log_debug!("Thread {} blocked: {:?}", thread.tid, reason);
    }
}

/// Wake up a thread by ID
///
/// # Arguments
///
/// * `tid` - Thread ID to wake
///
/// Returns true if the thread was woken up, false if not found or not blocked.
pub fn wake_thread(tid: ThreadId) -> bool {
    if let Some(thread) = get_thread_by_id(tid) {
        if thread.state() == ThreadState::Blocked {
            thread.wake();
            // TODO: Invoke scheduler to add thread to run queue
            log_debug!("Thread {} woken up", tid);
            return true;
        }
    }
    false
}

/// Block the current thread on an object wait
///
/// # Arguments
///
/// * `handle` - Handle to wait on
/// * `signals` - Signal mask to wait for
pub fn block_on_object_wait(handle: u32, signals: u64) {
    // This is called by the wait queue implementation
    // The wait queue has already added the thread to its internal list
    // Now we just need to block the thread
    block_current_thread(BlockReason::Lock); // Use Lock as generic block reason
}

/// Wake up threads waiting on an object
///
/// # Arguments
///
/// * `handle` - Handle that was signaled
/// * `signals` - Signals that were set
///
/// Returns the number of threads woken up
pub fn wake_object_waiters(handle: u32, signals: u64) -> usize {
    // This would iterate through all threads and check if they're waiting
    // on this handle. For now, return 0 as a placeholder.
    // TODO: Implement proper waiter lookup
    log_debug!("Would wake waiters on handle {:#x} with signals {:#x}", handle, signals);
    0
}

/// Get the current thread's handle table
///
/// Returns a reference to the handle table for the current thread.
/// This is used by syscalls to access handles.
pub fn current_thread_handle_table() -> &'static crate::kernel::object::handle::HandleTable {
    // TODO: Implement proper thread-local handle table lookup
    // For now, use a static stub
    use crate::kernel::object::handle::HandleTable;

    static mut STUB_TABLE: HandleTable = unsafe { HandleTable::new() };

    unsafe { &STUB_TABLE }
}

/// ============================================================================
/// Thread Local Storage (TLS)
/// ============================================================================

/// Thread-local storage pointer
///
/// Points to thread-local data for the current thread.
static mut TLS_POINTER: *mut u8 = core::ptr::null_mut();

/// Get the TLS pointer for the current thread
pub fn tls_get() -> *mut u8 {
    unsafe { TLS_POINTER }
}

/// Set the TLS pointer for the current thread
///
/// # Safety
///
/// Must be called with a valid pointer to thread-local data.
pub unsafe fn tls_set(ptr: *mut u8) {
    TLS_POINTER = ptr;
}

/// ============================================================================
/// Thread Creation Helper
/// ============================================================================

/// Entry point wrapper for new threads
///
/// This function is called when a new thread starts execution.
/// It sets up TLS and calls the actual entry point.
extern "C" fn thread_entry_wrapper() -> ! {
    // Get current thread (would need scheduler support)
    // let thread = scheduler::current_thread();

    // For now, we need a way to retrieve the current thread's entry point
    // In a real implementation, the scheduler would set up a thread pointer
    // that we can use to retrieve the current thread struct.

    // As a temporary measure, we'll need to store this in a way that
    // the architecture-specific context switch code can access it.
    // For now, we'll use a simple approach: the entry point and arg
    // should be passed via the stack or registers during context switch.

    // TODO: Implement proper current thread retrieval via:
    // 1. Per-CPU data (e.g., GS base on x86_64)
    // 2. Thread-local storage
    // 3. Scheduler API

    // Placeholder: retrieve entry point from current thread
    // This will be properly implemented when the scheduler is complete
    if let Some(thread) = get_current_thread() {
        let entry_point = thread.entry_point;
        let arg = thread.entry_arg;

        // Convert entry_point to function pointer and call it
        let entry_fn: extern "C" fn(usize) -> ! = unsafe {
            core::mem::transmute(entry_point)
        };

        entry_fn(arg);
    }

    // If we can't get the current thread, halt
    loop {
        core::hint::spin_loop();
    }
}

/// ============================================================================
// Module Initialization
// ============================================================================

/// Initialize the thread subsystem
pub fn init() {
    log_info!("Thread subsystem initialized");

    // Create idle thread for CPU 0
    // Thread::new_idle(0);
}

// ============================================================================
// LK Compatibility Types and Functions
// ============================================================================

/// Kernel stack type (LK compatibility)
pub type kstack_t = *mut u8;

/// Preempt current thread (LK compatibility stub)
pub fn thread_preempt() {
    // TODO: Implement thread preemption
}

/// Check if thread is signaled (LK compatibility stub)
pub fn thread_is_signaled(_thread: Option<&Thread>) -> bool {
    // TODO: Implement signal checking
    false
}

/// Process pending signals (LK compatibility stub)
pub fn thread_process_pending_signals() {
    // TODO: Implement signal processing
}

/// Secondary CPU early init (LK compatibility stub)
pub fn thread_secondary_cpu_init_early() {
    // TODO: Implement secondary CPU early thread init
}

/// Yield the current thread (LK compatibility stub)
pub fn yield_current() {
    // TODO: Implement thread yielding
    // For now, just hint to the CPU
    core::hint::spin_loop();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_state() {
        assert!(!ThreadState::New.is_schedulable());
        assert!(ThreadState::Ready.is_schedulable());
        assert!(ThreadState::Running.is_schedulable());
        assert!(!ThreadState::Blocked.is_schedulable());
        assert!(!ThreadState::Dying.is_schedulable());
        assert!(!ThreadState::Dead.is_schedulable());

        assert!(ThreadState::New.is_alive());
        assert!(ThreadState::Blocked.is_alive());
        assert!(!ThreadState::Dead.is_alive());
    }

    #[test]
    fn test_tid_allocator() {
        let tid1 = TID_ALLOCATOR.allocate();
        let tid2 = TID_ALLOCATOR.allocate();
        let tid3 = TID_ALLOCATOR.allocate();

        assert!(tid1 > 0);
        assert!(tid2 > tid1);
        assert!(tid3 > tid2);
    }

    #[test]
    fn test_cpu_mask() {
        assert_eq!(CPU_MASK_CPU0, 0x1);
        assert!(CPU_MASK_ALL & CPU_MASK_CPU0 != 0);
    }

    #[test]
    fn test_priority() {
        assert!(PRIORITY_IDLE < PRIORITY_DEFAULT);
        assert!(PRIORITY_DEFAULT < PRIORITY_REALTIME);
    }
}
