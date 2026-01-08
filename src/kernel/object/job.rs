// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Job Objects
//!
//! Jobs are containers for processes and other jobs, forming a hierarchical
//! tree structure. They are used for resource accounting and policy enforcement.
//!
//! # Design
//!
//! - **Hierarchical**: Jobs form a tree with a single root job
//! - **Policy**: Jobs can enforce CPU, memory, and job policies
//! - **Accounting**: Track resource usage across all child processes
//! - **Lifecycle**: Jobs are created explicitly and destroyed when all children exit
//!
//! # Usage
//!
//! ```rust
//! let root_job = Job::new_root();
//! let child_job = Job::new_child(&root_job, 0)?;
//! let process = Process::new(Some(child_job.clone()), ...)?;
//! ```


use crate::kernel::sync::Mutex;
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// ============================================================================
/// Job ID
/// ============================================================================

/// Job identifier
pub type JobId = u64;

/// Invalid job ID
pub const JOB_ID_INVALID: JobId = 0;

/// Root job ID
pub const JOB_ID_ROOT: JobId = 1;

/// Next job ID counter
static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(JOB_ID_ROOT + 1);

/// Allocate a new job ID
fn alloc_job_id() -> JobId {
    NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed)
}

/// ============================================================================
/// Job Policy
/// ============================================================================

/// Job policy for controlling child process behavior
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobPolicy {
    /// No special policy
    None = 0,

    /// Basic policy (minimal restrictions)
    Basic = 1,

    /// Restrict VMO creation (no new VMOs)
    NoNewVmos = 1 << 1,

    /// Restrict channel creation
    NoNewChannels = 1 << 2,

    /// Restrict event creation
    NoNewEvents = 1 << 3,

    /// Restrict socket creation
    NoNewSockets = 1 << 4,

    /// Restrict process creation
    NoNewProcesses = 1 << 5,

    /// Restrict thread creation
    NoNewThreads = 1 << 6,

    /// Kill all processes when job is closed
    KillOnClose = 1 << 7,

    /// Allow profiling
    AllowProfile = 1 << 8,

    /// Allow debugging
    AllowDebug = 1 << 9,
}

impl JobPolicy {
    /// Create from raw value
    pub const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Basic,
            _ => Self::None,
        }
    }

    /// Convert to raw flags
    pub fn to_flags(self) -> u32 {
        let mut flags = 0u32;

        if self.contains(JobPolicy::NoNewVmos) {
            flags |= JobPolicy::NoNewVmos as u32;
        }
        if self.contains(JobPolicy::NoNewChannels) {
            flags |= JobPolicy::NoNewChannels as u32;
        }
        if self.contains(JobPolicy::NoNewEvents) {
            flags |= JobPolicy::NoNewEvents as u32;
        }
        if self.contains(JobPolicy::NoNewSockets) {
            flags |= JobPolicy::NoNewSockets as u32;
        }
        if self.contains(JobPolicy::NoNewProcesses) {
            flags |= JobPolicy::NoNewProcesses as u32;
        }
        if self.contains(JobPolicy::NoNewThreads) {
            flags |= JobPolicy::NoNewThreads as u32;
        }
        if self.contains(JobPolicy::KillOnClose) {
            flags |= JobPolicy::KillOnClose as u32;
        }
        if self.contains(JobPolicy::AllowProfile) {
            flags |= JobPolicy::AllowProfile as u32;
        }
        if self.contains(JobPolicy::AllowDebug) {
            flags |= JobPolicy::AllowDebug as u32;
        }

        flags
    }

    /// Create from flags
    pub fn from_flags(flags: u32) -> Self {
        let mut policy = JobPolicy::None;

        if flags & (JobPolicy::NoNewVmos as u32) != 0 {
            policy |= JobPolicy::NoNewVmos;
        }
        if flags & (JobPolicy::NoNewChannels as u32) != 0 {
            policy |= JobPolicy::NoNewChannels;
        }
        if flags & (JobPolicy::NoNewEvents as u32) != 0 {
            policy |= JobPolicy::NoNewEvents;
        }
        if flags & (JobPolicy::NoNewSockets as u32) != 0 {
            policy |= JobPolicy::NoNewSockets;
        }
        if flags & (JobPolicy::NoNewProcesses as u32) != 0 {
            policy |= JobPolicy::NoNewProcesses;
        }
        if flags & (JobPolicy::NoNewThreads as u32) != 0 {
            policy |= JobPolicy::NoNewThreads;
        }
        if flags & (JobPolicy::KillOnClose as u32) != 0 {
            policy |= JobPolicy::KillOnClose;
        }
        if flags & (JobPolicy::AllowProfile as u32) != 0 {
            policy |= JobPolicy::AllowProfile;
        }
        if flags & (JobPolicy::AllowDebug as u32) != 0 {
            policy |= JobPolicy::AllowDebug;
        }

        policy
    }

    /// Check if policy contains a flag
    pub fn contains(self, other: JobPolicy) -> bool {
        (self.to_flags() & other as u32) != 0
    }
}

