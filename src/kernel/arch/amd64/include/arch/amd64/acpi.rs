// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ACPI interface definitions for the Rustux microkernel

use crate::arch::amd64::bootstrap16::X86RealModeEntryDataRegisters;

/// ACPI status code type imported from the ACPICA library
pub type AcpiStatus = i32; // Assuming this is the correct type from ACPICA

/// Error codes for ACPI operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiError {
    InvalidState,
    HardwareFailure,
    NotSupported,
    InvalidArgument,
}

/// Result type for ACPI operations
pub type AcpiResult<T> = Result<T, AcpiError>;

/// Initiates a transition to the requested ACPI S-state.
///
/// # Safety
///
/// This function is unsafe because:
/// - It must not be called before bootstrap16 is configured to handle the resume.
/// - It must be called from a kernel thread, unless it is a transition to
///   S5 (poweroff). Failure to do so may result in loss of usermode register state.
///
/// # Arguments
///
/// * `regs` - Register state to be used during resume
/// * `target_s_state` - The ACPI S-state to transition to
/// * `sleep_type_a` - The SLP_TYPa value for the target sleep state
/// * `sleep_type_b` - The SLP_TYPb value for the target sleep state
pub unsafe fn x86_acpi_transition_s_state(
    regs: &mut X86RealModeEntryDataRegisters,
    target_s_state: u8,
    sleep_type_a: u8,
    sleep_type_b: u8,
) -> AcpiStatus {
    // This is a wrapper around the C function that would be implemented elsewhere
    // The actual implementation would need to be linked with the ACPICA library
    extern "C" {
        fn x86_acpi_transition_s_state(
            regs: *mut X86RealModeEntryDataRegisters,
            target_s_state: u8,
            sleep_type_a: u8,
            sleep_type_b: u8,
        ) -> AcpiStatus;
    }

    x86_acpi_transition_s_state(regs, target_s_state, sleep_type_a, sleep_type_b)
}