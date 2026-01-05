// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Raw syscall interface
//!
//! This module provides the low-level syscall interface for
//! making system calls from userspace to the kernel.

#![no_std]

/// System call numbers
///
/// These must match the kernel's syscall definitions.
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SyscallNumber {
    // Process/thread syscalls
    ProcessCreate = 0x01,
    ProcessStart = 0x02,
    ProcessExit = 0x03,
    ThreadCreate = 0x04,
    ThreadStart = 0x05,
    ThreadExit = 0x06,
    ThreadReadState = 0x07,
    ThreadWriteState = 0x08,
    ThreadResume = 0x09,

    // Virtual memory syscalls
    VmoCreate = 0x10,
    VmoRead = 0x11,
    VmoWrite = 0x12,
    VmoGetSize = 0x13,
    VmoSetSize = 0x14,
    VmoOpRange = 0x15,
    VmarMap = 0x16,
    VmarUnmap = 0x17,
    VmarProtect = 0x18,
    VmarDestroy = 0x19,

    // Handle syscalls
    HandleClose = 0x20,
    HandleDuplicate = 0x21,
    HandleReplace = 0x22,
    HandleCloseMany = 0x23,

    // Object operations
    ObjectSignal = 0x30,
    ObjectWaitOne = 0x31,
    ObjectWaitMany = 0x32,
    ObjectGetProperty = 0x33,
    ObjectSetProperty = 0x34,
    ObjectSignalPeer = 0x35,
    ObjectGetInfo = 0x36,

    // Channel IPC
    ChannelCreate = 0x40,
    ChannelRead = 0x41,
    ChannelWrite = 0x42,
    ChannelCallEtc = 0x43,

    // Event/EventPair
    EventCreate = 0x50,
    EventPairCreate = 0x51,

    // Port/Waitset
    PortCreate = 0x60,
    PortQueue = 0x61,
    PortWait = 0x62,
    PortCancel = 0x63,

    // Futex
    FutexWait = 0x70,
    FutexWake = 0x71,
    FutexRequeue = 0x72,

    // Policy
    PolicyGetProfile = 0x80,
    PolicySetProfile = 0x81,

    // Hypervisor
    HypervisorCreate = 0x90,
    HypervisorOp = 0x91,

    // Misc
    SystemGetVersion = 0xA0,
    SystemGetPhysMem = 0xA1,
    SystemPowerctl = 0xA2,

    // Bootstrap
    ProcArgs = 0xB0,
    VmarRootSelf = 0xB1,
    JobDefault = 0xB2,
    ThreadSelf = 0xB3,

    // Socket
    SocketCreate = 0xC0,
    SocketWrite = 0xC1,
    SocketRead = 0xC2,
    SocketShutdown = 0xC3,
}

/// Make a syscall with no arguments
#[inline]
pub unsafe fn syscall0(n: u64) -> u64 {
    let ret: u64;
    match () {
        #[cfg(target_arch = "x86_64")]
        () => core::arch::asm!(
            "syscall",
            inlateout("rax") n => ret,
            lateout("rcx") _,
            lateout("r11") _,
        ),

        #[cfg(target_arch = "aarch64")]
        () => core::arch::asm!(
            "svc #0",
            inlateout("x8") n as u64 => ret,
            lateout("x0") _,
            lateout("x1") _,
            lateout("x2") _,
            lateout("x3") _,
            lateout("x4") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x16") _,
            lateout("x17") _,
        ),

        #[cfg(target_arch = "riscv64")]
        () => core::arch::asm!(
            "ecall",
            inlateout("x17") n as u64 => ret,
            lateout("x10") _,
            lateout("x11") _,
            lateout("x12") _,
            lateout("x13") _,
            lateout("x14") _,
            lateout("x15") _,
            lateout("x16") _,
            lateout("x1") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x28") _,
            lateout("x29") _,
            lateout("x30") _,
            lateout("x31") _,
        ),

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        () => unimplemented!("syscall0 not implemented for this architecture"),
    }
    ret
}

