// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! C Runtime (CRT0) for Rustux
//!
//! This module provides the process entry point and initialization
//! code for C and C++ programs running on Rustux.
//!
//! # Process Startup
//!
//! When a process is created, the kernel sets up:
//! 1. The stack with arguments and environment
//! 2. The instruction pointer to this entry point
//! 3. Thread-local storage (if configured)
//!
//! The entry point then:
//! 1. Initializes the C runtime
//! 2. Calls global constructors
//! 3. Calls main()
//! 4. Calls global destructors
//! 5. Exits the process

#![no_std]

// Startup functions
extern "C" {
    /// The main function (user-provided)
    fn main(argc: i32, argv: *const *const u8) -> i32;
}

/// Process entry point
///
/// This is the first function executed when a process starts.
/// It's called by the kernel with the following setup:
///
/// # Arguments
///
/// * `argc` - Argument count
/// * `argv` - Argument vector (pointers to null-terminated strings)
/// * `envp` - Environment vector (optional)
#[no_mangle]
pub unsafe extern "C" fn _start(
    arg_c: usize,
    arg_v: usize,
) -> ! {
    // Convert arguments to proper types
    let argc = arg_c as i32;
    let argv = arg_v as *const *const u8;

    // Initialize the C runtime
    crt_init();

    // Call global constructors (if any)
    // TODO: Implement constructor support via linker script

    // Call main
    let status = main(argc, argv);

    // Call global destructors (if any)
    // TODO: Implement destructor support

    // Exit the process
    libsys::Process::exit(status)
}

/// C runtime initialization
unsafe fn crt_init() {
    // TODO: Initialize:
    // - Thread-local storage
    // - Global data (BSS, data segments)
    // - Standard library (stdio, malloc, etc.)
    // - Signal handlers
}

/// Thread local storage initialization
///
/// This is called for each new thread to set up TLS.
#[no_mangle]
pub unsafe extern "C" fn _tls_init() {
    // TODO: Initialize TLS for the current thread
}

/// Thread local storage cleanup
///
/// This is called when a thread exits.
#[no_mangle]
pub unsafe extern "C" fn _tls_fini() {
    // TODO: Clean up TLS for the current thread
}

/// Panic handler for runtime code
#[cfg(target_arch = "x86_64")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        libsys::Process::exit(1);
    }
}

/// Out of memory handler
#[alloc_error_handler]
fn oom(layout: core::alloc::Layout) -> ! {
    unsafe {
        libsys::Process::exit(1);
    }
}

// VDSO Integration
//
// The VDSO (Virtual Dynamic Shared Object) is a shared library
// mapped into every process's address space by the kernel.
// It provides fast access to kernel functions without syscalls.

/// VDSO structure
///
/// This is the layout of the VDSO as provided by the kernel.
#[repr(C)]
pub struct Vdso {
    /// Version of the VDSO structure
    pub version: u32,

    /// Get monotonic clock (nanoseconds since boot)
    pub clock_get_monotonic: unsafe extern "C" fn() -> u64,

    /// Get real-time clock (wall-clock time)
    pub clock_get_realtime: unsafe extern "C" fn() -> u64,

    /// Get system call number for a function
    pub get_syscall_number: unsafe extern "C" fn(&str) -> u32,

    /// Reserved for future expansion
    pub reserved: [u64; 8],
}

/// Get the VDSO pointer
///
/// The VDSO is always mapped at a fixed location (e.g., 0x1000_0000).
/// This function returns a reference to it.
pub unsafe fn get_vdso() -> &'static Vdso {
    // Fixed VDSO location (kernel-configurable)
    const VDSO_ADDR: usize = 0x1000_0000;

    &*(VDSO_ADDR as *const Vdso)
}

/// Get current time using VDSO
///
/// This is a fast alternative to making a syscall.
#[inline]
pub fn vdso_clock_monotonic() -> u64 {
    unsafe {
        let vdso = get_vdso();
        (vdso.clock_get_monotonic)()
    }
}

/// Get real-time using VDSO
///
/// This is a fast alternative to making a syscall.
#[inline]
pub fn vdso_clock_realtime() -> u64 {
    unsafe {
        let vdso = get_vdso();
        (vdso.clock_get_realtime)()
    }
}

/// Get syscall number using VDSO
///
/// This avoids having to hardcode syscall numbers in userspace.
pub fn vdso_syscall_number(name: &str) -> u32 {
    unsafe {
        let vdso = get_vdso();
        (vdso.get_syscall_number)(name)
    }
}

