// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Process Management
//!
//! This module provides process management for the Rustux kernel.
//! Processes represent isolated execution contexts with their own address spaces.
//!
//! # Design
//!
//! - Each process has a unique process ID (PID)
//! - Processes have address spaces (VM mappings)
//! - Processes contain threads
//! - Processes have handle tables for capability-based security
//! - Processes are organized in a hierarchy with parent/child relationships
//! - Processes belong to jobs for resource accounting
//!
//! # Process States
//!
//! ```text
//! Creating -> Running -> Exiting -> Dead
//! ```
//!
//! # Usage
//!
//! ```rust
//! // Create a new process
//! let process = Process::new(parent_pid, job_id)?;
//!
//! // Add a thread to the process
//! process.add_thread(thread_id)?;
//!
//! // Get the process's address space
//! let aspace = process.address_space();
//! ```


use crate::kernel::vm::aspace::*;
use crate::kernel::vm::Result;
use crate::rustux::types::*;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// Import logging macros
use crate::{log_debug, log_info};
use crate::kernel::sync::{Mutex, MutexGuard};
use crate::kernel::sync::spin::SpinMutex;
use alloc::vec::Vec;

/// ============================================================================
/// Process ID
/// ============================================================================

/// Process ID type
pub type ProcessId = u64;

/// Invalid process ID
pub const PID_INVALID: ProcessId = 0;

/// Kernel process ID (PID 0)
pub const PID_KERNEL: ProcessId = 0;

/// First user process ID
pub const PID_FIRST_USER: ProcessId = 1;

/// Global process ID allocator
static PID_ALLOCATOR: PidAllocator = PidAllocator::new();

/// Process ID allocator
struct PidAllocator {
    next: AtomicU64,
}

impl PidAllocator {
    const fn new() -> Self {
        Self {
            next: AtomicU64::new(PID_FIRST_USER), // Start at 1
        }
    }

    fn allocate(&self) -> ProcessId {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

/// ============================================================================
/// Process State
/// ============================================================================

/// Process state
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Process is being created
    Creating = 0,

    /// Process is running (has at least one thread)
    Running = 1,

    /// Process is exiting (threads terminating)
    Exiting = 2,

    /// Process is dead (all threads terminated, resources freed)
    Dead = 3,
}

impl ProcessState {
    /// Check if process is alive
    pub const fn is_alive(self) -> bool {
        matches!(self, Self::Creating | Self::Running | Self::Exiting)
    }

    /// Check if process has exited
    pub const fn has_exited(self) -> bool {
        matches!(self, Self::Exiting | Self::Dead)
    }
}

/// ============================================================================
/// Job ID
/// ============================================================================

/// Job ID type
///
/// Jobs are containers for processes that provide resource accounting.
pub type JobId = u64;

/// Invalid job ID
pub const JOB_ID_INVALID: JobId = 0;

/// Root job ID
pub const JOB_ID_ROOT: JobId = 1;

/// ============================================================================
/// Handle
/// ============================================================================

/// Handle type
///
/// Handles are capabilities that reference kernel objects.
pub type Handle = u32;

/// Invalid handle
pub const HANDLE_INVALID: Handle = 0;

/// Handle rights
///
/// Rights control what operations can be performed on an object.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleRights {
    /// None
    None = 0,

    /// Read
    Read = 1 << 0,

    /// Write
    Write = 1 << 1,

    /// Execute
    Execute = 1 << 2,

    /// Duplicate
    Duplicate = 1 << 3,

    /// Transfer
    Transfer = 1 << 4,

    /// All rights
    All = 0xFFFF_FFFF,
}

impl HandleRights {
    /// Check if has right
    pub const fn has(self, right: Self) -> bool {
        (self as u32) & (right as u32) != 0
    }

    /// Add a right
    pub const fn add(self, right: Self) -> Self {
        unsafe { core::mem::transmute((self as u32) | (right as u32)) }
    }

    /// Remove a right
    pub const fn remove(self, right: Self) -> Self {
        unsafe { core::mem::transmute((self as u32) & !(right as u32)) }
    }
}

/// ============================================================================
/// Handle Table Entry
/// ============================================================================

