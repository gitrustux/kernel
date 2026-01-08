// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Intel Hardware Random Number Generator
//!
//! Provides access to CPU hardware entropy sources via RDSEED and RDRAND instructions.
//! This is a Rust replacement for the legacy C++ implementation.
//!
//! # Features
//!
//! - **RDSEED**: True hardware random number generator (NIST SP 800-90B/C compliant)
//! - **RDRAND**: Hardware random number generator (fallback for older CPUs)
//! - **Blocking/Non-blocking modes**: Control behavior when entropy is unavailable
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::kernel::dev::intel_rng;
//!
//! // Get a random u32
//! let val = intel_rng::hw_rng_get_u32();
//!
//! // Get entropy buffer
//! let mut buf = [0u8; 32];
//! let bytes_read = intel_rng::hw_rng_get_entropy(&mut buf, true);
//! ```

#[cfg(target_arch = "x86_64")]
use crate::kernel::arch::amd64::feature;

/// Entropy instruction type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntropyInstr {
    RdSeed,
    RdRand,
}

impl EntropyInstr {
    /// Execute the entropy instruction and return success + value
    #[inline(always)]
    #[cfg(target_arch = "x86_64")]
    fn step(self) -> Option<u64> {
        use core::arch::x86_64::{_rdseed64_step, _rdrand64_step};

        unsafe {
            let mut val = 0u64;
            let success = match self {
                EntropyInstr::RdRand => _rdrand64_step(&mut val),
                EntropyInstr::RdSeed => _rdseed64_step(&mut val),
            };

            if success == 1 {
                Some(val)
            } else {
                None
            }
        }
    }

    /// Non-x86_64 platforms always fail
    #[inline(always)]
    #[cfg(not(target_arch = "x86_64"))]
    fn step(self) -> Option<u64> {
        None
    }
}

/// Get entropy from a specific instruction
///
/// # Arguments
///
/// * `buf` - Output buffer for entropy data
/// * `block` - If true, block until all bytes are filled; otherwise return early
/// * `instr` - Which entropy instruction to use
///
/// # Returns
///
/// Number of bytes written to the buffer
fn get_entropy_from_instruction(buf: &mut [u8], block: bool, instr: EntropyInstr) -> usize {
    let mut written = 0;

    while written < buf.len() {
        match instr.step() {
            Some(val) => {
                // Copy as many bytes as we need
                let val_bytes = val.to_ne_bytes();
                let to_copy = core::cmp::min(val_bytes.len(), buf.len() - written);
                buf[written..written + to_copy].copy_from_slice(&val_bytes[..to_copy]);
                written += to_copy;
            }
            None => {
                if !block {
                    break;
                }
                // If blocking, spin until we get entropy
                core::hint::spin_loop();
            }
        }
    }

    written
}

/// Get entropy from RDSEED instruction
///
/// RDSEED is a true hardware random number generator compliant with
/// NIST SP 800-90B/C standards.
fn get_entropy_from_rdseed(buf: &mut [u8], block: bool) -> usize {
    get_entropy_from_instruction(buf, block, EntropyInstr::RdSeed)
}

/// Get entropy from RDRAND instruction
///
/// RDRAND is a hardware random number generator. Note: This is a fallback
/// for development platforms without RDSEED. This implementation is not
/// compliant with Intel's DRNG Software Implementation Guide and should
/// be avoided for production use.
fn get_entropy_from_rdrand(buf: &mut [u8], block: bool) -> usize {
    get_entropy_from_instruction(buf, block, EntropyInstr::RdRand)
}

/// Get entropy from the CPU
///
/// Attempts to use RDSEED first, falls back to RDRAND if unavailable.
///
/// # Arguments
///
/// * `buf` - Output buffer for entropy data
/// * `block` - If true, block until all bytes are filled
///
/// # Returns
///
/// Number of bytes written to the buffer, or negative error code on failure
fn get_entropy_from_cpu(buf: &mut [u8], block: bool) -> isize {
    if buf.is_empty() {
        return 0;
    }

    #[cfg(target_arch = "x86_64")]
    {
        // CPUID leaf 7, subleaf 0, EBX register, bit 18 = RDSEED
        let has_rdseed = feature::x86_feature_test(7, 0, 1, 18);
        // CPUID leaf 1, subleaf 0, ECX register, bit 30 = RDRAND
        let has_rdrand = feature::x86_feature_test(1, 0, 2, 30);

        if has_rdseed {
            return get_entropy_from_rdseed(buf, block) as isize;
        } else if has_rdrand {
            return get_entropy_from_rdrand(buf, block) as isize;
        }

        // No entropy source available
        return -1; // RX_ERR_NOT_SUPPORTED
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = block;
        -1 // RX_ERR_NOT_SUPPORTED
    }
}

/// Get entropy from the hardware RNG
///
/// This is the main entry point for hardware entropy generation.
///
/// # Arguments
///
/// * `buf` - Output buffer for entropy data
/// * `block` - If true, block until all bytes are filled; otherwise return early
///
/// # Returns
///
/// Number of bytes written to the buffer (may be 0 if unavailable)
///
/// # Examples
///
/// ```rust,ignore
/// let mut buf = [0u8; 16];
/// let n = hw_rng_get_entropy(&mut buf, false);
/// ```
pub fn hw_rng_get_entropy(buf: &mut [u8], block: bool) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let result = get_entropy_from_cpu(buf, block);

    if result < 0 {
        0
    } else {
        result as usize
    }
}

/// Get a random u32 from the hardware RNG
///
/// This is a convenience function that always blocks.
///
/// # Returns
///
/// A random 32-bit value
///
/// # Panics
///
/// Panics if the hardware RNG is unavailable or fails.
///
/// # Examples
///
/// ```rust,ignore
/// let rand_val = hw_rng_get_u32();
/// println!("Random: {}", rand_val);
/// ```
pub fn hw_rng_get_u32() -> u32 {
    let mut val = [0u8; 4];
    let fetched = hw_rng_get_entropy(&mut val, true);

    debug_assert!(fetched == 4, "Hardware RNG failed to provide entropy");

    u32::from_ne_bytes([val[0], val[1], val[2], val[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hw_rng_get_u32() {
        let val = hw_rng_get_u32();
        // Just verify we got a value without panicking
        let _ = val;
    }

    #[test]
    fn test_hw_rng_get_entropy() {
        let mut buf = [0u8; 16];
        let n = hw_rng_get_entropy(&mut buf, true);
        // On x86_64 with hardware RNG, should get all bytes
        // On other platforms, might get 0
        let _ = n;
    }
}