/// Make a syscall with one argument
#[inline]
pub unsafe fn syscall1(n: u64, a1: u64) -> u64 {
    let ret: u64;
    match () {
        #[cfg(target_arch = "x86_64")]
        () => core::arch::asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            lateout("rcx") _,
            lateout("r11") _,
        ),

        #[cfg(target_arch = "aarch64")]
        () => core::arch::asm!(
            "svc #0",
            inlateout("x8") n as u64 => ret,
            inlateout("x0") a1,
            lateout("x1") _,
            lateout("x2") _,
            lateout("x3") _,
            lateout("x4") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x16") _,
            lateout("x17") _,
        ),

        #[cfg(target_arch = "riscv64")]
        () => core::arch::asm!(
            "ecall",
            inlateout("x17") n as u64 => ret,
            inlateout("x10") a1,
            lateout("x11") _,
            lateout("x12") _,
            lateout("x13") _,
            lateout("x14") _,
            lateout("x15") _,
            lateout("x16") _,
            lateout("x1") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x28") _,
            lateout("x29") _,
            lateout("x30") _,
            lateout("x31") _,
        ),

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        () => unimplemented!("syscall1 not implemented for this architecture"),
    }
    ret
}

/// Make a syscall with two arguments
#[inline]
pub unsafe fn syscall2(n: u64, a1: u64, a2: u64) -> u64 {
    let ret: u64;
    match () {
        #[cfg(target_arch = "x86_64")]
        () => core::arch::asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            lateout("rcx") _,
            lateout("r11") _,
        ),

        #[cfg(target_arch = "aarch64")]
        () => core::arch::asm!(
            "svc #0",
            inlateout("x8") n as u64 => ret,
            inlateout("x0") a1,
            inlateout("x1") a2,
            lateout("x2") _,
            lateout("x3") _,
            lateout("x4") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x16") _,
            lateout("x17") _,
        ),

        #[cfg(target_arch = "riscv64")]
        () => core::arch::asm!(
            "ecall",
            inlateout("x17") n as u64 => ret,
            inlateout("x10") a1,
            inlateout("x11") a2,
            lateout("x12") _,
            lateout("x13") _,
            lateout("x14") _,
            lateout("x15") _,
            lateout("x16") _,
            lateout("x1") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x28") _,
            lateout("x29") _,
            lateout("x30") _,
            lateout("x31") _,
        ),

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        () => unimplemented!("syscall2 not implemented for this architecture"),
    }
    ret
}

/// Make a syscall with three arguments
#[inline]
pub unsafe fn syscall3(n: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    let ret: u64;
    match () {
        #[cfg(target_arch = "x86_64")]
        () => core::arch::asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            lateout("rcx") _,
            lateout("r11") _,
        ),

        #[cfg(target_arch = "aarch64")]
        () => core::arch::asm!(
            "svc #0",
            inlateout("x8") n as u64 => ret,
            inlateout("x0") a1,
            inlateout("x1") a2,
            inlateout("x2") a3,
            lateout("x3") _,
            lateout("x4") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x16") _,
            lateout("x17") _,
        ),

        #[cfg(target_arch = "riscv64")]
        () => core::arch::asm!(
            "ecall",
            inlateout("x17") n as u64 => ret,
            inlateout("x10") a1,
            inlateout("x11") a2,
            inlateout("x12") a3,
            lateout("x13") _,
            lateout("x14") _,
            lateout("x15") _,
            lateout("x16") _,
            lateout("x1") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x28") _,
            lateout("x29") _,
            lateout("x30") _,
            lateout("x31") _,
        ),

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        () => unimplemented!("syscall3 not implemented for this architecture"),
    }
    ret
}