/// Handle table entry
#[repr(C)]
#[derive(Debug, Clone)]
pub struct HandleEntry {
    /// Handle value
    pub handle: Handle,

    /// Object ID (what the handle refers to)
    pub object_id: u64,

    /// Handle rights
    pub rights: HandleRights,

    /// Type of object
    pub object_type: ObjectType,
}

/// Object type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    /// Process
    Process = 1,

    /// Thread
    Thread = 2,

    /// VMO (Virtual Memory Object)
    Vmo = 3,

    /// VMAR (Virtual Memory Address Region)
    Vmar = 4,

    /// Channel
    Channel = 5,

    /// Event
    Event = 6,

    /// Event pair
    EventPair = 7,

    /// Job
    Job = 8,

    /// Timer
    Timer = 9,

    /// Unknown
    Unknown = 0xFFFF,
}

/// ============================================================================
/// Handle Table
/// ============================================================================

/// Maximum handles per process
pub const MAX_HANDLES: usize = 256;

/// Handle table
///
/// Manages handles for a single process.
pub struct HandleTable {
    /// Handle entries
    handles: Mutex<[Option<HandleEntry>; MAX_HANDLES]>,

    /// Next handle index to allocate
    next_index: core::sync::atomic::AtomicUsize,

    /// Number of active handles
    count: core::sync::atomic::AtomicUsize,
}