// Stack setup
//
// The kernel sets up the initial stack with the following layout:
//
// +------------------+  <- Top of stack (high address)
// | envp[n]          |    NULL
// | envp[n-1]        |    Pointer to environment string
// | ...              |    ...
// | envp[0]          |
// | NULL             |
// | argv[argc]       |    NULL
// | argv[argc-1]     |    Pointer to argument string
// | ...              |
// | argv[0]          |
// | argc             |    Argument count
// +------------------+  <- Bottom of stack (low address)
// | auxv             |    Auxiliary vector
// +------------------+

/// Auxiliary vector entry type
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum AuxvEntry {
    /// Program header table entry
    Phent(u64),
    /// Number of program header entries
    Phnum(u64),
    /// Page size
    PageSize(u64),
    /// Base address of interpreter
    Base(u64),
    /// Flags
    Flags(u64),
    /// Entry point
    Entry(u64),
    /// Program header table
    Phdr(u64),
    /// ELF hash table
    GnuHash(u64),
    /// String table
    StrTab(u64),
    /// Symbol table
    SymTab(u64),
    /// Jump relocation table
    JumpRel(u64),
    /// Process ID
    Pid(u64),
    /// Real user ID
    Uid(u64),
    /// Effective user ID
    Euid(u64),
    /// Real group ID
    Gid(u64),
    /// Effective group ID
    Egid(u64),
    /// Thread pointer
    TlsAddr(u64),
    /// Stack pointer
    Stack(u64),
    /// Secure bit
    Secure(u64),
    /// Random bytes
    Random(u64),
    /// Executable filename
    ExecFn(u64),
    /// NULL terminator
    Null,
}

/// Parse the auxiliary vector from the stack
///
/// # Safety
///
/// The stack pointer must point to a valid auxiliary vector.
pub unsafe fn parse_auxv(stack: *const u8) -> impl Iterator<Item = AuxvEntry> + '_ {
    // Skip argc and argv
    let mut argc_ptr = stack as *const usize;
    let argc = *argc_ptr;
    argc_ptr = argc_ptr.add(1);

    // Skip argv
    argc_ptr = argc_ptr.add(argc as usize);
    argc_ptr = argc_ptr.add(1); // Skip NULL

    // Skip envp
    while *argc_ptr != 0 {
        argc_ptr = argc_ptr.add(1);
    }
    argc_ptr = argc_ptr.add(1); // Skip NULL

    // Now we're at auxv
    let mut auxv_ptr = argc_ptr as *const (u64, u64);

    AuxvIterator {
        auxv_ptr,
        _phantom: core::marker::PhantomData,
    }
}

/// Auxiliary vector iterator
struct AuxvIterator<'a> {
    auxv_ptr: *const (u64, u64),
    _phantom: core::marker::PhantomData<&'a ()>,
}

impl<'a> Iterator for AuxvIterator<'a> {
    type Item = AuxvEntry;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let (tag, value) = *self.auxv_ptr;
            self.auxv_ptr = self.auxv_ptr.add(1);

            if tag == 0 {
                None
            } else {
                Some(match tag {
                    3 => AuxvEntry::Phent(value),
                    4 => AuxvEntry::Phnum(value),
                    5 => AuxvEntry::PageSize(value),
                    6 => AuxvEntry::Base(value),
                    7 => AuxvEntry::Flags(value),
                    8 => AuxvEntry::Entry(value),
                    9 => AuxvEntry::Phdr(value),
                    0x6FFFFDF5 => AuxvEntry::GnuHash(value),
                    0x6FFFFDF6 => AuxvEntry::StrTab(value),
                    0x6FFFFDF8 => AuxvEntry::SymTab(value),
                    0x6FFFFDF9 => AuxvEntry::JumpRel(value),
                    0xB => AuxvEntry::Pid(value),
                    0x11 => AuxvEntry::Uid(value),
                    0x12 => AuxvEntry::Euid(value),
                    0x13 => AuxvEntry::Gid(value),
                    0x14 => AuxvEntry::Egid(value),
                    0x16 => AuxvEntry::TlsAddr(value),
                    0x17 => AuxvEntry::Stack(value),
                    0x18 => AuxvEntry::Secure(value),
                    0x19 => AuxvEntry::Random(value),
                    0x1D => AuxvEntry::ExecFn(value),
                    _ => AuxvEntry::Null,
                })
            }
        }
    }
}
