// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Version Information
//!
//! This module provides version information display functionality.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::rustux::types::*;
use crate::kernel::lib::console::{register_command, Cmd, CmdArg};

/// Maximum build ID string length (SHA256 would be 64 hex chars + null)
const MAX_BUILD_ID_STRING_LEN: usize = 65;

/// ELF build ID note header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BuildIdNote {
    /// Name size
    pub namesz: u32,
    /// Descriptor size
    pub descsz: u32,
    /// Note type
    pub type_: u32,
    /// Note name (padded to 4-byte boundary)
    pub name: [u8; 8],
    /// Build ID bytes follow
}

/// Version information structure
#[repr(C)]
#[derive(Debug)]
pub struct VersionInfo {
    /// Structure version
    pub struct_version: u32,
    /// Architecture string
    pub arch: &'static str,
    /// Platform string
    pub platform: &'static str,
    /// Target string
    pub target: &'static str,
    /// Project string
    pub project: &'static str,
    /// Build ID string
    pub buildid: &'static str,
    /// ELF build ID as hex string
    pub elf_build_id: &'static str,
}

/// Global version information
static VERSION_INFO: VersionInfo = VersionInfo {
    struct_version: 1,
    arch: env!("RUSTUX_ARCH"),
    platform: env!("RUSTUX_PLATFORM"),
    target: env!("TARGET"),
    project: env!("CARGO_PKG_NAME"),
    buildid: env!("CARGO_PKG_VERSION"),
    elf_build_id: "",
};

/// ELF build ID string buffer
static ELF_BUILD_ID_STRING: AtomicUsize = AtomicUsize::new(0);

/// Print version information
pub fn print_version() {
    println!("version:");
    println!("\tarch:     {}", VERSION_INFO.arch);
    println!("\tplatform: {}", VERSION_INFO.platform);
    println!("\ttarget:   {}", VERSION_INFO.target);
    println!("\tproject:  {}", VERSION_INFO.project);
    println!("\tbuildid:  {}", VERSION_INFO.buildid);

    let build_id = ELF_BUILD_ID_STRING.load(Ordering::Acquire);
    if build_id != 0 {
        // SAFETY: The build ID string is null-terminated
        unsafe {
            let s = core::ffi::CStr::from_ptr(build_id as *const i8);
            if let Ok(str) = s.to_str() {
                println!("\tELF build ID: {}", str);
            }
        }
    } else {
        println!("\tELF build ID: <not available>");
    }
}

/// Initialize ELF build ID from note section
///
/// This should be called during early boot initialization if
/// build ID information is available in the kernel binary.
pub fn init_elf_build_id() {
    // TODO: Parse ELF build ID note section
    // This requires reading from the kernel binary's note section
    // For now, we'll use a placeholder
    println!("Version: ELF build ID initialization not yet implemented");
}

/// Version command implementation
fn cmd_version(_argc: usize, _argv: &[CmdArg], _flags: u32) -> i32 {
    print_version();
    0
}

/// Print version during initialization (if debug enabled)
#[cfg(debug_assertions)]
pub fn print_version_init() {
    print_version();
}

/// Register version commands
pub fn version_register() {
    register_command(Cmd {
        name: "version",
        help: "print version information",
        func: Some(cmd_version),
        flags: 0,
    });
}

/// Initialize version module
pub fn init() {
    init_elf_build_id();
    version_register();

    #[cfg(debug_assertions)]
    print_version_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constants() {
        assert_eq!(MAX_BUILD_ID_STRING_LEN, 65);
    }

    #[test]
    fn test_version_info_fields() {
        assert!(!VERSION_INFO.arch.is_empty());
        assert!(!VERSION_INFO.platform.is_empty());
        assert!(!VERSION_INFO.target.is_empty());
        assert!(!VERSION_INFO.project.is_empty());
        assert!(!VERSION_INFO.buildid.is_empty());
    }

    #[test]
    fn test_build_id_note_size() {
        assert_eq!(core::mem::size_of::<BuildIdNote>(), 24);
    }
}
