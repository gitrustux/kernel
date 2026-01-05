// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ParaVirtualized Clock support for virtualized environments
//!
//! This module provides types and functions for interacting with the
//! paravirtualized clock provided by hypervisors like KVM and Xen.

use crate::rustux::types::*;

/// Old MSR number for KVM system time
pub const KVM_SYSTEM_TIME_MSR_OLD: u32 = 0x12;
/// Current MSR number for KVM system time
pub const KVM_SYSTEM_TIME_MSR: u32 = 0x4b564d01;

/// Old MSR number for KVM boot time
pub const KVM_BOOT_TIME_OLD: u32 = 0x11;
/// Current MSR number for KVM boot time
pub const KVM_BOOT_TIME: u32 = 0x4b564d00;

/// Old feature flag for KVM clock source
pub const KVM_FEATURE_CLOCK_SOURCE_OLD: u32 = 1 << 0;
/// Current feature flag for KVM clock source
pub const KVM_FEATURE_CLOCK_SOURCE: u32 = 1 << 3;

/// Flag indicating the KVM system time is stable
pub const KVM_SYSTEM_TIME_STABLE: u8 = 1 << 0;

/// Boot time information from the paravirtualized clock
///
/// With multiple VCPUs it is possible that one VCPU can try to read boot time
/// while we are updating it because another VCPU asked for the update. In this
/// case odd version value serves as an indicator for the guest that update is
/// in progress. Therefore we need to update version before we write anything
/// else and after, also we need to user proper memory barriers.
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct PvClockBootTime {
    /// Version number (odd values indicate update in progress)
    pub version: u32,
    /// Seconds component of the boot time
    pub seconds: u32,
    /// Nanoseconds component of the boot time
    pub nseconds: u32,
}

/// System time information from the paravirtualized clock
///
/// The version field follows the same update protocol as in the boot time
/// structure. Even though system time is per VCPU, other VCPUs can still
/// access system times of other VCPUs (though Linux never does that).
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct PvClockSystemTime {
    /// Version number (odd values indicate update in progress)
    pub version: u32,
    /// Padding for alignment
    pub pad0: u32,
    /// TSC timestamp
    pub tsc_timestamp: u64,
    /// System time value
    pub system_time: u64,
    /// TSC multiplier
    pub tsc_mul: u32,
    /// TSC shift
    pub tsc_shift: i8,
    /// Flags
    pub flags: u8,
    /// Padding for alignment
    pub pad1: [u8; 2],
}

// Static assertions to ensure the structures have the correct size
const _: () = assert!(core::mem::size_of::<PvClockBootTime>() == 12);
const _: () = assert!(core::mem::size_of::<PvClockSystemTime>() == 32);

/// Initialize the paravirtualized clock subsystem
///
/// # Returns
///
/// A status code indicating success or the type of failure
pub fn pvclock_init() -> RxStatus {
    unsafe { sys_pvclock_init() }
}

/// Check if paravirtualized clock is present
///
/// # Returns
///
/// `true` if the paravirtualized clock is available, `false` otherwise
pub fn pvclock_is_present() -> bool {
    unsafe { sys_pvclock_is_present() }
}

/// Check if the paravirtualized clock is stable
///
/// # Returns
///
/// `true` if the paravirtualized clock is stable, `false` otherwise
pub fn pvclock_is_stable() -> bool {
    unsafe { sys_pvclock_is_stable() }
}

/// Get the TSC frequency from the paravirtualized clock
///
/// # Returns
///
/// The TSC frequency in Hz
pub fn pvclock_get_tsc_freq() -> u64 {
    unsafe { sys_pvclock_get_tsc_freq() }
}

// External function declarations for the system implementations
extern "C" {
    fn sys_pvclock_init() -> RxStatus;
    fn sys_pvclock_is_present() -> bool;
    fn sys_pvclock_is_stable() -> bool;
    fn sys_pvclock_get_tsc_freq() -> u64;
}