impl core::ops::BitOr for JobPolicy {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_flags(self.to_flags() | rhs.to_flags())
    }
}

impl core::ops::BitAnd for JobPolicy {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_flags(self.to_flags() & rhs.to_flags())
    }
}

impl core::ops::BitOrAssign for JobPolicy {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

/// ============================================================================
/// Resource Limits
/// ============================================================================

/// Resource limits for a job
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ResourceLimits {
    /// Maximum memory in bytes (0 = no limit)
    pub max_memory: u64,

    /// Maximum CPU time in nanoseconds (0 = no limit)
    pub max_cpu_time: u64,

    /// Maximum number of processes (0 = no limit)
    pub max_processes: u32,

    /// Maximum number of threads (0 = no limit)
    pub max_threads: u32,

    /// Maximum number of VMOs (0 = no limit)
    pub max_vmos: u32,

    /// Maximum number of channels (0 = no limit)
    pub max_channels: u32,
}

impl ResourceLimits {
    /// Create unlimited resource limits
    pub const fn unlimited() -> Self {
        Self {
            max_memory: 0,
            max_cpu_time: 0,
            max_processes: 0,
            max_threads: 0,
            max_vmos: 0,
            max_channels: 0,
        }
    }

    /// Create strict resource limits
    pub const fn strict() -> Self {
        Self {
            max_memory: 16 * 1024 * 1024,  // 16 MB
            max_cpu_time: 1_000_000_000,     // 1 second
            max_processes: 10,
            max_threads: 100,
            max_vmos: 100,
            max_channels: 50,
        }
    }

    /// Check if a resource limit is set
    pub const fn is_limited(&self) -> bool {
        self.max_memory != 0
            || self.max_cpu_time != 0
            || self.max_processes != 0
            || self.max_threads != 0
            || self.max_vmos != 0
            || self.max_channels != 0
    }
}

/// ============================================================================
/// Job Statistics
/// ============================================================================

/// Job resource usage statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct JobStats {
    /// Current memory usage in bytes
    pub memory_used: u64,

    /// Total CPU time consumed (nanoseconds)
    pub cpu_time: u64,

    /// Number of processes in this job
    pub process_count: u32,

    /// Number of threads in this job
    pub thread_count: u32,

    /// Number of child jobs
    pub child_count: u32,

    /// Number of VMOs created
    pub vmo_count: u32,

    /// Number of channels created
    pub channel_count: u32,
}

impl JobStats {
    /// Create empty statistics
    pub const fn new() -> Self {
        Self {
            memory_used: 0,
            cpu_time: 0,
            process_count: 0,
            thread_count: 0,
            child_count: 0,
            vmo_count: 0,
            channel_count: 0,
        }
    }
}

/// ============================================================================
/// Job Structure
/// ============================================================================

/// Job object
///
/// Jobs represent a container for processes and other jobs. They are used
/// for resource accounting and policy enforcement.
pub struct Job {
    /// Job ID
    pub id: JobId,

    /// Parent job (None for root job)
    pub parent: Option<*const Job>,

    /// Child jobs
    pub children: Mutex<BTreeSet<JobId>>,

    /// Processes in this job
    pub processes: Mutex<BTreeSet<u32>>, // Process IDs

    /// Job policy
    pub policy: Mutex<JobPolicy>,

    /// Resource limits
    pub limits: Mutex<ResourceLimits>,

    /// Resource usage statistics
    pub stats: Mutex<JobStats>,

    /// Whether this job has been killed
    pub killed: AtomicBool,

    /// Reference count
    pub ref_count: AtomicUsize,
}

unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    /// Create the root job
    pub fn new_root() -> Arc<Self> {
        Arc::new(Self {
            id: JOB_ID_ROOT,
            parent: None,
            children: Mutex::new(BTreeSet::new()),
            processes: Mutex::new(BTreeSet::new()),
            policy: Mutex::new(JobPolicy::None),
            limits: Mutex::new(ResourceLimits::unlimited()),
            stats: Mutex::new(JobStats::new()),
            killed: AtomicBool::new(false),
            ref_count: AtomicUsize::new(1),
        })
    }

    /// Create a new child job
    pub fn new_child(parent: &Arc<Job>, options: u32) -> Result<Arc<Self>> {
        // Check if parent allows creating new jobs
        let parent_policy = *parent.policy.lock();
        if parent_policy.contains(JobPolicy::NoNewProcesses) {
            return Err(RX_ERR_ACCESS_DENIED);
        }

        // Create child job
        let id = alloc_job_id();
        let policy = JobPolicy::from_flags(options);

        let job = Arc::new(Self {
            id,
            parent: Some(Arc::as_ptr(parent)),
            children: Mutex::new(BTreeSet::new()),
            processes: Mutex::new(BTreeSet::new()),
            policy: Mutex::new(policy),
            limits: Mutex::new(ResourceLimits::unlimited()),
            stats: Mutex::new(JobStats::new()),
            killed: AtomicBool::new(false),
            ref_count: AtomicUsize::new(1),
        });

        // Add to parent's children
        parent.children.lock().insert(id);
        parent.stats.lock().child_count += 1;

        Ok(job)
    }

    /// Get the job ID
    pub fn job_id(&self) -> JobId {
        self.id
    }

    /// Check if this job has been killed
    pub fn is_killed(&self) -> bool {
        self.killed.load(Ordering::Acquire)
    }

    /// Kill this job and all children
    pub fn kill(&self) {
        self.killed.store(true, Ordering::Release);

        // TODO: Kill all child processes recursively
        // This would require iterating through all processes
        // and sending them a kill signal

        // For now, just mark the job as killed
    }

    /// Add a process to this job
    pub fn add_process(&self, pid: u32) -> Result {
        // Check if job has been killed
        if self.is_killed() {
            return Err(RX_ERR_BAD_STATE);
        }

        // Check policy
        let policy = *self.policy.lock();
        if policy.contains(JobPolicy::NoNewProcesses) {
            return Err(RX_ERR_ACCESS_DENIED);
        }

        // Check limits
        let mut stats = self.stats.lock();
        let limits = *self.limits.lock();

        if limits.max_processes != 0 && stats.process_count >= limits.max_processes {
            return Err(RX_ERR_NO_RESOURCES);
        }

        self.processes.lock().insert(pid);
        stats.process_count += 1;

        Ok(())
    }

    /// Remove a process from this job
    pub fn remove_process(&self, pid: u32) {
        if self.processes.lock().remove(&pid) {
            self.stats.lock().process_count -= 1;
        }
    }

    /// Add a child job
    pub fn add_child(&self, child_id: JobId) {
        self.children.lock().insert(child_id);
        self.stats.lock().child_count += 1;
    }

    /// Remove a child job
    pub fn remove_child(&self, child_id: JobId) {
        if self.children.lock().remove(&child_id) {
            self.stats.lock().child_count -= 1;
        }
    }

    /// Check if an operation is allowed by policy
    pub fn check_policy(&self, operation: JobPolicy) -> bool {
        let policy = *self.policy.lock();

        // Check this job's policy
        if policy.contains(operation) {
            return false;
        }

        // Check parent policies recursively
        if let Some(parent_ptr) = self.parent {
            unsafe {
                if !parent_ptr.as_ref().unwrap().check_policy(operation) {
                    return false;
                }
            }
        }

        true
    }

    /// Set resource limits
    pub fn set_limits(&self, limits: ResourceLimits) -> Result {
        *self.limits.lock() = limits;
        Ok(())
    }

    /// Get resource limits
    pub fn get_limits(&self) -> ResourceLimits {
        *self.limits.lock()
    }

    /// Get statistics
    pub fn get_stats(&self) -> JobStats {
        *self.stats.lock()
    }

    /// Update memory usage
    pub fn update_memory(&self, delta: i64) -> Result {
        let mut stats = self.stats.lock();
        let limits = *self.limits.lock();

        let new_usage = if delta >= 0 {
            stats.memory_used + delta as u64
        } else {
            stats.memory_used.saturating_sub((-delta) as u64)
        };

        // Check limit
        if limits.max_memory != 0 && new_usage > limits.max_memory {
            return Err(RX_ERR_NO_RESOURCES);
        }

        stats.memory_used = new_usage;
        Ok(())
    }

    /// Record CPU time usage
    pub fn add_cpu_time(&self, delta_ns: u64) {
        let mut stats = self.stats.lock();
        let limits = *self.limits.lock();

        stats.cpu_time += delta_ns;

        // Check if CPU time limit exceeded
        if limits.max_cpu_time != 0 && stats.cpu_time > limits.max_cpu_time {
            // Kill the job when CPU limit exceeded
            self.kill();
        }
    }

    /// Increment reference count
    pub fn ref_inc(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement reference count
    ///
    /// Returns true if this was the last reference.
    pub fn ref_dec(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::Release) == 1
    }
}

