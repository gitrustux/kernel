// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! User Boot
//!
//! This module handles bootstrapping the first user-space process.
//! It creates the initial process, thread, and passes necessary handles
//! and data to userspace.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Default stack size
pub const ZIRCON_DEFAULT_STACK_SIZE: usize = 256 * 1024;

/// Maximum command line length
pub const CMDLINE_MAX: usize = 4096;

/// Bootstrap handle indices
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapHandleIndex {
    Vdso = 0,
    VdsoLastVariant,
    Ramdisk,
    ResourceRoot,
    Stack,
    Proc,
    Thread,
    Job,
    VmarRoot,
    Crashlog,
    EntropyFile,
    Handles,
}

/// Bootstrap message
#[repr(C)]
#[derive(Debug)]
pub struct BootstrapMessage {
    /// Process args header
    pub header: ProcArgs,
    /// Handle info for each handle
    pub handle_info: [u32; BootstrapHandleIndex::Handles as usize],
    /// Kernel command line
    pub cmdline: [u8; CMDLINE_MAX],
}

/// Process args header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcArgs {
    /// Protocol version
    pub protocol: u32,
    /// Version
    pub version: u32,
    /// Environment offset
    pub environ_off: u32,
    /// Environment count
    pub environ_num: u32,
    /// Handle info offset
    pub handle_info_off: u32,
}

/// Userboot state
pub struct UserbootState {
    /// Initialized flag
    pub initialized: AtomicBool,
    /// VDSO base address
    pub vdso_base: AtomicU64,
    /// Entry point address
    pub entry_point: AtomicU64,
    /// Stack base address
    pub stack_base: AtomicU64,
    /// Stack pointer
    pub stack_pointer: AtomicU64,
}

unsafe impl Send for UserbootState {}
unsafe impl Sync for UserbootState {}

impl UserbootState {
    /// Create a new userboot state
    pub fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            vdso_base: AtomicU64::new(0),
            entry_point: AtomicU64::new(0),
            stack_base: AtomicU64::new(0),
            stack_pointer: AtomicU64::new(0),
        }
    }

    /// Attempt to bootstrap userboot
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err(status) on failure
    pub fn attempt_userboot(&self) -> Result<(), i32> {
        println!("Userboot: Attempting to bootstrap user process");

        // Get ramdisk if available
        if let Some((ramdisk_base, ramdisk_size)) = platform_get_ramdisk() {
            println!(
                "Userboot: Ramdisk {:#x} @ {:#x}",
                ramdisk_size, ramdisk_base
            );
        }

        // Create stack VMO
        let stack_vmo = self.create_stack_vmo()?;
        println!("Userboot: Created stack VMO (size: {})", ZIRCON_DEFAULT_STACK_SIZE);

        // Create bootstrap message
        let bootstrap_msg = self.prepare_bootstrap_message()?;
        println!("Userboot: Prepared bootstrap message");

        // Create process and thread
        let (proc_handle, thread_handle) = self.create_process_thread()?;
        println!("Userboot: Created process and thread");

        // Map userboot image
        let (vdso_base, entry) = self.map_userboot_image()?;
        self.vdso_base.store(vdso_base, Ordering::Release);
        self.entry_point.store(entry, Ordering::Release);

        // Map stack
        let stack_base = self.map_stack()?;
        self.stack_base.store(stack_base, Ordering::Release);

        // Compute stack pointer
        let sp = self.compute_initial_stack_pointer(stack_base);
        self.stack_pointer.store(sp, Ordering::Release);

        println!("Userboot: Entry point @ {:#x}", entry);
        println!("Userboot: Stack @ {:#x} (SP @ {:#x})", stack_base, sp);

        // Start the thread
        // TODO: Implement thread start
        println!("Userboot: Starting user process thread");

        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    /// Create stack VMO
    fn create_stack_vmo(&self) -> Result<(), i32> {
        // TODO: Implement VMO creation for stack
        Ok(())
    }

    /// Prepare bootstrap message
    fn prepare_bootstrap_message(&self) -> Result<BootstrapMessage, i32> {
        let mut msg = BootstrapMessage {
            header: ProcArgs {
                protocol: 0,
                version: 0,
                environ_off: 0,
                environ_num: 0,
                handle_info_off: 0,
            },
            handle_info: [0u32; BootstrapHandleIndex::Handles as usize],
            cmdline: [0u8; CMDLINE_MAX],
        };

        // Fill in handle info
        let handle_info = &mut msg.handle_info;
        handle_info[BootstrapHandleIndex::Vdso as usize] = make_handle_info(0, 0);
        handle_info[BootstrapHandleIndex::Ramdisk as usize] = make_handle_info(1, 0);
        handle_info[BootstrapHandleIndex::ResourceRoot as usize] = make_handle_info(2, 0);
        handle_info[BootstrapHandleIndex::Stack as usize] = make_handle_info(3, 0);
        handle_info[BootstrapHandleIndex::Proc as usize] = make_handle_info(4, 0);
        handle_info[BootstrapHandleIndex::Thread as usize] = make_handle_info(5, 0);
        handle_info[BootstrapHandleIndex::Job as usize] = make_handle_info(6, 0);
        handle_info[BootstrapHandleIndex::VmarRoot as usize] = make_handle_info(7, 0);
        handle_info[BootstrapHandleIndex::Crashlog as usize] = make_handle_info(8, 0);

        // Fill in command line
        let cmdline = get_kernel_cmdline();
        let len = cmdline.len().min(CMDLINE_MAX);
        msg.cmdline[..len].copy_from_slice(cmdline.as_bytes());

        Ok(msg)
    }

    /// Create process and thread
    fn create_process_thread(&self) -> Result<(u32, u32), i32> {
        // TODO: Implement process/thread creation
        // For now, return placeholder handles
        Ok((0, 0))
    }

    /// Map userboot image
    fn map_userboot_image(&self) -> Result<(u64, u64), i32> {
        // TODO: Implement userboot image mapping
        // Return placeholder addresses
        Ok((0x10000000, 0x10001000))
    }

    /// Map stack
    fn map_stack(&self) -> Result<u64, i32> {
        // TODO: Implement stack mapping
        Ok(0x20000000)
    }

    /// Compute initial stack pointer
    fn compute_initial_stack_pointer(&self, stack_base: u64) -> u64 {
        stack_base + ZIRCON_DEFAULT_STACK_SIZE as u64
    }
}

