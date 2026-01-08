// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Profile System Calls
//!
//! This module implements profile-related system calls.
//!
//! # Syscalls Implemented
//!
//! - `rx_profile_create` - Create a profile for CPU scheduling
//! - `rx_object_set_profile` - Apply a profile to a thread
//!
//! # Design
//!
//! Profiles allow privileged processes to control CPU scheduling behavior,
//! such as CPU affinity, priority, and scheduling parameters.


use crate::kernel::object::{Handle, HandleTable, ObjectType, Rights, KernelObjectBase};
use crate::kernel::usercopy::{copy_from_user, UserPtr};
use crate::kernel::syscalls::{SyscallRet, err_to_ret, ok_to_ret};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Import logging macros
use crate::{log_debug, log_error, log_info};

/// ============================================================================
/// Profile Constants
/// ============================================================================

/// Maximum profile name length
const MAX_PROFILE_NAME_LEN: usize = 64;

/// Profile flags
pub mod profile_flags {
    /// No flags
    pub const NONE: u32 = 0;

    /// Profile has CPU affinity
    pub const HAS_CPU_AFFINITY: u32 = 0x01;

    /// Profile has priority
    pub const HAS_PRIORITY: u32 = 0x02;

    /// Profile is for real-time
    pub const REALTIME: u32 = 0x04;
}

/// ============================================================================
/// Profile Information
/// ============================================================================

/// Profile information structure
///
/// This structure defines CPU scheduling parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProfileInfo {
    /// Profile flags
    pub flags: u32,

    /// CPU affinity mask (bitfield of CPU IDs)
    pub cpu_affinity: u64,

    /// Priority level
    pub priority: u32,

    /// Reserved for future use
    pub reserved: [u32; 8],
}

impl Default for ProfileInfo {
    fn default() -> Self {
        Self {
            flags: 0,
            cpu_affinity: 0,
            priority: 0,
            reserved: [0; 8],
        }
    }
}

/// ============================================================================
/// Profile Object
/// ============================================================================

/// Maximum number of profiles in the system
const MAX_PROFILES: usize = 128;

/// Next profile ID counter
static mut NEXT_PROFILE_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a new profile ID
fn alloc_profile_id() -> u64 {
    unsafe { NEXT_PROFILE_ID.fetch_add(1, Ordering::Relaxed) }
}

/// Profile object
///
/// Represents a CPU scheduling profile that can be applied to threads.
pub struct Profile {
    /// Kernel object base
    pub base: KernelObjectBase,

    /// Profile ID
    id: u64,

    /// Profile information
    info: ProfileInfo,
}

impl Profile {
    /// Create a new profile
    pub fn new(info: ProfileInfo) -> Self {
        let id = alloc_profile_id();
        log_debug!("Profile::new: id={} flags={:#x}", id, info.flags);

        Self {
            base: KernelObjectBase::new(ObjectType::Profile),
            id,
            info,
        }
    }

    /// Get profile ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get profile information
    pub fn info(&self) -> ProfileInfo {
        self.info
    }

    /// Increment reference count
    pub fn inc_ref(&self) {
        self.base.ref_inc();
    }

    /// Decrement reference count
    pub fn dec_ref(&self) {
        if self.base.ref_dec() {
            // TODO: Add to cleanup list
        }
    }
}

/// ============================================================================
/// Syscall: Profile Create
/// ============================================================================

