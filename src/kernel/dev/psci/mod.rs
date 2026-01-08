// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM Power State Coordination Interface (PSCI)
//!
//! Provides interface to ARM PSCI firmware for power management operations.
//! This is a Rust replacement for the legacy C++ implementation.
//!
//! # Features
//!
//! - System power control (system off, system reset)
//! - CPU power management (CPU on, CPU off, CPU suspend)
//! - Affinity info queries
//! - SMC/HVC calling convention support
//!
//! # PSCI Functions
//!
//! - `PSCI_VERSION` - Get PSCI version
//! - `CPU_SUSPEND` - Suspend a CPU
//! - `CPU_OFF` - Power down calling CPU
//! - `CPU_ON` - Power on a CPU
//! - `AFFINITY_INFO` - Get CPU affinity state
//! - `SYSTEM_OFF` - Power down the system
//! - `SYSTEM_RESET` - Reset the system
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::kernel::dev::psci;
//!
//! // Power off the system
//! psci::system_off();
//!
//! // Reset the system
//! psci::system_reset(psci::RebootFlags::Normal);
//! ```

use crate::kernel::vm::PAddr;

/// PSCI function identifiers (64-bit)
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum PsciFunction {
    /// PSCI version
    PsciVersion = 0x84000000,

    /// CPU suspend
    CpuSuspend = 0xC4000001,

    /// CPU off
    CpuOff = 0x84000002,

    /// CPU on
    CpuOn = 0xC4000003,

    /// Affinity info
    AffinityInfo = 0xC4000004,

    /// Migrate
    Migrate = 0xC4000005,

    /// Migrate info type
    MigrateInfoType = 0x84000006,

    /// Migrate info UP CPU
    MigrateInfoUpCpu = 0xC4000007,

    /// System off
    SystemOff = 0x84000008,

    /// System reset
    SystemReset = 0x84000009,

    /// PSCI features
    PsciFeatures = 0x8400000A,

    /// CPU freeze
    CpuFreeze = 0x8400000B,

    /// CPU default suspend
    CpuDefaultSuspend = 0xC400000C,

    /// Node HW state
    NodeHwState = 0xC400000D,

    /// System suspend
    SystemSuspend = 0xC400000E,

    /// PSCI set suspend mode
    PsciSetSuspendMode = 0x8400000F,

    /// PSCI stat residency
    PsciStatResidency = 0xC4000010,

    /// PSCI stat count
    PsciStatCount = 0xC4000011,
}

/// PSCI return codes
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsciReturn {
    Success = 0,
    NotSupported = -1,
    InvalidParameters = -2,
    Denied = -3,
    AlreadyOn = -4,
    OnPending = -5,
    InternalFailure = -6,
    NotPresent = -7,
    Disabled = -8,
    InvalidAddress = -9,
}

/// Reboot flags for system reset
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebootFlags {
    /// Normal reboot
    Normal = 0,

    /// Reboot into bootloader
    Bootloader = 1,

    /// Reboot into recovery mode
    Recovery = 2,
}

/// PSCI calling convention
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsciCallType {
    /// Use SMC (Secure Monitor Call)
    Smc,

    /// Use HVC (Hypervisor Call)
    Hvc,
}

/// PSCI driver state
struct PsciState {
    /// Calling convention to use
    call_type: PsciCallType,

    /// Arguments for system off
    shutdown_args: [u64; 3],

    /// Arguments for normal reboot
    reboot_args: [u64; 3],

    /// Arguments for bootloader reboot
    reboot_bootloader_args: [u64; 3],

    /// Arguments for recovery reboot
    reboot_recovery_args: [u64; 3],
}

/// Global PSCI state
static mut PSCI_STATE: PsciState = PsciState {
    call_type: PsciCallType::Smc,
    shutdown_args: [0, 0, 0],
    reboot_args: [0, 0, 0],
    reboot_bootloader_args: [0, 0, 0],
    reboot_recovery_args: [0, 0, 0],
};