impl HandleTable {
    /// Create a new handle table
    pub const fn new() -> Self {
        Self {
            handles: Mutex::new([const { None }; MAX_HANDLES]),
            next_index: core::sync::atomic::AtomicUsize::new(0),
            count: core::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Allocate a new handle
    pub fn alloc(&self, object_id: u64, rights: HandleRights, object_type: ObjectType) -> Result<Handle> {
        // Find a free slot
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = start;

        loop {
            {
                let mut handles = self.handles.lock();

                // Try to allocate at current index
                if handles[idx].is_none() {
                    let handle = ((idx + 1) as u32); // Handles start at 1

                    handles[idx] = Some(HandleEntry {
                        handle,
                        object_id,
                        rights,
                        object_type,
                    });

                    self.count.fetch_add(1, Ordering::Relaxed);
                    self.next_index.store((idx + 1) % MAX_HANDLES, Ordering::Relaxed);

                    return Ok(handle);
                }
            }

            idx = (idx + 1) % MAX_HANDLES;

            if idx == start {
                return Err(crate::kernel::vm::VmError::NoMemory);
            }
        }
    }

    /// Free a handle
    pub fn free(&self, handle: Handle) -> Result {
        let idx = (handle as usize) - 1;

        if idx >= MAX_HANDLES {
            return Err(crate::kernel::vm::VmError::InvalidArgs);
        }

        let mut handles = self.handles.lock();

        if handles[idx].is_none() {
            return Err(crate::kernel::vm::VmError::NotFound);
        }

        handles[idx] = None;
        self.count.fetch_sub(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get a handle entry
    pub fn get(&self, handle: Handle) -> Result<HandleEntry> {
        let idx = (handle as usize) - 1;

        if idx >= MAX_HANDLES {
            return Err(crate::kernel::vm::VmError::InvalidArgs);
        }

        let handles = self.handles.lock();

        if let Some(ref entry) = handles[idx] {
            Ok(entry.clone())
        } else {
            Err(crate::kernel::vm::VmError::NotFound)
        }
    }

    /// Check if a handle has specific rights
    pub fn check_rights(&self, handle: Handle, rights: HandleRights) -> Result {
        let entry = self.get(handle)?;

        if !entry.rights.has(rights) {
            return Err(crate::kernel::vm::VmError::PermissionDenied);
        }

        Ok(())
    }

    /// Get the number of active handles
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// ============================================================================
/// Process Structure
/// ============================================================================

/// Maximum number of threads per process
pub const MAX_THREADS_PER_PROCESS: usize = 1024;

/// Process structure
///
/// Represents a process in the system.
pub struct Process {
    /// Process ID
    pub pid: ProcessId,

    /// Process state
    pub state: Mutex<ProcessState>,

    /// Address space
    pub address_space: Mutex<Option<AddressSpace>>,

    /// Handle table
    pub handles: HandleTable,

    /// Threads in this process
    pub threads: Mutex<Vec<crate::kernel::thread::ThreadId>>,

    /// Parent process ID
    pub parent_pid: Mutex<Option<ProcessId>>,

    /// Job ID
    pub job_id: JobId,

    /// Return code (when process exits)
    pub return_code: Mutex<Option<rx_status_t>>,

    /// Process name (for debugging)
    pub name: Mutex<Option<&'static str>>,

    /// Reference count
    pub ref_count: AtomicU64,

    /// Creation flags
    pub flags: ProcessFlags,
}

/// Process creation flags
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessFlags {
    /// None
    None = 0,

    /// Created with loader stub
    Loader = 1 << 0,

    /// Created for testing
    Test = 1 << 1,

    /// Created as system process
    System = 1 << 2,
}

impl Process {
    /// Create a new process
    pub fn new(parent_pid: Option<ProcessId>, job_id: JobId, flags: ProcessFlags) -> Result<Self> {
        let pid = PID_ALLOCATOR.allocate();

        log_debug!(
            "Creating process: pid={} parent={:?} job={}",
            pid,
            parent_pid,
            job_id
        );

        Ok(Self {
            pid,
            state: Mutex::new(ProcessState::Creating),
            address_space: Mutex::new(None),
            handles: HandleTable::new(),
            threads: Mutex::new(Vec::new()),
            parent_pid: Mutex::new(parent_pid),
            job_id,
            return_code: Mutex::new(None),
            name: Mutex::new(None),
            ref_count: AtomicU64::new(1),
            flags,
        })
    }

    /// Get the process ID
    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    /// Get the process state
    pub fn state(&self) -> ProcessState {
        *self.state.lock()
    }

    /// Set the process state
    pub fn set_state(&self, new_state: ProcessState) {
        *self.state.lock() = new_state;
    }

    /// Set the address space
    pub fn set_address_space(&self, aspace: AddressSpace) {
        *self.address_space.lock() = Some(aspace);
    }

    /// Get the address space (lock must be held by caller)
    pub fn address_space(&self) -> MutexGuard<Option<AddressSpace>> {
        self.address_space.lock()
    }

    /// Add a thread to the process
    pub fn add_thread(&self, tid: crate::kernel::thread::ThreadId) -> Result {
        let mut threads = self.threads.lock();

        if threads.len() >= MAX_THREADS_PER_PROCESS {
            return Err(crate::kernel::vm::VmError::NoMemory);
        }

        threads.push(tid);
        log_debug!("Thread added to process: pid={} tid={}", self.pid, tid);

        Ok(())
    }

    /// Remove a thread from the process
    pub fn remove_thread(&self, tid: crate::kernel::thread::ThreadId) {
        let mut threads = self.threads.lock();
        if let Some(pos) = threads.iter().position(|&t| t == tid) {
            threads.remove(pos);
            log_debug!("Thread removed from process: pid={} tid={}", self.pid, tid);
        }
    }

    /// Get the number of threads
    pub fn thread_count(&self) -> usize {
        self.threads.lock().len()
    }

    /// Get the parent process ID
    pub fn parent_pid(&self) -> Option<ProcessId> {
        *self.parent_pid.lock()
    }

    /// Exit the process
    pub fn exit(&self, code: rx_status_t) {
        // Set return code
        *self.return_code.lock() = Some(code);

        // Transition to exiting state
        self.set_state(ProcessState::Exiting);

        // Wake up any waiters
        // This would be implemented with a wait queue

        log_debug!("Process exiting: pid={} code={}", self.pid, code);
    }

    /// Set the process name
    pub fn set_name(&self, name: &'static str) {
        *self.name.lock() = Some(name);
    }

    /// Get the process name
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

    /// Allocate a handle
    pub fn alloc_handle(&self, object_id: u64, rights: HandleRights, object_type: ObjectType) -> Result<Handle> {
        self.handles.alloc(object_id, rights, object_type)
    }

    /// Free a handle
    pub fn free_handle(&self, handle: Handle) -> Result {
        self.handles.free(handle)
    }

    /// Get a handle entry
    pub fn get_handle(&self, handle: Handle) -> Result<HandleEntry> {
        self.handles.get(handle)
    }
}

/// ============================================================================
/// Global Process Table
/// ============================================================================

/// Maximum number of processes in the system
pub const MAX_PROCESSES: usize = 4096;

/// Global process table
static mut PROCESS_TABLE: [Option<Process>; MAX_PROCESSES] = [const { None }; MAX_PROCESSES];

/// Process table lock
static PROCESS_TABLE_LOCK: AtomicBool = AtomicBool::new(false);

/// Initialize the process subsystem
pub fn init() {
    log_info!("Process subsystem initialized");
    log_info!("  Max processes: {}", MAX_PROCESSES);
    log_info!("  Max threads per process: {}", MAX_THREADS_PER_PROCESS);
    log_info!("  Max handles per process: {}", MAX_HANDLES);
}

/// Look up a process by PID
pub fn lookup(pid: ProcessId) -> Option<&'static Process> {
    if pid == PID_INVALID {
        return None;
    }

    let idx = (pid as usize) % MAX_PROCESSES;

    unsafe {
        PROCESS_TABLE[idx].as_ref().filter(|p| p.pid == pid)
    }
}

/// Look up a process by PID (mutable)
pub fn lookup_mut(pid: ProcessId) -> Option<&'static mut Process> {
    if pid == PID_INVALID {
        return None;
    }

    let idx = (pid as usize) % MAX_PROCESSES;

    unsafe {
        PROCESS_TABLE[idx].as_mut().filter(|p| p.pid == pid)
    }
}

/// Insert a process into the table
pub fn insert(process: Process) -> Result {
    let pid = process.pid;
    let idx = (pid as usize) % MAX_PROCESSES;

    // Acquire lock
    while PROCESS_TABLE_LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }

    unsafe {
        if PROCESS_TABLE[idx].is_some() {
            PROCESS_TABLE_LOCK.store(false, Ordering::Release);
            return Err(crate::kernel::vm::VmError::Busy);
        }

        PROCESS_TABLE[idx] = Some(process);
    }

    // Release lock
    PROCESS_TABLE_LOCK.store(false, Ordering::Release);

    log_debug!("Process inserted into table: pid={}", pid);

    Ok(())
}

/// Remove a process from the table
pub fn remove(pid: ProcessId) -> Option<Process> {
    if pid == PID_INVALID {
        return None;
    }

    let idx = (pid as usize) % MAX_PROCESSES;

    // Acquire lock
    while PROCESS_TABLE_LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }

    let result = unsafe {
        core::mem::replace(&mut PROCESS_TABLE[idx], None)
    };

    // Release lock
    PROCESS_TABLE_LOCK.store(false, Ordering::Release);

    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid_allocator() {
        let pid1 = PID_ALLOCATOR.allocate();
        let pid2 = PID_ALLOCATOR.allocate();
        let pid3 = PID_ALLOCATOR.allocate();

        assert!(pid1 >= PID_FIRST_USER);
        assert!(pid2 > pid1);
        assert!(pid3 > pid2);
    }

    #[test]
    fn test_process_state() {
        assert!(ProcessState::Creating.is_alive());
        assert!(ProcessState::Running.is_alive());
        assert!(ProcessState::Exiting.has_exited());
        assert!(ProcessState::Dead.has_exited());
    }

    #[test]
    fn test_handle_rights() {
        let rights = HandleRights::Read;

        assert!(rights.has(HandleRights::Read));
        assert!(!rights.has(HandleRights::Write));

        let combined = rights.add(HandleRights::Write);
        assert!(combined.has(HandleRights::Read));
        assert!(combined.has(HandleRights::Write));
    }

    #[test]
    fn test_process_creation() {
        let process = Process::new(Some(PID_KERNEL), JOB_ID_ROOT, ProcessFlags::Test);

        assert!(process.is_ok());

        let process = process.unwrap();
        assert!(process.pid >= PID_FIRST_USER);
        assert_eq!(process.parent_pid(), Some(PID_KERNEL));
        assert_eq!(process.job_id, JOB_ID_ROOT);
        assert_eq!(process.thread_count(), 0);
    }
}
