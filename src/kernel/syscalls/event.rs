// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Event System Calls
//!
//! This module implements the Event and EventPair system calls.
//! Events are simple synchronization primitives for signaling between threads.
//!
//! # Syscalls Implemented
//!
//! - `rx_event_create` - Create an event
//! - `rx_eventpair_create` - Create an event pair
//! - `rx_object_signal` - Signal an object
//!
//! # Design
//!
//! - Events are binary (signaled/not signaled)
//! - EventPairs are pairs that signal each other
//! - Both auto-reset and manual-reset modes supported
//! - Multiple threads can wait on same event


use crate::kernel::object::event::{self, Event, EventPair, EventFlags};
use crate::kernel::object::{Handle, HandleTable, KernelObjectBase, ObjectType, Rights};
use crate::kernel::sync::Mutex;
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Event Registry
/// ============================================================================

/// Maximum number of events in the system
const MAX_EVENTS: usize = 65536;

/// Event registry entry
struct EventEntry {
    /// Event ID
    id: event::EventId,

    /// Event object
    event: Arc<Event>,
}

/// Global event registry
///
/// Maps event IDs to event objects. This is used to resolve handles to events.
struct EventRegistry {
    /// Event entries
    entries: [Option<EventEntry>; MAX_EVENTS],

    /// Next event index to allocate
    next_index: AtomicUsize,

    /// Number of active events
    count: AtomicUsize,
}

