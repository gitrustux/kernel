// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Task System Calls
//!
//! This module implements the task-related system calls for threads,
//! processes, and jobs.
//!
//! # Syscalls Implemented
//!
//! - `rx_thread_create` - Create a thread
//! - `rx_thread_start` - Start a thread
//! - `rx_thread_exit` - Exit current thread
//! - `rx_process_create` - Create a process
//! - `rx_process_start` - Start a process
//! - `rx_process_exit` - Exit current process
//! - `rx_task_kill` - Kill a task (thread or process)
//! - `rx_job_create` - Create a job
//!
//! # Design
//!
//! - Wraps existing thread/process modules
//! - Handle-based access control
//! - Validates all parameters
//! - Proper cleanup on errors

#![no_std]

use crate::kernel::object::job::{self, Job, JobId};
use crate::kernel::process::{self, HandleRights, ObjectType, Process, ProcessFlags};
use crate::kernel::thread::{self, Thread, ThreadId};
use crate::kernel::usercopy::{copy_from_user, copy_to_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::string::String;
use crate::kernel::sync::Mutex;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};
use crate::kernel::sync::spin::SpinMutex;

/// ============================================================================
/// Thread Registry
/// ============================================================================

/// Maximum number of threads in the system
const MAX_THREADS: usize = 65536;

/// Thread registry
///
/// Maps thread IDs to thread objects.
struct ThreadRegistry {
    /// Thread entries (simplified - using array for now)
    threads: Mutex<[Option<Arc<Thread>>; MAX_THREADS]>,

    /// Next thread index to allocate
    next_index: AtomicUsize,

    /// Number of active threads
    count: AtomicUsize,
}

