// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Trace
//!
//! This module provides kernel tracing functionality for debugging and performance analysis.

#![no_std]

extern crate alloc;

use alloc::collections::LinkedList;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, AtomicU8, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Default trace buffer size
const DEFAULT_KTRACE_BUFSIZE: usize = 1024 * 1024; // 1MB

/// Trace group mask bits
pub const KTRACE_GRP_ALL: u32 = 0x0000ffff;
pub const KTRACE_GRP_META: u32 = 0x00000001;
pub const KTRACE_GRP_LIFECYCLE: u32 = 0x00000002;
pub const KTRACE_GRP_SCHEDULER: u32 = 0x00000004;
pub const KTRACE_GRP_SYSCALL: u32 = 0x00000008;
pub const KTRACE_GRP_PLATFORM: u32 = 0x00000010;
pub const KTRACE_GRP_IPC: u32 = 0x00000020;
pub const KTRACE_GRP_PROBE: u32 = 0x00000040;

/// Trace actions
pub const KTRACE_ACTION_START: u32 = 0;
pub const KTRACE_ACTION_STOP: u32 = 1;
pub const KTRACE_ACTION_REWIND: u32 = 2;

/// Convert group number to mask
#[inline]
pub const fn ktrace_grp_to_mask(grp: u32) -> u32 {
    1u32 << grp
}

/// Trace tag names
pub const TAG_SYSCALL_NAME: u32 = 0x100;
pub const TAG_PROBE_NAME: u32 = 0x101;
pub const TAG_THREAD_NAME: u32 = 0x102;
pub const TAG_PROCESS_NAME: u32 = 0x103;

/// Syscall info structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct KtraceSyscallInfo {
    pub id: u32,
    pub nargs: u32,
    pub name: &'static str,
}

/// Probe info structure
#[repr(C)]
#[derive(Debug)]
pub struct KtraceProbeInfo {
    pub name: &'static str,
    pub num: u32,
    pub next: Option<*mut KtraceProbeInfo>,
}

/// KTrace state
struct KtraceState {
    /// Current write offset
    offset: AtomicU32,
    /// Group mask (0 = disabled)
    grpmask: AtomicU32,
    /// Total buffer size
    bufsize: u32,
    /// Offset where tracing was stopped
    marker: AtomicU32,
    /// Trace buffer
    buffer: Vec<u8>,
}

impl KtraceState {
    fn new(bufsize: usize) -> Self {
        Self {
            offset: AtomicU32::new(0),
            grpmask: AtomicU32::new(0),
            bufsize: bufsize as u32,
            marker: AtomicU32::new(0),
            buffer: vec![0; bufsize],
        }
    }
}

/// Global ktrace state
static KTRACE_STATE: Mutex<Option<KtraceState>> = Mutex::new(None);

/// Probe list
static PROBE_LIST: Mutex<LinkedList<Arc<KtraceProbeInfo>>> = Mutex::new(LinkedList::new());

/// Probe number counter
static PROBE_NUMBER: AtomicU32 = AtomicU32::new(1);

/// Initialize ktrace with the specified buffer size
pub fn ktrace_init(bufsize: usize) {
    let mut state = KTRACE_STATE.lock();

    if state.is_some() {
        return; // Already initialized
    }

    *state = Some(KtraceState::new(bufsize));

    println!("KTRACE: initialized with {} byte buffer", bufsize);
}

/// Read ktrace data to user buffer
///
/// # Arguments
///
/// * `ptr` - User buffer to read to (null = query size)
/// * `off` - Offset in trace buffer
/// * `len` - Length to read
///
/// # Returns
///
/// Number of bytes read, or buffer size if ptr is null
pub fn ktrace_read_user(ptr: Option<&mut [u8]>, off: u32, len: usize) -> Result<usize, i32> {
    let state = KTRACE_STATE.lock();

    let ks = state.as_ref().ok_or(-1)?;

    // Determine max offset
    let max = if ks.marker.load(Ordering::Acquire) != 0 {
        ks.marker.load(Ordering::Acquire)
    } else {
        let offset = ks.offset.load(Ordering::Acquire);
        if offset > ks.bufsize {
            ks.bufsize
        } else {
            offset
        }
    };

    // Null read = query for size
    if ptr.is_none() {
        return Ok(max as usize);
    }

    let ptr = ptr.unwrap();

    // Constrain read to available buffer
    if off >= max {
        return Ok(0);
    }

    let available = (max - off) as usize;
    let to_read = len.min(available);

    // Copy to user buffer
    // TODO: Implement arch_copy_to_user
    let buffer_slice = &ks.buffer[off as usize..(off as usize + to_read)];
    ptr[..to_read].copy_from_slice(buffer_slice);

    Ok(to_read)
}

/// Control ktrace
///
/// # Arguments
///
/// * `action` - Action to perform (START, STOP, REWIND)
/// * `options` - Options for the action
pub fn ktrace_control(action: u32, options: u32) -> Result<(), i32> {
    let mut state = KTRACE_STATE.lock();

    let ks = state.as_mut().ok_or(-1)?;

    match action {
        KTRACE_ACTION_START => {
            let mask = if options != 0 {
                ktrace_grp_to_mask(options)
            } else {
                KTRACE_GRP_ALL
            };

            ks.marker.store(0, Ordering::Release);
            ks.grpmask.store(mask, Ordering::Release);

            // Report live processes and threads
            // TODO: Implement ktrace_report_live_processes and ktrace_report_live_threads

            println!("KTRACE: started");
        }
        KTRACE_ACTION_STOP => {
            ks.grpmask.store(0, Ordering::Release);

            let offset = ks.offset.load(Ordering::Acquire);
            let marker = if offset > ks.bufsize {
                ks.bufsize
            } else {
                offset
            };
            ks.marker.store(marker, Ordering::Release);

            println!("KTRACE: stopped at {}", marker);
        }
        KTRACE_ACTION_REWIND => {
            ks.offset.store(0, Ordering::Release);
            ks.marker.store(0, Ordering::Release);

            println!("KTRACE: rewound");
        }
        _ => {
            return Err(-2); // Invalid action
        }
    }

    Ok(())
}