impl Default for UserbootState {
    fn default() -> Self {
        Self::new()
    }
}

/// Global userboot state
static USERBOOT_STATE: Mutex<UserbootState> = Mutex::new(UserbootState::new());

/// Initialize userboot
///
/// This function is called during late boot to bootstrap
/// the first user-space process.
pub fn userboot_init() {
    println!("Userboot: Initializing");

    let state = USERBOOT_STATE.lock();
    if let Err(e) = state.attempt_userboot() {
        println!("Userboot: Failed to bootstrap: {}", e);
    }
}

/// Make handle info value
///
/// # Arguments
///
/// * `kind` - Handle kind
/// * `id` - Handle ID
///
/// # Returns
///
/// Handle info value
fn make_handle_info(kind: u32, id: u32) -> u32 {
    (kind << 16) | (id & 0xFFFF)
}

/// Get ramdisk from platform
///
/// # Returns
///
/// Some((base, size)) if ramdisk exists, None otherwise
fn platform_get_ramdisk() -> Option<(u64, u64)> {
    // TODO: Implement platform ramdisk query
    None
}

/// Get kernel command line
///
/// # Returns
///
/// Kernel command line string
fn get_kernel_cmdline() -> &'static str {
    // TODO: Get actual kernel command line
    "rustux.kernel"
}

/// Get VDSO base address
pub fn get_vdso_base() -> u64 {
    let state = USERBOOT_STATE.lock();
    state.vdso_base.load(Ordering::Acquire)
}

/// Get entry point address
pub fn get_entry_point() -> u64 {
    let state = USERBOOT_STATE.lock();
    state.entry_point.load(Ordering::Acquire)
}

/// Check if userboot is initialized
pub fn is_userboot_initialized() -> bool {
    let state = USERBOOT_STATE.lock();
    state.initialized.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_userboot_state() {
        let state = UserbootState::new();
        assert!(!state.initialized.load(Ordering::Acquire));
    }

    #[test]
    fn test_bootstrap_handle_indices() {
        assert_eq!(BootstrapHandleIndex::Vdso as usize, 0);
        assert_eq!(BootstrapHandleIndex::Ramdisk as usize, 2);
        assert_eq!(BootstrapHandleIndex::Proc as usize, 5);
    }

    #[test]
    fn test_make_handle_info() {
        let info = make_handle_info(1, 42);
        assert_eq!(info, 0x0001002a);

        let kind = (info >> 16) & 0xFF;
        let id = info & 0xFFFF;
        assert_eq!(kind, 1);
        assert_eq!(id, 42);
    }

    #[test]
    fn test_constants() {
        assert_eq!(ZIRCON_DEFAULT_STACK_SIZE, 256 * 1024);
        assert_eq!(CMDLINE_MAX, 4096);
    }
}
