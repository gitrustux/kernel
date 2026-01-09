// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit exception handlers
//!
//! This module provides exception handling for RISC-V, including
//! page faults, illegal instructions, and system calls.


use crate::arch::riscv64::registers::csr;
use crate::arch::riscv64::registers::scause;
use crate::debug;
use crate::kernel::thread;
use crate::print;
use crate::rustux::types::*;

/// System call numbers
///
/// These match the Linux RISC-V system call numbering for compatibility
pub mod syscall_nr {
    pub const SYS_READ: usize = 63;
    pub const SYS_WRITE: usize = 64;
    pub const SYS_OPEN: usize = 1024;
    pub const SYS_CLOSE: usize = 57;
    pub const SYS_STAT: usize = 1038;
    pub const SYS_FSTAT: usize = 80;
    pub const SYS_POLL: usize = 83;
    pub const SYS_MMAP: usize = 222;
    pub const SYS_MPROTECT: usize = 226;
    pub const SYS_MUNMAP: usize = 215;
    pub const SYS_BRK: usize = 214;
    pub const SYS_RT_SIGACTION: usize = 135;
    pub const SYS_RT_SIGPROCMASK: usize = 136;
    pub const SYS_IOCTL: usize = 29;
    pub const SYS_PREAD64: usize = 67;
    pub const SYS_PWRITE64: usize = 68;
    pub const SYS_READV: usize = 66;
    pub const SYS_WRITEV: usize = 65;
    pub const SYS_ACCESS: usize = 1033;
    pub const SYS_PIPE: usize = 59;
    pub const SYS_SELECT: usize = 52;
    pub const SYS_SCHED_YIELD: usize = 124;
    pub const SYS_MREMAP: usize = 216;
    pub const SYS_MSYNC: usize = 227;
    pub const SYS_MINCORE: usize = 232;
    pub const SYS_MADVISE: usize = 233;
    pub const SYS_SHMGET: usize = 194;
    pub const SYS_SHMAT: usize = 30;
    pub const SYS_SHMCTL: usize = 31;
    pub const SYS_DUP: usize = 23;
    pub const SYS_DUP2: usize = 24;
    pub const SYS_PAUSE: usize = 235;
    pub const SYS_NANOSLEEP: usize = 101;
    pub const SYS_GETPID: usize = 172;
    pub const SYS_SOCKET: usize = 198;
    pub const SYS_CONNECT: usize = 203;
    pub const SYS_ACCEPT: usize = 202;
    pub const SYS_SENDTO: usize = 206;
    pub const SYS_RECVFROM: usize = 207;
    pub const SYS_SENDMSG: usize = 211;
    pub const SYS_RECVMSG: usize = 212;
    pub const SYS_SHUTDOWN: usize = 205;
    pub const SYS_BIND: usize = 200;
    pub const SYS_LISTEN: usize = 201;
    pub const SYS_GETSOCKNAME: usize = 204;
    pub const SYS_GETPEERNAME: usize = 208;
    pub const SYS_SOCKETPAIR: usize = 199;
    pub const SYS_SETSOCKOPT: usize = 209;
    pub const SYS_GETSOCKOPT: usize = 210;
    pub const SYS_CLONE: usize = 220;
    pub const SYS_FORK: usize = 1739;
    pub const SYS_VFORK: usize = 190;
    pub const SYS_EXECVE: usize = 221;
    pub const SYS_EXIT: usize = 93;
    pub const SYS_WAIT4: usize = 61;
    pub const SYS_KILL: usize = 129;
    pub const SYS_UNAME: usize = 160;

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

/// RISC-V interrupt frame
///
/// Captures the processor state at exception time
#[repr(C)]
#[derive(Debug)]
pub struct RiscvIframe {
    /// General-purpose registers
    pub ra: u64,   // x1 (return address)
    pub sp: u64,   // x2 (stack pointer)
    pub gp: u64,   // x3 (global pointer)
    pub tp: u64,   // x4 (thread pointer)
    pub t0: u64,   // x5
    pub t1: u64,   // x6
    pub t2: u64,   // x7
    pub s0: u64,   // x8 / fp
    pub s1: u64,   // x9
    pub a0: u64,   // x10 (argument/return value)
    pub a1: u64,   // x11
    pub a2: u64,   // x12
    pub a3: u64,   // x13
    pub a4: u64,   // x14
    pub a5: u64,   // x15
    pub a6: u64,   // x16
    pub a7: u64,   // x17
    pub s2: u64,   // x18
    pub s3: u64,   // x19
    pub s4: u64,   // x20
    pub s5: u64,   // x21
    pub s6: u64,   // x22
    pub s7: u64,   // x23
    pub s8: u64,   // x24
    pub s9: u64,   // x25
    pub s10: u64,  // x26
    pub s11: u64,  // x27
    pub t3: u64,   // x28
    pub t4: u64,   // x29
    pub t5: u64,   // x30
    pub t6: u64,   // x31
    /// Exception-specific registers
    pub pc: u64,   // Program counter (saved from EPC)
    pub status: u64, // Status register (saved from SSTATUS)
    pub cause: u64, // Exception cause (SCAUSE)
    pub tval: u64,  // Trap value (STVAL)
}

/// Exception dispatch context
#[repr(C)]
pub struct ExceptionContext {
    pub iframe: *mut RiscvIframe,
    pub cause: u64,
    pub tval: u64,
}

/// Exception type for user-space dispatch
#[repr(u32)]
pub enum ExceptionType {
    Unknown = 0,
    IllegalInstruction = 1,
    Breakpoint = 2,
    PageFault = 3,
    UserCopy = 4,
    General = 8,
}

/// Dump the exception frame for debugging
fn dump_iframe(iframe: &RiscvIframe) {
    println!("RISC-V Exception Frame:");
    println!("  PC     = {:#18x}", iframe.pc);
    println!("  SSTATUS = {:#18x}", iframe.status);
    println!("  SCAUSE  = {:#18x}", iframe.cause);
    println!("  STVAL   = {:#18x}", iframe.tval);
    println!();
    println!("  RA  = {:#18x}  SP  = {:#18x}  GP  = {:#18x}", iframe.ra, iframe.sp, iframe.gp);
    println!("  TP  = {:#18x}  T0  = {:#18x}  T1  = {:#18x}", iframe.tp, iframe.t0, iframe.t1);
    println!("  T2  = {:#18x}  S0  = {:#18x}  S1  = {:#18x}", iframe.t2, iframe.s0, iframe.s1);
    println!("  A0  = {:#18x}  A1  = {:#18x}  A2  = {:#18x}", iframe.a0, iframe.a1, iframe.a2);
    println!("  A3  = {:#18x}  A4  = {:#18x}  A5  = {:#18x}", iframe.a3, iframe.a4, iframe.a5);
    println!("  A6  = {:#18x}  A7  = {:#18x}", iframe.a6, iframe.a7);
    println!("  S2  = {:#18x}  S3  = {:#18x}  S4  = {:#18x}", iframe.s2, iframe.s3, iframe.s4);
    println!("  S5  = {:#18x}  S6  = {:#18x}  S7  = {:#18x}", iframe.s5, iframe.s6, iframe.s7);
    println!("  S8  = {:#18x}  S9  = {:#18x}  S10 = {:#18x}", iframe.s8, iframe.s9, iframe.s10);
    println!("  S11 = {:#18x}  T3  = {:#18x}  T4  = {:#18x}", iframe.s11, iframe.t3, iframe.t4);
    println!("  T5  = {:#18x}  T6  = {:#18x}", iframe.t5, iframe.t6);
}

/// Check if exception came from user mode
fn is_from_user(iframe: &RiscvIframe) -> bool {
    // SPP bit in SSTATUS indicates previous privilege mode
    // 0 = user mode, 1 = supervisor mode
    (iframe.status & (1 << 8)) == 0
}

/// Fatal exception handler - prints diagnostic and halts
#[cold]
fn exception_die(iframe: &RiscvIframe, msg: &str) -> ! {
    println!("{}", msg);
    println!("Exception cause: {:#x}", iframe.cause);
    if iframe.tval != 0 {
        println!("Trap value: {:#x}", iframe.tval);
    }
    dump_iframe(iframe);

    // TODO: platform_halt(HALT_ACTION_HALT, HALT_REASON_SW_PANIC);

    loop {
        unsafe { core::arch::asm!("wfi") };
    }
}

/// Page fault handler
fn riscv_page_fault_handler(iframe: &mut RiscvIframe) {
    let is_load = match iframe.cause {
        scause::LOAD_PAGE_FAULT => true,
        scause::STORE_AMO_PAGE_FAULT => false,
        scause::INSTRUCTION_PAGE_FAULT => {
            // Instruction page fault
            exception_die(iframe, "Instruction page fault\n");
        }
        _ => {
            exception_die(iframe, "Invalid page fault cause\n");
        }
    };

    let fault_addr = iframe.tval as VAddr;

    println!(
        "Page fault: {} address {:#x} at PC {:#x}",
        if is_load { "load" } else { "store" },
        fault_addr,
        iframe.pc
    );

    // Check if user mode
    if is_from_user(iframe) {
        // TODO: Dispatch user exception
        // For now, fatal error
        exception_die(iframe, "User page fault (unimplemented)\n");
    } else {
        // Kernel page fault - always fatal
        exception_die(iframe, "Kernel page fault\n");
    }
}

/// Illegal instruction handler
fn riscv_illegal_instruction_handler(iframe: &mut RiscvIframe) {
    if is_from_user(iframe) {
        // TODO: Try to dispatch to user-space exception handler
        exception_die(iframe, "User illegal instruction (unimplemented)\n");
    } else {
        exception_die(iframe, "Kernel illegal instruction\n");
    }
}

/// Breakpoint handler
fn riscv_breakpoint_handler(iframe: &mut RiscvIframe) {
    if is_from_user(iframe) {
        // TODO: Try to dispatch to user-space exception handler
        exception_die(iframe, "User breakpoint (unimplemented)\n");
    } else {
        exception_die(iframe, "Kernel breakpoint\n");
    }
}

/// Supervisor software interrupt (IPI from other harts)
fn riscv_software_interrupt_handler(_iframe: &mut RiscvIframe) {
    // TODO: Handle inter-processor interrupt
    // Typically used for TLB shootdowns, rescheduling, etc.
    println!("Software interrupt received");
}

/// Supervisor timer interrupt
fn riscv_timer_interrupt_handler(_iframe: &mut RiscvIframe) {
    // TODO: Handle timer interrupt
    // Typically used for preemption, timeout, etc.
    println!("Timer interrupt received");
}

/// Supervisor external interrupt (PLIC)
fn riscv_external_interrupt_handler(_iframe: &mut RiscvIframe) {
    // TODO: Read from PLIC to determine which device interrupted
    println!("External interrupt received");
}

/// Dispatch a system call to its handler
///
/// # Arguments
///
/// * `nr` - System call number
/// * `a0`-`a5` - System call arguments
///
/// # Returns
///
/// The system call return value (negative for errors)
fn dispatch_syscall(
    nr: usize,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    _a4: u64,
    _a5: u64,
) -> i64 {
    match nr {
        // Basic I/O
        syscall_nr::SYS_READ => sys_read(a0, a1, a2),
        syscall_nr::SYS_WRITE => sys_write(a0, a1, a2),

        // Process management
        syscall_nr::SYS_EXIT => sys_exit(a0 as i32),
        syscall_nr::SYS_GETPID => sys_getpid(),

        // Thread management (Rustux-specific)
        syscall_nr::SYS_RUSTUX_THREAD_YIELD => sys_thread_yield(),

        // Debug output (Rustux-specific)
        syscall_nr::SYS_RUSTUX_DEBUG_PRINT => sys_debug_print(a0, a1),

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
fn sys_write(fd: u64, _buf: u64, count: u64) -> i64 {
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
        unsafe { core::arch::asm!("wfi") };
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

/// Environment call from user mode (syscall)
fn riscv_syscall_handler(iframe: &mut RiscvIframe) {
    // Syscall number is in a7
    // Arguments are in a0-a6
    // Return value goes in a0

    let syscall_num = iframe.a7 as usize;
    let arg0 = iframe.a0;
    let arg1 = iframe.a1;
    let arg2 = iframe.a2;
    let arg3 = iframe.a3;
    let arg4 = iframe.a4;
    let arg5 = iframe.a5;

    // Dispatch the system call
    let result = dispatch_syscall(syscall_num, arg0, arg1, arg2, arg3, arg4, arg5);

    // Return value goes in a0
    iframe.a0 = result as u64;
}

/// Main exception dispatch handler
///
/// Called from assembly exception entry point in exceptions.S
///
/// # Arguments
///
/// * `iframe` - Pointer to the interrupt frame
///
/// # Safety
///
/// iframe must point to valid memory
#[no_mangle]
pub unsafe extern "C" fn riscv_exception_handler(iframe: *mut RiscvIframe) {
    let iframe = &mut *iframe;

    // Get the exception cause
    let cause = iframe.cause;

    // Check if it's an interrupt (high bit set)
    if cause & scause::INTERRUPT_BIT != 0 {
        // It's an interrupt
        let interrupt_code = cause & !scause::INTERRUPT_BIT;

        match interrupt_code {
            scause::SUPERVISOR_SOFTWARE_INTERRUPT => {
                riscv_software_interrupt_handler(iframe);
            }
            scause::SUPERVISOR_TIMER_INTERRUPT => {
                riscv_timer_interrupt_handler(iframe);
            }
            scause::SUPERVISOR_EXTERNAL_INTERRUPT => {
                riscv_external_interrupt_handler(iframe);
            }
            _ => {
                println!("Unknown interrupt: {:#x}", interrupt_code);
            }
        }
    } else {
        // It's an exception
        match cause {
            scause::INSTRUCTION_ADDRESS_MISALIGNED => {
                exception_die(iframe, "Instruction address misaligned\n");
            }
            scause::INSTRUCTION_ACCESS_FAULT => {
                exception_die(iframe, "Instruction access fault\n");
            }
            scause::ILLEGAL_INSTRUCTION => {
                riscv_illegal_instruction_handler(iframe);
            }
            scause::BREAKPOINT => {
                riscv_breakpoint_handler(iframe);
            }
            scause::LOAD_ADDRESS_MISALIGNED => {
                exception_die(iframe, "Load address misaligned\n");
            }
            scause::LOAD_ACCESS_FAULT => {
                exception_die(iframe, "Load access fault\n");
            }
            scause::STORE_AMO_ADDRESS_MISALIGNED => {
                exception_die(iframe, "Store/AMO address misaligned\n");
            }
            scause::STORE_AMO_ACCESS_FAULT => {
                exception_die(iframe, "Store/AMO access fault\n");
            }
            scause::ENV_CALL_FROM_U_MODE => {
                riscv_syscall_handler(iframe);
            }
            scause::ENV_CALL_FROM_S_MODE => {
                exception_die(iframe, "Environment call from S-mode\n");
            }
            scause::INSTRUCTION_PAGE_FAULT => {
                riscv_page_fault_handler(iframe);
            }
            scause::LOAD_PAGE_FAULT => {
                riscv_page_fault_handler(iframe);
            }
            scause::STORE_AMO_PAGE_FAULT => {
                riscv_page_fault_handler(iframe);
            }
            _ => {
                println!("Unknown exception: {:#x}", cause);
                exception_die(iframe, "Unknown exception\n");
            }
        }
    }
}

/// Get the SSTATUS register value
#[inline]
pub fn riscv_read_sstatus() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::SSTATUS) }
}

/// Get the SCAUSE register value
#[inline]
pub fn riscv_read_scause() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::SCAUSE) }
}

/// Get the STVAL register value
#[inline]
pub fn riscv_read_stval() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::STVAL) }
}

/// Get the SEPC register value
#[inline]
pub fn riscv_read_sepc() -> u64 {
    unsafe { crate::arch::riscv64::registers::read_csr(csr::SEPC) }
}

/// Write to the SEPC register (to set where to return after exception)
#[inline]
pub fn riscv_write_sepc(value: u64) {
    unsafe { crate::arch::riscv64::registers::write_csr(csr::SEPC, value) }
}

/// End of interrupt - acknowledge interrupt handling
#[inline]
pub fn riscv_end_of_interrupt() {
    // For RISC-V, there's no explicit EOI like x86's APIC EOI
    // For PLIC (Platform-Level Interrupt Controller), we need
    // to claim and complete the interrupt
    // TODO: Implement PLIC handling
}

// External functions for exception dispatch
extern "C" {
    /// Dispatch exception to user-space handler
    fn dispatch_user_exception(
        kind: u32,
        context: *mut ExceptionContext,
    ) -> i32;
}
