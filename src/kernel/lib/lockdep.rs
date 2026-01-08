// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Lock Dependency Tracking
//!
//! This module provides lockdep (lock dependency) tracking to detect
//! potential deadlocks and circular lock dependencies at runtime.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Maximum number of lock classes
const MAX_LOCK_CLASSES: usize = 256;

/// Lock class ID type
pub type LockClassId = u16;

/// Lock order counter
static LOCK_ORDER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Acquired lock entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AcquiredLockEntry {
    /// Lock class ID
    pub id: LockClassId,
    /// Lock order (when this lock was acquired)
    pub order: u64,
}

impl AcquiredLockEntry {
    /// Create a new acquired lock entry
    pub fn new(id: LockClassId) -> Self {
        Self {
            id,
            order: LOCK_ORDER_COUNTER.fetch_add(1, Ordering::AcqRel),
        }
    }

    /// Get the lock class ID
    pub fn id(&self) -> LockClassId {
        self.id
    }

    /// Get the lock order
    pub fn order(&self) -> u64 {
        self.order
    }
}

/// Lock result types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockResult {
    /// Lock acquisition is valid
    Ok,
    /// Lock order violation detected
    OrderViolation,
    /// Circular dependency detected
    CircularDependency,
    /// Invalid lock class
    InvalidLockClass,
}

impl LockResult {
    /// Convert lock result to string
    pub fn to_string(self) -> &'static str {
        match self {
            LockResult::Ok => "Ok",
            LockResult::OrderViolation => "Order violation",
            LockResult::CircularDependency => "Circular dependency",
            LockResult::InvalidLockClass => "Invalid lock class",
        }
    }
}

/// Lock class state
pub struct LockClassState {
    /// Lock class name
    pub name: String,
    /// Lock class ID
    pub id: LockClassId,
    /// Set of locks this lock depends on (must acquire after)
    pub dependency_set: Vec<LockClassId>,
    /// Connected set pointer (for cycle detection)
    pub connected_set: Option<usize>,
}

impl LockClassState {
    /// Create a new lock class state
    pub fn new(name: &str, id: LockClassId) -> Self {
        Self {
            name: name.to_string(),
            id,
            dependency_set: Vec::new(),
            connected_set: None,
        }
    }

    /// Get the lock class name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the dependency set
    pub fn dependency_set(&self) -> &[LockClassId] {
        &self.dependency_set
    }

    /// Get the connected set index
    pub fn connected_set(&self) -> Option<usize> {
        self.connected_set
    }
}

/// Thread-local lock state
#[repr(C)]
#[derive(Debug)]
pub struct ThreadLockState {
    /// List of currently held locks
    acquired_locks: Vec<AcquiredLockEntry>,
    /// Current lock depth
    lock_depth: usize,
}

impl ThreadLockState {
    /// Create a new thread lock state
    pub fn new() -> Self {
        Self {
            acquired_locks: Vec::new(),
            lock_depth: 0,
        }
    }

    /// Add an acquired lock
    pub fn push_lock(&mut self, entry: AcquiredLockEntry) {
        self.acquired_locks.push(entry);
        self.lock_depth += 1;
    }

    /// Remove the most recently acquired lock
    pub fn pop_lock(&mut self) -> Option<AcquiredLockEntry> {
        if self.lock_depth > 0 {
            self.lock_depth -= 1;
            self.acquired_locks.pop()
        } else {
            None
        }
    }

    /// Get the list of acquired locks
    pub fn acquired_locks(&self) -> &[AcquiredLockEntry] {
        &self.acquired_locks
    }

    /// Get the current lock depth
    pub fn lock_depth(&self) -> usize {
        self.lock_depth
    }

    /// Find a lock entry by ID
    pub fn find_lock(&self, id: LockClassId) -> Option<&AcquiredLockEntry> {
        self.acquired_locks.iter().find(|e| e.id() == id)
    }
}

/// Global lock dependency tracking state
static LOCK_CLASSES: Mutex<BTreeMap<LockClassId, LockClassState>> = Mutex::new(BTreeMap::new());

/// Next lock class ID
static NEXT_LOCK_CLASS_ID: AtomicUsize = AtomicUsize::new(0);

/// Loop detection enabled flag
static LOOP_DETECTION_ENABLED: AtomicBool = AtomicBool::new(false);

/// Initialize lock dependency tracking
pub fn init() {
    println!("LockDep: Lock dependency tracking initialized");
}

/// Register a new lock class
///
/// # Arguments
///
/// * `name` - Lock class name
///
/// # Returns
///
/// Lock class ID
pub fn register_lock_class(name: &str) -> LockClassId {
    let id = NEXT_LOCK_CLASS_ID.fetch_add(1, Ordering::AcqRel) as LockClassId;

    if id >= MAX_LOCK_CLASSES as u16 {
        panic!("LockDep: Too many lock classes registered");
    }

    let state = LockClassState::new(name, id);

    let mut classes = LOCK_CLASSES.lock();
    classes.insert(id, state);

    println!("LockDep: Registered lock class '{}' with ID {}", name, id);

    id
}