/// Write a ktrace record
///
/// # Arguments
///
/// * `tag` - Trace tag
/// * `args` - Trace arguments (up to 4)
pub fn ktrace_write(tag: u32, args: [u64; 4]) {
    // TODO: Implement trace writing
    let _ = (tag, args);
}

/// Write a name record
///
/// # Arguments
///
/// * `tag` - Name tag
/// * `id` - ID
/// * `arg` - Additional argument
/// * `name` - Name string
pub fn ktrace_name_etc(tag: u32, id: u32, arg: u32, name: &str) {
    // TODO: Implement name recording
    let _ = (tag, id, arg, name);
}

/// Add a probe
///
/// # Arguments
///
/// * `name` - Probe name
///
/// # Returns
///
/// Probe number
pub fn ktrace_add_probe(name: &'static str) -> u32 {
    let num = PROBE_NUMBER.fetch_add(1, Ordering::AcqRel);

    let info = Arc::new(KtraceProbeInfo {
        name,
        num,
        next: None,
    });

    PROBE_LIST.lock().push_back(info.clone());

    // Report the probe
    ktrace_name_etc(TAG_PROBE_NAME, num, 0, name);

    num
}

/// Find a probe by name
pub fn ktrace_find_probe(name: &str) -> Option<Arc<KtraceProbeInfo>> {
    let list = PROBE_LIST.lock();
    for probe in list.iter() {
        if probe.name == name {
            return Some(probe.clone());
        }
    }
    None
}

/// Report all probes
pub fn ktrace_report_probes() {
    let list = PROBE_LIST.lock();
    for probe in list.iter() {
        ktrace_name_etc(TAG_PROBE_NAME, probe.num, 0, probe.name);
    }
}

/// Get current trace state
pub fn ktrace_get_state() -> (u32, u32, u32) {
    let state = KTRACE_STATE.lock();

    if let Some(ks) = state.as_ref() {
        (
            ks.offset.load(Ordering::Acquire),
            ks.grpmask.load(Ordering::Acquire),
            ks.marker.load(Ordering::Acquire),
        )
    } else {
        (0, 0, 0)
    }
}

/// Check if tracing is enabled for a group
pub fn ktrace_enabled(grp: u32) -> bool {
    let state = KTRACE_STATE.lock();

    if let Some(ks) = state.as_ref() {
        let mask = ks.grpmask.load(Ordering::Acquire);
        mask != 0 && (mask & ktrace_grp_to_mask(grp)) != 0
    } else {
        false
    }
}

/// Quick trace (only if tracing is enabled)
#[inline]
pub fn ktrace_quick(tag: u32, arg0: u32, arg1: u32) {
    if ktrace_enabled(tag >> 16) {
        ktrace_write(tag, [arg0 as u64, arg1 as u64, 0, 0]);
    }
}

/// Kernel trace probe with 64-bit argument (stub)
#[inline]
pub fn ktrace_probe64(_tag: u32, _arg: u64) {
    // Stub - no-op for now
}

/// Kernel trace probe with no arguments (stub)
#[inline]
pub fn ktrace_probe0(_tag: u32) {
    // Stub - no-op for now
}

/// Kernel trace probe with two arguments (stub)
#[inline]
pub fn ktrace_probe2(_tag: u32, _arg1: u64, _arg2: u64) {
    // Stub - no-op for now
}

/// Initialize kernel tracing
pub fn init() {
    ktrace_init(DEFAULT_KTRACE_BUFSIZE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ktrace_init() {
        ktrace_init(1024);

        let (offset, grpmask, marker) = ktrace_get_state();
        assert_eq!(offset, 0);
        assert_eq!(grpmask, 0);
        assert_eq!(marker, 0);
    }

    #[test]
    fn test_ktrace_control() {
        ktrace_init(1024);

        // Start tracing
        ktrace_control(KTRACE_ACTION_START, KTRACE_GRP_ALL).unwrap();

        let (_, grpmask, _) = ktrace_get_state();
        assert_eq!(grpmask, KTRACE_GRP_ALL);

        // Stop tracing
        ktrace_control(KTRACE_ACTION_STOP, 0).unwrap();

        let (_, grpmask, marker) = ktrace_get_state();
        assert_eq!(grpmask, 0);
        assert_eq!(marker, 0);
    }

    #[test]
    fn test_ktrace_enabled() {
        ktrace_init(1024);

        assert!(!ktrace_enabled(KTRACE_GRP_META));

        ktrace_control(KTRACE_ACTION_START, KTRACE_GRP_META).unwrap();
        assert!(ktrace_enabled(KTRACE_GRP_META));
    }

    #[test]
    fn test_probe() {
        let num = ktrace_add_probe("test_probe");
        assert!(num > 0);

        let probe = ktrace_find_probe("test_probe");
        assert!(probe.is_some());
        assert_eq!(probe.unwrap().name, "test_probe");
    }
}