/// Make a PSCI call using the configured convention
///
/// # Safety
///
/// This function performs raw SMC/HVC calls and should only be called
/// with valid function identifiers and arguments.
unsafe fn psci_call(
    function: u32,
    arg0: u64,
    arg1: u64,
    arg2: u64,
) -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        #[repr(C)]
        struct ArmSmcccResult {
            x0: u64,
            x1: u64,
            x2: u64,
            x3: u64,
        }

        extern "C" {
            fn arm_smccc_smc(
                w0: u32,
                x1: u64, x2: u64,
                x3: u64, x4: u64,
                x5: u64, x6: u64,
                w7: u32,
            ) -> ArmSmcccResult;

            fn arm_smccc_hvc(
                w0: u32,
                x1: u64, x2: u64,
                x3: u64, x4: u64,
                x5: u64, x6: u64,
                w7: u32,
            ) -> ArmSmcccResult;
        }

        match PSCI_STATE.call_type {
            PsciCallType::Smc => {
                let result = arm_smccc_smc(
                    function as u32,
                    arg0,
                    arg1,
                    arg2,
                    0,
                    0,
                    0,
                    0,
                );
                result.x0
            }
            PsciCallType::Hvc => {
                let result = arm_smccc_hvc(
                    function as u32,
                    arg0,
                    arg1,
                    arg2,
                    0,
                    0,
                    0,
                    0,
                );
                result.x0
            }
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = (function, arg0, arg1, arg2);
        0
    }
}

/// Get PSCI version
///
/// Returns the PSCI version number.
/// The format is: major_version (bits 31-16) | minor_version (bits 15-0)
pub fn psci_get_version() -> u32 {
    unsafe { psci_call(PsciFunction::PsciVersion as u32, 0, 0, 0) as u32 }
}

/// Power down the calling CPU
///
/// This function powers down the CPU that calls it. It will only return
/// if the call fails.
///
/// # Returns
///
/// PSCI return code
pub fn psci_cpu_off() -> PsciReturn {
    unsafe {
        let result = psci_call(PsciFunction::CpuOff as u32, 0, 0, 0) as i32;
        match result {
            0 => PsciReturn::Success,
            -1 => PsciReturn::NotSupported,
            -2 => PsciReturn::InvalidParameters,
            -3 => PsciReturn::Denied,
            -4 => PsciReturn::AlreadyOn,
            -5 => PsciReturn::OnPending,
            -6 => PsciReturn::InternalFailure,
            -7 => PsciReturn::NotPresent,
            -8 => PsciReturn::Disabled,
            -9 => PsciReturn::InvalidAddress,
            _ => PsciReturn::InternalFailure,
        }
    }
}

/// Power on a CPU
///
/// # Arguments
///
/// * `cluster` - Cluster number
/// * `cpuid` - CPU ID within the cluster
/// * `entry` - Physical address of entry point
///
/// # Returns
///
/// PSCI return code
pub fn psci_cpu_on(cluster: u64, cpuid: u64, entry: PAddr) -> PsciReturn {
    #[cfg(target_arch = "aarch64")]
    let mpid = {
        use crate::kernel::arch::arm64::ARM64_MPID;
        ARM64_MPID(cluster, cpuid)
    };

    #[cfg(not(target_arch = "aarch64"))]
    let mpid = cluster << 8 | cpuid;

    unsafe {
        let result = psci_call(PsciFunction::CpuOn as u32, mpid, entry as u64, 0) as i32;
        match result {
            0 => PsciReturn::Success,
            -1 => PsciReturn::NotSupported,
            -2 => PsciReturn::InvalidParameters,
            -3 => PsciReturn::Denied,
            -4 => PsciReturn::AlreadyOn,
            -5 => PsciReturn::OnPending,
            -6 => PsciReturn::InternalFailure,
            -7 => PsciReturn::NotPresent,
            -8 => PsciReturn::Disabled,
            -9 => PsciReturn::InvalidAddress,
            _ => PsciReturn::InternalFailure,
        }
    }
}