/// Validate lock acquisition
///
/// # Arguments
///
/// * `state` - Thread lock state
/// * `id` - Lock class ID to acquire
///
/// # Returns
///
/// LockResult indicating if acquisition is valid
pub fn validate_lock_acquire(state: &ThreadLockState, id: LockClassId) -> LockResult {
    let classes = LOCK_CLASSES.lock();

    // Check if lock class exists
    if !classes.contains_key(&id) {
        return LockResult::InvalidLockClass;
    }

    // Check for circular dependencies
    for entry in state.acquired_locks() {
        if let Some(class) = classes.get(&entry.id()) {
            // Check if the new lock depends on any held lock
            // This would create a circular dependency
            if class.dependency_set().contains(&id) {
                return LockResult::CircularDependency;
            }
        }
    }

    LockResult::Ok
}

/// Record a lock dependency
///
/// # Arguments
///
/// * `held_id` - Currently held lock class ID
/// * `new_id` - Newly acquired lock class ID
pub fn record_lock_dependency(held_id: LockClassId, new_id: LockClassId) {
    let mut classes = LOCK_CLASSES.lock();

    if let Some(class) = classes.get_mut(&held_id) {
        // Add the dependency if not already present
        if !class.dependency_set.contains(&new_id) {
            class.dependency_set.push(new_id);

            // Trigger loop detection if enabled
            if LOOP_DETECTION_ENABLED.load(Ordering::Acquire) {
                drop(classes);
                trigger_loop_detection();
            }
        }
    }
}

/// Enable loop detection
pub fn enable_loop_detection() {
    LOOP_DETECTION_ENABLED.store(true, Ordering::Release);
    println!("LockDep: Loop detection enabled");
}

/// Trigger loop detection pass
pub fn trigger_loop_detection() {
    // TODO: Implement loop detection algorithm
    println!("LockDep: Loop detection triggered");
}

/// Perform a loop detection pass
pub fn loop_detection_pass() {
    let classes = LOCK_CLASSES.lock();

    println!("LockDep: Performing loop detection pass...");

    // TODO: Implement proper cycle detection using DFS or Union-Find
    // For now, just check for trivial cycles

    for (id, class) in classes.iter() {
        for dep_id in class.dependency_set() {
            if let Some(dep_class) = classes.get(dep_id) {
                // Check if the dependency depends on us (direct cycle)
                if dep_class.dependency_set().contains(id) {
                    system_circular_lock_dependency_detected(*id);
                    return;
                }
            }
        }
    }

    println!("LockDep: Loop detection complete - no cycles found");
}

/// Get the current thread's lock state
///
/// # Returns
///
/// Thread lock state for the current thread
pub fn get_thread_lock_state() -> ThreadLockState {
    // TODO: Get from thread-local storage
    ThreadLockState::new()
}

/// Initialize thread lock state
pub fn init_thread_lock_state() -> ThreadLockState {
    ThreadLockState::new()
}

/// Dump lock class state for debugging
pub fn dump_lock_class_state() {
    let classes = LOCK_CLASSES.lock();

    println!("Lock class states:");
    for (_id, class) in classes.iter() {
        println!("  {} {{", class.name());
        for dep_id in class.dependency_set() {
            if let Some(dep_class) = classes.get(dep_id) {
                println!("    {}", dep_class.name());
            }
        }
        println!("  }}");
    }

    println!("\nConnected sets:");
    // TODO: Implement connected set dumping
}

/// System callback: circular lock dependency detected
fn system_circular_lock_dependency_detected(_root_id: LockClassId) {
    println!("\nRUSTUX KERNEL OOPS");
    println!("Circular lock dependency detected:");

    let classes = LOCK_CLASSES.lock();
    for (_id, class) in classes.iter() {
        println!("  {}", class.name());
    }
    println!();
}

/// System callback: lock validation error
fn system_lock_validation_error(
    _bad_entry: &AcquiredLockEntry,
    _conflicting_entry: &AcquiredLockEntry,
    _caller_address: usize,
    _caller_frame: usize,
    result: LockResult,
) {
    println!("\nRUSTUX KERNEL PANIC");
    println!("Lock validation failed:");
    println!("Reason: {}", result.to_string());
    // TODO: Print more detailed information
}

/// System callback: fatal lock violation
fn system_lock_validation_fatal(
    _lock_entry: &AcquiredLockEntry,
    _caller_address: usize,
    _caller_frame: usize,
    result: LockResult,
) -> ! {
    println!("\nRUSTUX KERNEL PANIC");
    println!("Fatal lock violation detected! reason={}", result.to_string());

    // TODO: Print backtrace and halt

    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_class_registration() {
        let id1 = register_lock_class("test_lock1");
        let id2 = register_lock_class("test_lock2");

        assert!(id1 < id2);
    }

    #[test]
    fn test_acquired_lock_entry() {
        let entry = AcquiredLockEntry::new(42);
        assert_eq!(entry.id(), 42);
        assert_eq!(entry.order(), 0); // First lock
    }

    #[test]
    fn test_thread_lock_state() {
        let mut state = ThreadLockState::new();

        assert_eq!(state.lock_depth(), 0);

        let entry = AcquiredLockEntry::new(1);
        state.push_lock(entry);

        assert_eq!(state.lock_depth(), 1);

        let popped = state.pop_lock();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().id(), 1);
        assert_eq!(state.lock_depth(), 0);
    }

    #[test]
    fn test_lock_result_string() {
        assert_eq!(LockResult::Ok.to_string(), "Ok");
        assert_eq!(
            LockResult::OrderViolation.to_string(),
            "Order violation"
        );
        assert_eq!(
            LockResult::CircularDependency.to_string(),
            "Circular dependency"
        );
        assert_eq!(
            LockResult::InvalidLockClass.to_string(),
            "Invalid lock class"
        );
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_LOCK_CLASSES, 256);
    }
}