impl EventRegistry {
    /// Create a new event registry
    const fn new() -> Self {
        const INIT: Option<EventEntry> = None;

        Self {
            entries: [INIT; MAX_EVENTS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert an event into the registry
    pub fn insert(&mut self, event: Arc<Event>) -> Result<event::EventId> {
        let id = event.id;

        // Find a free slot
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (id as usize) % MAX_EVENTS;

        loop {
            // Try to allocate at current index
            if self.entries[idx].is_none() {
                self.entries[idx] = Some(EventEntry { id, event });
                self.count.fetch_add(1, Ordering::Relaxed);
                self.next_index.store((idx + 1) % MAX_EVENTS, Ordering::Relaxed);
                return Ok(id);
            }

            // Linear probe
            idx = (idx + 1) % MAX_EVENTS;

            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    /// Get an event from the registry
    pub fn get(&self, id: event::EventId) -> Option<Arc<Event>> {
        let idx = (id as usize) % MAX_EVENTS;

        self.entries[idx]
            .as_ref()
            .filter(|entry| entry.id == id)
            .map(|entry| entry.event.clone())
    }

    /// Remove an event from the registry
    pub fn remove(&mut self, id: event::EventId) -> Option<Arc<Event>> {
        let idx = (id as usize) % MAX_EVENTS;

        if let Some(entry) = self.entries[idx].take() {
            if entry.id == id {
                self.count.fetch_sub(1, Ordering::Relaxed);
                return Some(entry.event);
            }
        }

        None
    }

    /// Get the number of active events
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global event registry
static EVENT_REGISTRY: Mutex<EventRegistry> = Mutex::new(EventRegistry::new());

/// ============================================================================
/// EventPair Registry
/// ============================================================================

/// EventPair registry entry
struct EventPairEntry {
    /// EventPair ID
    id: event::EventPairId,

    /// EventPair object
    eventpair: Arc<EventPair>,
}

/// Global eventpair registry
struct EventPairRegistry {
    /// EventPair entries
    entries: [Option<EventPairEntry>; MAX_EVENTS],

    /// Next eventpair index to allocate
    next_index: AtomicUsize,

    /// Number of active eventpairs
    count: AtomicUsize,
}

impl EventPairRegistry {
    /// Create a new eventpair registry
    const fn new() -> Self {
        const INIT: Option<EventPairEntry> = None;

        Self {
            entries: [INIT; MAX_EVENTS],
            next_index: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
        }
    }

    /// Insert an eventpair into the registry
    pub fn insert(&mut self, eventpair: Arc<EventPair>) -> Result<event::EventPairId> {
        let id = eventpair.id;

        // Find a free slot
        let start = self.next_index.load(Ordering::Relaxed);
        let mut idx = (id as usize) % MAX_EVENTS;

        loop {
            // Try to allocate at current index
            if self.entries[idx].is_none() {
                self.entries[idx] = Some(EventPairEntry { id, eventpair });
                self.count.fetch_add(1, Ordering::Relaxed);
                self.next_index.store((idx + 1) % MAX_EVENTS, Ordering::Relaxed);
                return Ok(id);
            }

            // Linear probe
            idx = (idx + 1) % MAX_EVENTS;

            if idx == start {
                return Err(RX_ERR_NO_RESOURCES);
            }
        }
    }

    /// Get an eventpair from the registry
    pub fn get(&self, id: event::EventPairId) -> Option<Arc<EventPair>> {
        let idx = (id as usize) % MAX_EVENTS;

        self.entries[idx]
            .as_ref()
            .filter(|entry| entry.id == id)
            .map(|entry| entry.eventpair.clone())
    }

    /// Remove an eventpair from the registry
    pub fn remove(&mut self, id: event::EventPairId) -> Option<Arc<EventPair>> {
        let idx = (id as usize) % MAX_EVENTS;

        if let Some(entry) = self.entries[idx].take() {
            if entry.id == id {
                self.count.fetch_sub(1, Ordering::Relaxed);
                return Some(entry.eventpair);
            }
        }

        None
    }

    /// Get the number of active eventpairs
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Global eventpair registry
static EVENTPAIR_REGISTRY: Mutex<EventPairRegistry> = Mutex::new(EventPairRegistry::new());

/// ============================================================================
/// Handle to Event Resolution
/// ============================================================================

/// Get the current process's handle table
///
/// This is a placeholder that returns NULL for now.
/// In a real implementation, this would use thread-local storage
/// or per-CPU data to get the current process.
fn current_process_handle_table() -> Option<&'static HandleTable> {
    // TODO: Implement proper current process tracking
    // For now, return None to indicate not implemented
    None
}

/// Look up an event from a handle value
///
/// This function:
/// 1. Gets the current process's handle table
/// 2. Looks up the handle in the table
/// 3. Validates the handle type and rights
/// 4. Returns the event object
fn lookup_event_from_handle(
    handle_val: u32,
    required_rights: Rights,
) -> Result<(Arc<Event>, Handle)> {
    // Get current process handle table
    let handle_table = current_process_handle_table()
        .ok_or(RX_ERR_NOT_SUPPORTED)?;

    // Get the handle from the table
    let handle = handle_table.get(handle_val)
        .ok_or(RX_ERR_INVALID_ARGS)?;

    // Validate object type (Event or EventPair both use Event base)
    let obj_type = handle.obj_type();
    if obj_type != ObjectType::Event && obj_type != ObjectType::EventPair {
        return Err(RX_ERR_WRONG_TYPE);
    }

    // Validate rights
    handle.require(required_rights)?;

    // Get event ID from handle (stored as part of base pointer for now)
    let event_id = handle.id as event::EventId;

    // Get event from registry
    let event = EVENT_REGISTRY.lock().get(event_id)
        .ok_or(RX_ERR_NOT_FOUND)?;

    Ok((event, handle))
}

/// Look up an eventpair from a handle value
fn lookup_eventpair_from_handle(
    handle_val: u32,
    required_rights: Rights,
) -> Result<(Arc<EventPair>, Handle)> {
    // Get current process handle table
    let handle_table = current_process_handle_table()
        .ok_or(RX_ERR_NOT_SUPPORTED)?;

    // Get the handle from the table
    let handle = handle_table.get(handle_val)
        .ok_or(RX_ERR_INVALID_ARGS)?;

    // Validate object type
    if handle.obj_type() != ObjectType::EventPair {
        return Err(RX_ERR_WRONG_TYPE);
    }

    // Validate rights
    handle.require(required_rights)?;

    // Get eventpair ID from handle
    let eventpair_id = handle.id as event::EventPairId;

    // Get eventpair from registry
    let eventpair = EVENTPAIR_REGISTRY.lock().get(eventpair_id)
        .ok_or(RX_ERR_NOT_FOUND)?;

    Ok((eventpair, handle))
}

/// ============================================================================
/// Event Kernel Object Base
/// ============================================================================

/// Create a kernel object base for an event
fn event_to_kernel_base(event: &Arc<Event>) -> KernelObjectBase {
    KernelObjectBase::new(ObjectType::Event)
}

/// Create a kernel object base for an eventpair
fn eventpair_to_kernel_base(_eventpair: &Arc<EventPair>) -> KernelObjectBase {
    KernelObjectBase::new(ObjectType::EventPair)
}

/// ============================================================================
/// Syscall: Event Create
/// ============================================================================

/// Create a new event syscall handler
///
/// # Arguments
///
/// * `options` - Event creation options
///   - bit 0: initial signal state (0 = unsignaled, 1 = signaled)
///   - bit 1: manual reset flag
///
/// # Returns
///
/// * On success: Handle value for the new event
/// * On error: Negative error code
pub fn sys_event_create_impl(options: u32) -> SyscallRet {
    log_debug!("sys_event_create: options={:#x}", options);

    // Parse options
    let initial_signaled = (options & 0x01) != 0;
    let flags = if (options & 0x02) != 0 {
        EventFlags::MANUAL_RESET
    } else {
        EventFlags::empty
    };

    // Create the event
    let event = Event::new(initial_signaled, flags);

    log_debug!("sys_event_create: created event id={}", event.id);

    // Wrap in Arc for registry
    let event_arc = Arc::new(event);

    // Insert into event registry
    let event_id = match EVENT_REGISTRY.lock().insert(event_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_event_create: failed to insert event into registry: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Create kernel object base
    let base = event_to_kernel_base(&event_arc);

    // Create handle with default rights (SIGNAL | WAIT)
    let rights = Rights::SIGNAL | Rights::WAIT;
    let handle = Handle::new(&base as *const KernelObjectBase, rights);

    // TODO: Add handle to current process's handle table
    // For now, return the event ID as the handle value
    let handle_value = event_id as u32;

    log_debug!("sys_event_create: success handle={}", handle_value);

    ok_to_ret(handle_value as usize)
}

/// ============================================================================
/// Syscall: EventPair Create
/// ============================================================================

/// Create an event pair syscall handler
///
/// # Returns
///
/// * On success: Two handle values packed (lower 32 bits = handle_a, upper 32 bits = handle_b)
/// * On error: Negative error code
pub fn sys_eventpair_create_impl() -> SyscallRet {
    log_debug!("sys_eventpair_create:");

    // Create the event pair
    let (pair_a, pair_b) = match EventPair::create() {
        Ok(pairs) => pairs,
        Err(err) => {
            log_error!("sys_eventpair_create: failed to create eventpair: {:?}", err);
            return err_to_ret(err);
        }
    };

    log_debug!("sys_eventpair_create: created eventpairs id_a={} id_b={}",
        pair_a.id, pair_b.id);

    // Wrap in Arc for registry
    let pair_a_arc = Arc::new(pair_a);
    let pair_b_arc = Arc::new(pair_b);

    // Insert into eventpair registry
    let id_a = match EVENTPAIR_REGISTRY.lock().insert(pair_a_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_eventpair_create: failed to insert pair_a: {:?}", err);
            return err_to_ret(err);
        }
    };

    let id_b = match EVENTPAIR_REGISTRY.lock().insert(pair_b_arc.clone()) {
        Ok(id) => id,
        Err(err) => {
            log_error!("sys_eventpair_create: failed to insert pair_b: {:?}", err);
            // Cleanup pair_a
            EVENTPAIR_REGISTRY.lock().remove(id_a);
            return err_to_ret(err);
        }
    };

    // Create kernel object bases
    let base_a = eventpair_to_kernel_base(&pair_a_arc);
    let base_b = eventpair_to_kernel_base(&pair_b_arc);

    // Create handles with default rights (SIGNAL | WAIT)
    let rights = Rights::SIGNAL | Rights::WAIT;
    let handle_a = Handle::new(&base_a as *const KernelObjectBase, rights);
    let handle_b = Handle::new(&base_b as *const KernelObjectBase, rights);

    // TODO: Add handles to current process's handle table
    // For now, return the eventpair IDs as the handle values
    let handle_value_a = id_a as u32;
    let handle_value_b = id_b as u32;

    // Pack two handles into return value: lower 32 bits = handle_a, upper 32 bits = handle_b
    let packed = (handle_value_b as u64) << 32 | (handle_value_a as u64);

    log_debug!("sys_eventpair_create: success handle_a={} handle_b={}",
        handle_value_a, handle_value_b);

    ok_to_ret(packed as usize)
}

/// ============================================================================
/// Syscall: Object Signal
/// ============================================================================

/// Signal an object syscall handler
///
/// # Arguments
///
/// * `handle_val` - Handle value of the object to signal
/// * `options` - Signal options
///   - bit 0: user facing signal mask (currently unused)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_signal_impl(handle_val: u32, options: u32) -> SyscallRet {
    log_debug!("sys_object_signal: handle={} options={:#x}", handle_val, options);

    // Extract signal mask from options
    let signal_mask = match options {
        0 => crate::kernel::syscalls::object_wait::signal::USER_0,
        _ => options as u64,
    };

    // Try to look up as an event first
    if let Ok((event, _handle)) = lookup_event_from_handle(handle_val, Rights::SIGNAL) {
        event.signal();

        // Wake up any threads waiting on this object
        let woken = crate::kernel::syscalls::object_wait::wake_waiters(handle_val, signal_mask);
        log_debug!("sys_object_signal: signaled event, woke {} waiters", woken);

        return ok_to_ret(0);
    }

    // Try to look up as an eventpair
    if let Ok((eventpair, _handle)) = lookup_eventpair_from_handle(handle_val, Rights::SIGNAL) {
        eventpair.signal();

        // Wake up any threads waiting on this object
        let woken = crate::kernel::syscalls::object_wait::wake_waiters(handle_val, signal_mask);
        log_debug!("sys_object_signal: signaled eventpair, woke {} waiters", woken);

        return ok_to_ret(0);
    }

    // TODO: Support signaling other object types (channel, timer, etc.)

    log_error!("sys_object_signal: handle not found or not signalable");
    err_to_ret(RX_ERR_INVALID_ARGS)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get event subsystem statistics
pub fn get_stats() -> EventStats {
    EventStats {
        total_events: EVENT_REGISTRY.lock().count(),
        total_eventpairs: EVENTPAIR_REGISTRY.lock().count(),
        signaled_count: 0, // TODO: Track signaled events
    }
}

/// Event subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EventStats {
    /// Total number of events
    pub total_events: usize,

    /// Total number of eventpairs
    pub total_eventpairs: usize,

    /// Number of currently signaled events
    pub signaled_count: usize,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the event syscall subsystem
pub fn init() {
    log_info!("Event syscall subsystem initialized");
    log_info!("  Max events: {}", MAX_EVENTS);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_registry_insert_get() {
        let event = Event::new(false, EventFlags::empty);
        let event_arc = Arc::new(event);

        let id = EVENT_REGISTRY.lock().insert(event_arc.clone()).unwrap();
        assert_eq!(id, event_arc.id);

        let retrieved = EVENT_REGISTRY.lock().get(id).unwrap();
        assert_eq!(retrieved.id, event_arc.id);
    }

    #[test]
    fn test_event_registry_remove() {
        let event = Event::new(false, EventFlags::empty);
        let event_arc = Arc::new(event);

        let id = EVENT_REGISTRY.lock().insert(event_arc.clone()).unwrap();
        let removed = EVENT_REGISTRY.lock().remove(id).unwrap();

        assert_eq!(removed.id, event_arc.id);
        assert!(EVENT_REGISTRY.lock().get(id).is_none());
    }

    #[test]
    fn test_event_create_unsignaled() {
        let event = Event::new(false, EventFlags::empty);
        assert!(!event.is_signaled());
    }

    #[test]
    fn test_event_create_signaled() {
        let event = Event::new(true, EventFlags::empty);
        assert!(event.is_signaled());
    }

    #[test]
    fn test_event_signal_unsignal() {
        let event = Event::new(false, EventFlags::MANUAL_RESET);

        event.signal();
        assert!(event.is_signaled());

        event.unsignal();
        assert!(!event.is_signaled());
    }

    #[test]
    fn test_event_flags_manual_reset() {
        let flags = EventFlags::MANUAL_RESET;
        assert!(flags.is_manual_reset());
    }

    #[test]
    fn test_eventpair_registry_insert_get() {
        let (pair_a, _pair_b) = EventPair::create().unwrap();
        let pair_arc = Arc::new(pair_a);

        let id = EVENTPAIR_REGISTRY.lock().insert(pair_arc.clone()).unwrap();
        assert_eq!(id, pair_arc.id);

        let retrieved = EVENTPAIR_REGISTRY.lock().get(id).unwrap();
        assert_eq!(retrieved.id, pair_arc.id);
    }

    #[test]
    fn test_eventpair_create() {
        let (pair_a, pair_b) = EventPair::create().unwrap();
        assert_ne!(pair_a.id, pair_b.id);
        assert_eq!(pair_a.peer.load(Ordering::Relaxed), pair_b.id as usize);
        assert_eq!(pair_b.peer.load(Ordering::Relaxed), pair_a.id as usize);
    }
}