/// Make a syscall with four arguments
#[inline]
pub unsafe fn syscall4(n: u64, a1: u64, a2: u64, a3: u64, a4: u64) -> u64 {
    let ret: u64;
    match () {
        #[cfg(target_arch = "x86_64")]
        () => core::arch::asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            lateout("rcx") _,
            lateout("r11") _,
        ),

        #[cfg(target_arch = "aarch64")]
        () => core::arch::asm!(
            "svc #0",
            inlateout("x8") n as u64 => ret,
            inlateout("x0") a1,
            inlateout("x1") a2,
            inlateout("x2") a3,
            inlateout("x3") a4,
            lateout("x4") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x16") _,
            lateout("x17") _,
        ),

        #[cfg(target_arch = "riscv64")]
        () => core::arch::asm!(
            "ecall",
            inlateout("x17") n as u64 => ret,
            inlateout("x10") a1,
            inlateout("x11") a2,
            inlateout("x12") a3,
            inlateout("x13") a4,
            lateout("x14") _,
            lateout("x15") _,
            lateout("x16") _,
            lateout("x1") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x28") _,
            lateout("x29") _,
            lateout("x30") _,
            lateout("x31") _,
        ),

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        () => unimplemented!("syscall4 not implemented for this architecture"),
    }
    ret
}

/// Make a syscall with five arguments
#[inline]
pub unsafe fn syscall5(n: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    let ret: u64;
    match () {
        #[cfg(target_arch = "x86_64")]
        () => core::arch::asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            lateout("rcx") _,
            lateout("r11") _,
        ),

        #[cfg(target_arch = "aarch64")]
        () => core::arch::asm!(
            "svc #0",
            inlateout("x8") n as u64 => ret,
            inlateout("x0") a1,
            inlateout("x1") a2,
            inlateout("x2") a3,
            inlateout("x3") a4,
            inlateout("x4") a5,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x16") _,
            lateout("x17") _,
        ),

        #[cfg(target_arch = "riscv64")]
        () => core::arch::asm!(
            "ecall",
            inlateout("x17") n as u64 => ret,
            inlateout("x10") a1,
            inlateout("x11") a2,
            inlateout("x12") a3,
            inlateout("x13") a4,
            inlateout("x14") a5,
            lateout("x15") _,
            lateout("x16") _,
            lateout("x1") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x28") _,
            lateout("x29") _,
            lateout("x30") _,
            lateout("x31") _,
        ),

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        () => unimplemented!("syscall5 not implemented for this architecture"),
    }
    ret
}

/// Make a syscall with six arguments
#[inline]
pub unsafe fn syscall6(n: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> u64 {
    let ret: u64;
    match () {
        #[cfg(target_arch = "x86_64")]
        () => core::arch::asm!(
            "syscall",
            inlateout("rax") n => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            in("r10") a4,
            in("r8") a5,
            in("r9") a6,
            lateout("rcx") _,
            lateout("r11") _,
        ),

        #[cfg(target_arch = "aarch64")]
        () => core::arch::asm!(
            "svc #0",
            inlateout("x8") n as u64 => ret,
            inlateout("x0") a1,
            inlateout("x1") a2,
            inlateout("x2") a3,
            inlateout("x3") a4,
            inlateout("x4") a5,
            inlateout("x5") a6,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x16") _,
            lateout("x17") _,
        ),

        #[cfg(target_arch = "riscv64")]
        () => core::arch::asm!(
            "ecall",
            inlateout("x17") n as u64 => ret,
            inlateout("x10") a1,
            inlateout("x11") a2,
            inlateout("x12") a3,
            inlateout("x13") a4,
            inlateout("x14") a5,
            inlateout("x15") a6,
            lateout("x16") _,
            lateout("x1") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x28") _,
            lateout("x29") _,
            lateout("x30") _,
            lateout("x31") _,
        ),

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "riscv64")))]
        () => unimplemented!("syscall6 not implemented for this architecture"),
    }
    ret
}