/// Get affinity info for a CPU
///
/// # Arguments
///
/// * `cluster` - Cluster number
/// * `cpuid` - CPU ID within the cluster
///
/// # Returns
///
/// PSCI return code (0 = OFF, 1 = ON_PENDING, 2 = ON)
pub fn psci_get_affinity_info(cluster: u64, cpuid: u64) -> i32 {
    #[cfg(target_arch = "aarch64")]
    let mpid = {
        use crate::kernel::arch::arm64::ARM64_MPID;
        ARM64_MPID(cluster, cpuid)
    };

    #[cfg(not(target_arch = "aarch64"))]
    let mpid = cluster << 8 | cpuid;

    unsafe { psci_call(PsciFunction::AffinityInfo as u32, mpid, 0, 0) as i32 }
}

/// System off - power down the system
///
/// This function powers down the entire system and does not return.
pub fn system_off() {
    crate::log_info!("PSCI: System powering off...");

    unsafe {
        psci_call(
            PsciFunction::SystemOff as u32,
            PSCI_STATE.shutdown_args[0],
            PSCI_STATE.shutdown_args[1],
            PSCI_STATE.shutdown_args[2],
        );
    }

    // Should not reach here
    crate::log_warn!("PSCI system_off returned unexpectedly");
}

/// System reset - reboot the system
///
/// # Arguments
///
/// * `flags` - Reboot flags
///
/// This function reboots the system and does not return.
pub fn system_reset(flags: RebootFlags) {
    let args = match flags {
        RebootFlags::Normal => &unsafe { PSCI_STATE.reboot_args },
        RebootFlags::Bootloader => &unsafe { PSCI_STATE.reboot_bootloader_args },
        RebootFlags::Recovery => &unsafe { PSCI_STATE.reboot_recovery_args },
    };

    crate::log_info!("PSCI: System resetting (flags={:?})", flags);

    unsafe {
        psci_call(
            PsciFunction::SystemReset as u32,
            args[0],
            args[1],
            args[2],
        );
    }

    // Should not reach here
    crate::log_warn!("PSCI system_reset returned unexpectedly");
}

/// Initialize PSCI driver
///
/// # Arguments
///
/// * `use_hvc` - true to use HVC calls, false to use SMC calls
/// * `shutdown_args` - Arguments for system off
/// * `reboot_args` - Arguments for normal reboot
/// * `reboot_bootloader_args` - Arguments for bootloader reboot
/// * `reboot_recovery_args` - Arguments for recovery reboot
pub fn init(
    use_hvc: bool,
    shutdown_args: [u64; 3],
    reboot_args: [u64; 3],
    reboot_bootloader_args: [u64; 3],
    reboot_recovery_args: [u64; 3],
) {
    unsafe {
        PSCI_STATE.call_type = if use_hvc {
            PsciCallType::Hvc
        } else {
            PsciCallType::Smc
        };
        PSCI_STATE.shutdown_args = shutdown_args;
        PSCI_STATE.reboot_args = reboot_args;
        PSCI_STATE.reboot_bootloader_args = reboot_bootloader_args;
        PSCI_STATE.reboot_recovery_args = reboot_recovery_args;
    }

    let version = psci_get_version();
    crate::log_info!("PSCI initialized (version={:#x}, call_type={:?})",
        version,
        unsafe { PSCI_STATE.call_type }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psci_function_values() {
        assert_eq!(PsciFunction::PsciVersion as u32, 0x84000000);
        assert_eq!(PsciFunction::SystemOff as u32, 0x84000008);
        assert_eq!(PsciFunction::SystemReset as u32, 0x84000009);
    }

    #[test]
    fn test_psci_return_codes() {
        assert_eq!(PsciReturn::Success as i32, 0);
        assert_eq!(PsciReturn::NotSupported as i32, -1);
        assert_eq!(PsciReturn::InvalidParameters as i32, -2);
    }

    #[test]
    fn test_reboot_flags() {
        assert_eq!(RebootFlags::Normal as u32, 0);
        assert_eq!(RebootFlags::Bootloader as u32, 1);
        assert_eq!(RebootFlags::Recovery as u32, 2);
    }
}