/// ============================================================================
/// Job Registry
/// ============================================================================

/// Maximum number of jobs in the system
const MAX_JOBS: usize = 65536;

/// Job registry
struct JobRegistry {
    /// Job entries
    entries: [Option<Arc<Job>>; MAX_JOBS],

    /// Next index to check
    next_index: AtomicUsize,

    /// Number of active jobs
    count: AtomicUsize,
}

impl JobRegistry {
    const fn new() -> Self {
        const INIT: Option<Arc<Job>> = None;
        Self {
            entries: [INIT; MAX_JOBS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    fn insert(&mut self, job: Arc<Job>) -> Result {
        let id = job.id;
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (id as usize) % MAX_JOBS;

        loop {
            if self.entries[idx].is_none() {
                self.entries[idx] = Some(job);
                self.count.fetch_add(1, Ordering::Relaxed);
                self.next_index.store((idx + 1) % MAX_JOBS, Ordering::Relaxed);
                return Ok(());
            }

            idx = (idx + 1) % MAX_JOBS;
            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    fn get(&self, id: JobId) -> Option<Arc<Job>> {
        let idx = (id as usize) % MAX_JOBS;
        self.entries[idx].as_ref().filter(|j| j.id == id).cloned()
    }

    fn remove(&mut self, id: JobId) -> Result {
        let idx = (id as usize) % MAX_JOBS;
        if self.entries[idx].is_some() {
            self.entries[idx] = None;
            self.count.fetch_sub(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RX_ERR_NOT_FOUND)
        }
    }

    fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global job registry
static JOB_REGISTRY: Mutex<JobRegistry> = Mutex::new(JobRegistry::new());

/// ============================================================================
/// Root Job
/// ============================================================================

/// Root job for the system
static mut ROOT_JOB: Option<Arc<Job>> = None;

/// Initialize the root job
pub fn init_root_job() {
    unsafe {
        if ROOT_JOB.is_none() {
            ROOT_JOB = Some(Job::new_root());
            // Don't register root job in registry - it's always accessible
        }
    }
}

/// Get the root job
pub fn get_root_job() -> Option<&'static Arc<Job>> {
    unsafe { ROOT_JOB.as_ref() }
}

/// ============================================================================
/// Public API
/// ============================================================================

/// Look up a job by ID
pub fn lookup(id: JobId) -> Option<Arc<Job>> {
    if id == JOB_ID_ROOT {
        get_root_job().cloned()
    } else {
        JOB_REGISTRY.lock().get(id)
    }
}

/// Register a job
pub fn register(job: Arc<Job>) -> Result {
    JOB_REGISTRY.lock().insert(job)
}

/// Unregister a job
pub fn unregister(id: JobId) -> Result {
    JOB_REGISTRY.lock().remove(id)
}

/// Get the number of active jobs
pub fn count() -> usize {
    JOB_REGISTRY.lock().count()
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_policy() {
        let policy = JobPolicy::NoNewVmos | JobPolicy::NoNewChannels;

        assert!(policy.contains(JobPolicy::NoNewVmos));
        assert!(policy.contains(JobPolicy::NoNewChannels));
        assert!(!policy.contains(JobPolicy::NoNewEvents));
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits::strict();

        assert!(limits.max_memory > 0);
        assert!(limits.max_processes > 0);
        assert!(limits.is_limited());

        let unlimited = ResourceLimits::unlimited();
        assert!(!unlimited.is_limited());
    }

    #[test]
    fn test_job_creation() {
        let root = Job::new_root();
        assert_eq!(root.id(), JOB_ID_ROOT);
        assert!(root.parent.is_none());
        assert!(!root.is_killed());

        let child = Job::new_child(&root, 0);
        assert!(child.is_ok());
        let child = child.unwrap();
        assert_ne!(child.id(), JOB_ID_ROOT);
        assert!(child.parent.is_some());
    }

    #[test]
    fn test_job_policy_enforcement() {
        let root = Job::new_root();
        let policy = JobPolicy::NoNewProcesses;

        let child = Job::new_child(&root, policy.to_flags());
        assert!(child.is_ok());

        // Adding process to restricted job should fail
        let job = child.unwrap();
        let result = job.add_process(1234);
        assert!(result.is_err());
    }

    #[test]
    fn test_job_kill() {
        let root = Job::new_root();
        let child = Job::new_child(&root, 0).unwrap();

        assert!(!child.is_killed());
        child.kill();
        assert!(child.is_killed());

        // Can't add process to killed job
        let result = child.add_process(1234);
        assert!(result.is_err());
    }
}
