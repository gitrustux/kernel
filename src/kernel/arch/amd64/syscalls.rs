// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86-64 System Call Interface
//!
//! This module provides the system call entry point and dispatch for AMD64.
//! It uses the `syscall` and `sysret` instructions for fast system calls.

use crate::kernel::arch::amd64::X86Iframe;
use crate::rustux::types::*;

/// System call numbers
///
/// These match the Linux x86-64 system call numbering for compatibility
pub mod syscall_nr {
    pub const SYS_READ: usize = 0;
    pub const SYS_WRITE: usize = 1;
    pub const SYS_OPEN: usize = 2;
    pub const SYS_CLOSE: usize = 3;
    pub const SYS_STAT: usize = 4;
    pub const SYS_FSTAT: usize = 5;
    pub const SYS_POLL: usize = 7;
    pub const SYS_MMAP: usize = 9;
    pub const SYS_MPROTECT: usize = 10;
    pub const SYS_MUNMAP: usize = 11;
    pub const SYS_BRK: usize = 12;
    pub const SYS_RT_SIGACTION: usize = 13;
    pub const SYS_RT_SIGPROCMASK: usize = 14;
    pub const SYS_IOCTL: usize = 16;
    pub const SYS_PREAD64: usize = 17;
    pub const SYS_PWRITE64: usize = 18;
    pub const SYS_READV: usize = 19;
    pub const SYS_WRITEV: usize = 20;
    pub const SYS_ACCESS: usize = 21;
    pub const SYS_PIPE: usize = 22;
    pub const SYS_SELECT: usize = 23;
    pub const SYS_SCHED_YIELD: usize = 24;
    pub const SYS_MREMAP: usize = 25;
    pub const SYS_MSYNC: usize = 26;
    pub const SYS_MINCORE: usize = 27;
    pub const SYS_MADVISE: usize = 28;
    pub const SYS_SHMGET: usize = 29;
    pub const SYS_SHMAT: usize = 30;
    pub const SYS_SHMCTL: usize = 31;
    pub const SYS_DUP: usize = 32;
    pub const SYS_DUP2: usize = 33;
    pub const SYS_PAUSE: usize = 34;
    pub const SYS_NANOSLEEP: usize = 35;
    pub const SYS_GETPID: usize = 39;
    pub const SYS_SOCKET: usize = 41;
    pub const SYS_CONNECT: usize = 42;
    pub const SYS_ACCEPT: usize = 43;
    pub const SYS_SENDTO: usize = 44;
    pub const SYS_RECVFROM: usize = 45;
    pub const SYS_SENDMSG: usize = 46;
    pub const SYS_RECVMSG: usize = 47;
    pub const SYS_SHUTDOWN: usize = 48;
    pub const SYS_BIND: usize = 49;
    pub const SYS_LISTEN: usize = 50;
    pub const SYS_GETSOCKNAME: usize = 51;
    pub const SYS_GETPEERNAME: usize = 52;
    pub const SYS_SOCKETPAIR: usize = 53;
    pub const SYS_SETSOCKOPT: usize = 54;
    pub const SYS_GETSOCKOPT: usize = 55;
    pub const SYS_CLONE: usize = 56;
    pub const SYS_FORK: usize = 57;
    pub const SYS_VFORK: usize = 58;
    pub const SYS_EXECVE: usize = 59;
    pub const SYS_EXIT: usize = 60;
    pub const SYS_WAIT4: usize = 61;
    pub const SYS_KILL: usize = 62;
    pub const SYS_UNAME: usize = 63;

    // Rustux-specific system calls
    pub const SYS_RUSTUX_DEBUG_PRINT: usize = 1000;
    pub const SYS_RUSTUX_THREAD_CREATE: usize = 1001;
    pub const SYS_RUSTUX_THREAD_EXIT: usize = 1002;
    pub const SYS_RUSTUX_THREAD_YIELD: usize = 1003;
}

/// System call return values
pub mod syscall_ret {
    pub const OK: i64 = 0;
    pub const ENOSYS: i64 = -38;  // Function not implemented
    pub const EINVAL: i64 = -22;  // Invalid argument
    pub const EPERM: i64 = -1;    // Operation not permitted
    pub const EFAULT: i64 = -14;  // Bad address
    pub const ENOMEM: i64 = -12;  // Out of memory
    pub const EAGAIN: i64 = -11;  // Try again
}

/// Initialize the system call subsystem
///
/// This sets up the syscall MSR (MSR_LSTAR) to point to our syscall entry point.
pub fn syscall_init() {
    unsafe {
        use crate::kernel::arch::amd64::mmu;

        // MSR for syscall entry point (RIP in long mode)
        const IA32_LSTAR_MSR: u32 = 0xC0000082;

        // Set the syscall entry point
        let syscall_entry = x86_syscall_entry as u64;
        mmu::x86_write_msr(IA32_LSTAR_MSR, syscall_entry);

        // TODO: Set up IA32_STAR_MSR for compatibility mode syscalls
        // TODO: Set up IA32_FMASK_MSR for RFLAG masking
    }
}