/// Create a profile syscall handler
///
/// # Arguments
///
/// * `root_job_handle` - Root job handle (must have MANAGE_PROCESS rights)
/// * `profile_info_user` - User pointer to profile information
/// * `profile_out` - User pointer to store the new profile handle
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_profile_create_impl(
    root_job_handle: u32,
    profile_info_user: usize,
    profile_out: usize,
) -> SyscallRet {
    log_debug!(
        "sys_profile_create: root_job={:#x} profile_info={:#x}",
        root_job_handle,
        profile_info_user
    );

    // TODO: Validate root job handle

    // Copy profile info from user
    let user_ptr = UserPtr::<u8>::new(profile_info_user);
    let mut profile_info = ProfileInfo::default();

    unsafe {
        if let Err(err) = copy_from_user(
            &mut profile_info as *mut ProfileInfo as *mut u8,
            user_ptr,
            1,
        ) {
            log_error!("sys_profile_create: copy_from_user failed: {:?}", err);
            return err_to_ret(err.into());
        }
    }

    // Validate profile flags
    if profile_info.flags & !(profile_flags::HAS_CPU_AFFINITY | profile_flags::HAS_PRIORITY | profile_flags::REALTIME)
        != 0
    {
        log_error!("sys_profile_create: invalid flags {:#x}", profile_info.flags);
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // Create the profile
    let profile = Arc::new(Profile::new(profile_info));

    // Add to current process's handle table
    let handle_table = crate::kernel::thread::current_thread_handle_table();

    // Create handle with APPLY_PROFILE rights
    let handle = Handle::new(&profile.base as *const KernelObjectBase, Rights::APPLY_PROFILE);
    let handle_value = match handle_table.add(handle) {
        Ok(val) => val,
        Err(err) => {
            log_error!("sys_profile_create: failed to add handle: {:?}", err);
            return err_to_ret(err);
        }
    };

    // Write handle value to user
    let out_ptr = UserPtr::<u8>::new(profile_out);
    unsafe {
        if let Err(err) =
            crate::kernel::usercopy::copy_to_user(out_ptr, &handle_value as *const u32 as *const u8, 4)
        {
            log_error!("sys_profile_create: copy_to_user failed: {:?}", err);
            let _ = handle_table.remove(handle_value);
            return err_to_ret(err.into());
        }
    }

    log_debug!("sys_profile_create: success handle={:#x}", handle_value);
    ok_to_ret(0)
}

/// ============================================================================
/// Syscall: Object Set Profile
/// ============================================================================

/// Set profile on object syscall handler
///
/// # Arguments
///
/// * `handle` - Object handle (currently only threads supported)
/// * `profile_handle` - Profile handle
/// * `options` - Options (must be 0)
///
/// # Returns
///
/// * On success: 0
/// * On error: Negative error code
pub fn sys_object_set_profile_impl(
    handle: u32,
    profile_handle: u32,
    options: u32,
) -> SyscallRet {
    log_debug!(
        "sys_object_set_profile: handle={:#x} profile={:#x} options={:#x}",
        handle,
        profile_handle,
        options
    );

    if options != 0 {
        log_error!("sys_object_set_profile: invalid options");
        return err_to_ret(RX_ERR_INVALID_ARGS);
    }

    // TODO: Get thread object from handle
    // TODO: Get profile object from profile_handle
    // TODO: Apply profile to thread

    // For now, just log
    log_info!("sys_object_set_profile: applied profile (stub)");

    ok_to_ret(0)
}

/// ============================================================================
/// Module Statistics
/// ============================================================================

/// Get profile subsystem statistics
pub fn get_stats() -> ProfileStats {
    ProfileStats {
        total_profiles: 0, // TODO: Track profiles
        total_applied: 0,  // TODO: Track applications
    }
}

/// Profile subsystem statistics
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProfileStats {
    /// Total number of profiles
    pub total_profiles: usize,

    /// Total number of profiles applied
    pub total_applied: u64,
}

/// ============================================================================
/// Module Initialization
/// ============================================================================

/// Initialize the profile syscall subsystem
pub fn init() {
    log_info!("Profile syscall subsystem initialized");
    log_info!("  Max profiles: {}", MAX_PROFILES);
}

/// ============================================================================
/// Tests
/// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_info_default() {
        let info = ProfileInfo::default();
        assert_eq!(info.flags, 0);
        assert_eq!(info.cpu_affinity, 0);
        assert_eq!(info.priority, 0);
    }

    #[test]
    fn test_profile_flags() {
        assert_eq!(profile_flags::NONE, 0);
        assert_eq!(profile_flags::HAS_CPU_AFFINITY, 0x01);
        assert_eq!(profile_flags::HAS_PRIORITY, 0x02);
        assert_eq!(profile_flags::REALTIME, 0x04);
    }

    #[test]
    fn test_profile_new() {
        let info = ProfileInfo {
            flags: profile_flags::HAS_PRIORITY,
            cpu_affinity: 0xFF,
            priority: 10,
            ..Default::default()
        };

        let profile = Profile::new(info);
        assert_eq!(profile.info().flags, profile_flags::HAS_PRIORITY);
        assert_eq!(profile.info().priority, 10);
    }

    #[test]
    fn test_profile_refcount() {
        let info = ProfileInfo::default();
        let profile = Profile::new(info);

        assert_eq!(profile.refcount.load(Ordering::Relaxed), 1);

        profile.inc_ref();
        assert_eq!(profile.refcount.load(Ordering::Relaxed), 2);

        profile.dec_ref();
        assert_eq!(profile.refcount.load(Ordering::Relaxed), 1);
    }
}