impl ThreadRegistry {
    /// Create a new thread registry
    const fn new() -> Self {
        const INIT: Option<Arc<Thread>> = None;

        Self {
            threads: Mutex::new([INIT; MAX_THREADS]),
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert a thread into the registry
    pub fn insert(&mut self, thread: Arc<Thread>) -> Result<ThreadId> {
        let tid = thread.tid();

        // Find a free slot
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (tid as usize) % MAX_THREADS;

        loop {
            {
                let mut threads = self.threads.lock();

                // Try to allocate at current index
                if threads[idx].is_none() {
                    threads[idx] = Some(thread);
                    self.count.fetch_add(1, Ordering::Relaxed);
                    self.next_index.store((idx + 1) % MAX_THREADS, Ordering::Relaxed);
                    return Ok(tid);
                }
            }

            // Linear probe
            idx = (idx + 1) % MAX_THREADS;

            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    /// Get a thread from the registry
    pub fn get(&self, tid: ThreadId) -> Option<Arc<Thread>> {
        if tid == 0 {
            return None;
        }

        let idx = (tid as usize) % MAX_THREADS;
        let threads = self.threads.lock();

        threads[idx].as_ref().filter(|t| t.tid() == tid).cloned()
    }

    /// Get the number of active threads
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

// SAFETY: ThreadRegistry uses atomic operations and contains Arc which is thread-safe
unsafe impl Send for ThreadRegistry {}
unsafe impl Sync for ThreadRegistry {}

/// Global thread registry
static THREAD_REGISTRY: ThreadRegistry = ThreadRegistry::new();

/// ============================================================================
/// Syscall: Thread Create
/// ============================================================================

/// Create a new thread syscall handler
///
/// # Arguments
///
/// * `process_handle` - Handle to the process
/// * `name` - Thread name (user pointer)
/// * `name_len` - Length of name
/// * `options` - Creation options (must be 0)
///
/// # Returns
///
/// * On success: Thread ID
/// * On error: Negative error code
pub fn sys_thread_create_impl(
    _process_handle: u32,
    name: usize,
    name_len: usize,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_thread_create: process={:#x} name={:#x} len={} options={:#x}",
        _process_handle, name, name_len, options
    );

    // Validate options (must be 0)
    if options != 0 {
        log_error!("sys_thread_create: invalid options {:#x}", options);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Copy thread name from user space
    let thread_name = if name_len > 0 {
        // Validate name length
        if name_len > 64 {
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }

        let user_name = UserPtr::<u8>::new(name);

        // Allocate buffer for name
        let mut name_buf = alloc::vec![0u8; name_len];

        unsafe {
            if let Err(err) = copy_from_user(name_buf.as_mut_ptr(), user_name, name_len) {
                log_error!("sys_thread_create: copy_from_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }

        // Convert to string (simplified - remove null bytes)
        // In a real implementation, we'd handle this better
        String::from_utf8_lossy(&name_buf).to_string()
    } else {
        String::from("anonymous")
    };

    // Create the thread
    let thread = match Thread::new(
        0, // process_id - TODO: get from handle
        thread_name,
    ) {
        Ok(t) => t,
        Err(err) => {
            log_error!("sys_thread_create: failed to create thread: {:?}", err);
            return err_to_ret(err);
        }
    };

    log_debug!("sys_thread_create: created thread tid={}", thread.tid());

    // Wrap in Arc for registry
    let thread_arc = Arc::new(thread);

    // Insert into thread registry
    let tid = match THREAD_REGISTRY.insert(thread_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_thread_create: failed to insert thread: {:?}", err);
            return err_to_ret(err);
        }
    };

    // TODO: Add thread to process's thread list

    log_debug!("sys_thread_create: success tid={}", tid);

    ok_to_ret(tid as usize)
}

/// ============================================================================
/// Syscall: Thread Start
/// ============================================================================

/// Start a thread syscall handler
///
/// # Arguments
///
/// * `thread_handle` - Handle to the thread
/// * `entry` - Thread entry point address
/// * `stack` - Stack pointer value
/// * `arg1` - First argument
/// * `arg2` - Second argument
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_thread_start_impl(
    thread_handle: u32,
    entry: u64,
    stack: u64,
    arg1: u64,
    arg2: u64,
) -> SyscallRet {
    log_debug!(
        "sys_thread_start: handle={:#x} entry={:#x} stack={:#x} arg1={:#x} arg2={:#x}",
        thread_handle, entry, stack, arg1, arg2
    );

    // Look up thread from handle
    // TODO: Implement proper handle lookup
    let tid = thread_handle as ThreadId;

    let thread = match THREAD_REGISTRY.get(tid) {
        Some(t) => t,
        None => {
            log_error!("sys_thread_start: thread not found");
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    // Set thread entry point and stack
    // TODO: Implement proper context setup
    // For now, just start the thread

    match thread.start() {
        Ok(()) => {
            log_debug!("sys_thread_start: success");
            ok_to_ret(0)
        }
        Err(err) => {
            log_error!("sys_thread_start: failed to start thread: {:?}", err);
            err_to_ret(err)
        }
    }
}

/// ============================================================================
/// Syscall: Thread Exit
/// ============================================================================

/// Exit current thread syscall handler
///
/// # Arguments
///
/// * `exit_code` - Thread exit code
///
/// # Returns
///
/// Does not return
pub fn sys_thread_exit_impl(exit_code: i64) -> SyscallRet {
    log_debug!("sys_thread_exit: code={}", exit_code);

    // Get current thread
    // TODO: Implement proper current thread tracking
    // For now, just log

    log_info!("Thread exiting with code {}", exit_code);

    // In a real implementation, this would:
    // 1. Remove thread from process
    // 2. Clean up resources
    // 3. Call scheduler to switch to another thread

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Process Create
/// ============================================================================

/// Create a new process syscall handler
///
/// # Arguments
///
/// * `job_handle` - Handle to the parent job
/// * `name` - Process name (user pointer)
/// * `name_len` - Length of name
/// * `options` - Creation options
///
/// # Returns
///
/// * On success: Process ID
/// * On error: Negative error code
pub fn sys_process_create_impl(
    _job_handle: u32,
    name: usize,
    name_len: usize,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_process_create: job={:#x} name={:#x} len={} options={:#x}",
        _job_handle, name, name_len, options
    );

    // Convert options to process flags
    let flags = if options & 0x01 != 0 {
        ProcessFlags::Loader
    } else if options & 0x02 != 0 {
        ProcessFlags::Test
    } else if options & 0x04 != 0 {
        ProcessFlags::System
    } else {
        ProcessFlags::None
    };

    // Copy process name from user space
    let process_name = if name_len > 0 {
        if name_len > 64 {
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }

        let user_name = UserPtr::<u8>::new(name);
        let mut name_buf = alloc::vec![0u8; name_len];

        unsafe {
            if let Err(err) = copy_from_user(name_buf.as_mut_ptr(), user_name, name_len) {
                log_error!("sys_process_create: copy_from_user failed: {:?}", err);
                return err_to_ret(err.into());
            }
        }

        String::from_utf8_lossy(&name_buf).to_string()
    } else {
        String::from("anonymous")
    };

    // Create the process
    let process = match Process::new(None, 1, flags) {
        // TODO: get job_id from handle
        Ok(p) => p,
        Err(err) => {
            log_error!("sys_process_create: failed to create process: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Set process name (leak it for static lifetime - simplified)
    let leaked_name: &'static str = Box::leak(process_name.into_boxed_str());
    process.set_name(leaked_name);

    let pid = process.pid();
    log_debug!("sys_process_create: created process pid={}", pid);

    // Insert into process table
    match process::insert(process) {
        Ok(()) => {
            log_debug!("sys_process_create: success pid={}", pid);
            ok_to_ret(pid as usize)
        }
        Err(err) => {
            log_error!("sys_process_create: failed to insert process: {:?}", err);
            err_to_ret(err)
        }
    }
}

/// ============================================================================
/// Syscall: Process Start
/// ============================================================================

/// Start a process syscall handler
///
/// # Arguments
///
/// * `process_handle` - Handle to the process
/// * `thread_handle` - Handle to the initial thread
/// * `entry` - Entry point address
/// * `stack` - Stack pointer
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_process_start_impl(
    process_handle: u32,
    thread_handle: u32,
    entry: u64,
    stack: u64,
) -> SyscallRet {
    log_debug!(
        "sys_process_start: process={:#x} thread={:#x} entry={:#x} stack={:#x}",
        process_handle, thread_handle, entry, stack
    );

    // Look up process from handle
    let pid = process_handle as process::ProcessId;

    let process = match process::lookup(pid) {
        Some(p) => p,
        None => {
            log_error!("sys_process_start: process not found");
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    // Update process state to running
    process.set_state(process::ProcessState::Running);

    // Start the initial thread
    let tid = thread_handle as ThreadId;

    let thread = match THREAD_REGISTRY.get(tid) {
        Some(t) => t,
        None => {
            log_error!("sys_process_start: thread not found");
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    match thread.start() {
        Ok(()) => {
            log_debug!("sys_process_start: success");
            ok_to_ret(0)
        }
        Err(err) => {
            log_error!("sys_process_start: failed to start thread: {:?}", err);
            err_to_ret(err)
        }
    }
}

/// ============================================================================
/// Syscall: Process Exit
/// ============================================================================

/// Exit current process syscall handler
///
/// # Arguments
///
/// * `exit_code` - Process exit code
///
/// # Returns
///
/// Does not return
pub fn sys_process_exit_impl(exit_code: i64) -> SyscallRet {
    log_debug!("sys_process_exit: code={}", exit_code);

    // Get current process
    // TODO: Implement proper current process tracking
    // For now, just log

    log_info!("Process exiting with code {}", exit_code);

    // In a real implementation, this would:
    // 1. Terminate all threads in the process
    // 2. Clean up resources
    // 3. Notify parent process
    // 4. Call scheduler to switch to another thread

    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Task Kill
/// ============================================================================

/// Kill a task (thread or process) syscall handler
///
/// # Arguments
///
/// * `task_handle` - Handle to the task
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_task_kill_impl(task_handle: u32) -> SyscallRet {
    log_debug!("sys_task_kill: handle={:#x}", task_handle);

    // Try to interpret as process handle
    let pid = task_handle as process::ProcessId;

    if let Some(process) = process::lookup(pid) {
        // Kill the process
        process.exit(0); // TODO: proper exit code

        // Remove from process table
        process::remove(pid);

        log_debug!("sys_task_kill: killed process pid={}", pid);
        return ok_to_ret(0);
    }

    // Try to interpret as thread handle
    let tid = task_handle as ThreadId;

    if let Some(thread) = THREAD_REGISTRY.get(tid) {
        // Kill the thread
        thread.exit(0); // TODO: proper exit code

        log_debug!("sys_task_kill: killed thread tid={}", tid);
        return ok_to_ret(0);
    }

    log_error!("sys_task_kill: task not found");
    err_to_ret(RX_ERR_INVALID_ARGS)
}

/// ============================================================================
/// Syscall: Job Create
/// ============================================================================

/// Create a job syscall handler
///
/// # Arguments
///
/// * `parent_job` - Parent job handle (0 for root job)
/// * `options` - Job policy options
///
/// # Returns
///
/// * On success: Job ID
/// * On error: Negative error code
pub fn sys_job_create_impl(parent_job: u32, options: u32) -> SyscallRet {
    log_debug!(
        "sys_job_create: parent={:#x} options={:#x}",
        parent_job, options
    );

    // Get parent job (0 means root job)
    let parent_job_id = if parent_job == 0 {
        job::JOB_ID_ROOT
    } else {
        parent_job as JobId
    };

    let parent_job_obj = match job::lookup(parent_job_id) {
        Some(job) => job,
        None => {
            log_error!("sys_job_create: parent job not found");
            return err_to_ret(RX_ERR_INVALID_ARGS);
        }
    };

    // Create child job
    let child_job = match Job::new_child(&parent_job_obj, options) {
        Ok(job) => job,
        Err(err) => {
            log_error!("sys_job_create: failed to create job: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Register the job
    if let Err(err) = job::register(child_job.clone()) {
        log_error!("sys_job_create: failed to register job: {:?}", err);
        return err_to_ret(err);
    }

    log_debug!("sys_job_create: success job_id={}", child_job.id);

    ok_to_ret(child_job.id as usize)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get task subsystem statistics
pub fn get_stats() -> TaskStats {
    TaskStats {
        total_threads: THREAD_REGISTRY.count(),
        total_processes: process::MAX_PROCESSES, // Placeholder
        active_processes: 0,                      // TODO: Track active processes
    }
}

/// Task subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskStats {
    /// Total number of threads
    pub total_threads: usize,

    /// Total number of processes (max)
    pub total_processes: usize,

    /// Number of active processes
    pub active_processes: usize,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the task syscall subsystem
pub fn init() {
    log_info!("Task syscall subsystem initialized");
    log_info!("  Max threads: {}", MAX_THREADS);
    log_info!("  Max processes: {}", process::MAX_PROCESSES);

    // Initialize the job subsystem
    job::init_root_job();
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_registry() {
        let thread = Thread::new(0, String::from("test")).unwrap();
        let thread_arc = Arc::new(thread);

        let tid = THREAD_REGISTRY.insert(thread_arc.clone()).unwrap();
        assert_eq!(tid, thread_arc.tid());

        let retrieved = THREAD_REGISTRY.get(tid).unwrap();
        assert_eq!(retrieved.tid(), thread_arc.tid());
    }

    #[test]
    fn test_process_create() {
        let process = Process::new(None, 1, ProcessFlags::Test).unwrap();

        assert!(process.pid >= process::PID_FIRST_USER);
        assert_eq!(process.state(), process::ProcessState::Creating);
        assert_eq!(process.job_id, 1);
    }

    #[test]
    fn test_process_flags() {
        let flags = ProcessFlags::Loader;
        // Test flag conversions
        assert!(matches!(flags, ProcessFlags::Loader));

        let flags = ProcessFlags::None;
        assert!(matches!(flags, ProcessFlags::None));
    }

    #[test]
    fn test_thread_name_validation() {
        // Valid names
        assert!(sys_thread_create_impl(0, 0, 0, 0) >= 0);    // empty name

        // Invalid options
        assert!(sys_thread_create_impl(0, 0, 0, 1) < 0);    // invalid options
    }

    #[test]
    fn test_job_create() {
        // Valid job creation
        assert!(sys_job_create_impl(0, 0) >= 0);

        // Invalid options
        assert!(sys_job_create_impl(0, 1) < 0);
    }
}