/// System call entry point (called from assembly)
///
/// # Arguments
///
/// * `frame` - Pointer to the interrupt frame containing register state
///
/// # Returns
///
/// The return value to be placed in RAX
#[no_mangle]
pub unsafe extern "C" fn x86_syscall_entry(frame: *mut X86Iframe) -> i64 {
    let frame = &mut *frame;

    // System call number is in RAX
    let syscall_nr = frame.rax as usize;

    // Arguments are in RDI, RSI, RDX, R10, R8, R9
    let arg1 = frame.rdi;
    let arg2 = frame.rsi;
    let arg3 = frame.rdx;
    let arg4 = frame.r10;
    let arg5 = frame.r8;
    let arg6 = frame.r9;

    // Dispatch the system call
    let result = dispatch_syscall(syscall_nr, arg1, arg2, arg3, arg4, arg5, arg6);

    // Update RAX with return value
    frame.rax = result as u64;

    result
}

/// Dispatch a system call to its handler
///
/// # Arguments
///
/// * `nr` - System call number
/// * `a1`-`a6` - System call arguments
///
/// # Returns
///
/// The system call return value (negative for errors)
fn dispatch_syscall(
    nr: usize,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    _a6: u64,
) -> i64 {
    match nr {
        // Basic I/O
        syscall_nr::SYS_READ => sys_read(a1, a2, a3),
        syscall_nr::SYS_WRITE => sys_write(a1, a2, a3),

        // Process management
        syscall_nr::SYS_EXIT => sys_exit(a1 as i32),
        syscall_nr::SYS_GETPID => sys_getpid(),

        // Thread management (Rustux-specific)
        syscall_nr::SYS_RUSTUX_THREAD_YIELD => sys_thread_yield(),

        // Debug output (Rustux-specific)
        syscall_nr::SYS_RUSTUX_DEBUG_PRINT => sys_debug_print(a1, a2),

        // TODO: Implement more system calls
        _ => {
            // Unimplemented system call
            syscall_ret::ENOSYS
        }
    }
}

// ============================================================================
// System Call Implementations
// ============================================================================

/// Read from a file descriptor
fn sys_read(_fd: u64, _buf: u64, _count: u64) -> i64 {
    // TODO: Implement proper file I/O
    syscall_ret::ENOSYS
}

/// Write to a file descriptor
fn sys_write(fd: u64, buf: u64, count: u64) -> i64 {
    // fd 1 is stdout, fd 2 is stderr
    if fd == 1 || fd == 2 {
        // TODO: Validate user pointer and copy data
        // For now, just return the count as if we wrote it
        return count as i64;
    }

    syscall_ret::EINVAL
}

/// Exit the current process
fn sys_exit(_code: i32) -> ! {
    // TODO: Implement proper process exit
    // For now, just halt
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

/// Get process ID
fn sys_getpid() -> i64 {
    // TODO: Implement proper process management
    // For now, return 1 (init process)
    1
}

/// Yield the current thread
fn sys_thread_yield() -> i64 {
    // TODO: Implement proper thread yielding
    syscall_ret::OK
}

/// Debug print (Rustux-specific)
///
/// Prints a null-terminated string from user space
fn sys_debug_print(_str: u64, _len: u64) -> i64 {
    // TODO: Validate user pointer and safely read string
    // For now, return success
    syscall_ret::OK
}

/// Save all general-purpose registers (for syscall context switch)
///
/// This is called from assembly to save the full user register state
/// before entering the kernel.
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "sysv64" fn x86syscall_save_all_gsbase() -> ! {
    core::arch::naked_asm!(
        "swapgs",                    // Swap to kernel GS
        "mov gs:[8], rsp",          // Save user RSP to kernel stack
        "push rax",                  // Save RAX (syscall number)
        "push rcx",                  // Save RCX (return address)
        "push r11",                  // Save R11 (RFLAGS)
        "push rbp",                  // Save frame pointer
        "jmp x86_syscall_entry",     // Jump to C entry
    );
}

/// Restore all general-purpose registers and return to user space
///
/// This is called from assembly to restore the user register state
/// and return from the system call.
#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "sysv64" fn x86syscall_restore_all_gsbase() -> ! {
    core::arch::naked_asm!(
        "pop rbp",                   // Restore frame pointer
        "pop r11",                   // Restore RFLAGS
        "pop rcx",                   // Restore return address to RCX
        "pop rax",                   // Restore return value to RAX
        "mov gs:[8], rsp",           // Restore user RSP
        "swapgs",                    // Swap back to user GS
        "sysretq",                   // Return to user space
    );
}

/// Get the kernel GS base value
///
/// Returns the base address of the kernel per-CPU data area
#[inline]
pub fn x86_get_gs_base() -> u64 {
    let gs_base: u64;
    unsafe {
        core::arch::asm!(
            "mov {}, gs:[0]",
            out(reg) gs_base,
            options(nostack, pure, readonly)
        );
    }
    gs_base
}

/// Set the kernel GS base value
///
/// # Safety
///
/// Must be called with a valid kernel per-CPU data address
#[inline]
pub unsafe fn x86_set_gs_base(base: u64) {
    core::arch::asm!(
        "mov gs:[0], {}",
        in(reg) base,
        options(nostack)
    );
}
