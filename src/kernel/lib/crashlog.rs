// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Crash Log
//!
//! This module provides crash logging functionality for kernel panics.
//! It formats panic information including uptime, version, registers, and backtrace.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::rustux::types::*;

/// Maximum crash log size
const MAX_CRASHLOG_SIZE: usize = 4096;

/// Architecture string
#[cfg(target_arch = "x86_64")]
const ARCH_NAME: &str = "x86_64";

#[cfg(target_arch = "aarch64")]
const ARCH_NAME: &str = "aarch64";

#[cfg(target_arch = "riscv64")]
const ARCH_NAME: &str = "riscv64";

/// Interrupt frame for x86_64
#[cfg(target_arch = "x86_64")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InterruptFrame {
    pub cs: u64,
    pub ip: u64,
    pub flags: u64,
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub user_sp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub err_code: u64,
}

/// Interrupt frame for aarch64
#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InterruptFrame {
    pub r: [u64; 30],
    pub lr: u64,
    pub usp: u64,
    pub elr: u64,
    pub spsr: u64,
}

/// Crash log data
pub struct CrashLog {
    /// Interrupt frame (if available)
    pub iframe: Option<&'static InterruptFrame>,
    /// Kernel base address
    pub base_address: u64,
}

/// Global crash log instance
static CRASHLOG: Mutex<CrashLog> = Mutex::new(CrashLog {
    iframe: None,
    base_address: 0,
});

/// Current time in nanoseconds (stub)
fn current_time() -> i64 {
    // TODO: Implement proper time tracking
    0
}

/// Format crash log to string
///
/// # Arguments
///
/// * `out` - Output buffer
///
/// # Returns
///
/// Number of bytes written
pub fn crashlog_to_string(out: &mut [u8]) -> usize {
    let log = CRASHLOG.lock();
    let mut pos = 0;

    // Header
    let header = "RUSTUX KERNEL PANIC\n\n";
    if pos + header.len() <= out.len() {
        out[pos..pos + header.len()].copy_from_slice(header.as_bytes());
        pos += header.len();
    } else {
        return out.len();
    }

    // Uptime
    let uptime_ms = current_time() / 1_000_000;
    let uptime_str = format!("UPTIME (ms)\n{}\n\n", uptime_ms);
    if pos + uptime_str.len() <= out.len() {
        out[pos..pos + uptime_str.len()].copy_from_slice(uptime_str.as_bytes());
        pos += uptime_str.len();
    } else {
        return out.len();
    }

    // Version info
    let version = format!(
        "VERSION\narch: {}\nbuild_id: {}\ndso: id={} base={:#x} name=kernel.elf\n\n",
        ARCH_NAME,
        env!("CARGO_PKG_VERSION"),
        "buildid_placeholder", // TODO: Get actual build ID
        log.base_address
    );
    if pos + version.len() <= out.len() {
        out[pos..pos + version.len()].copy_from_slice(version.as_bytes());
        pos += version.len();
    } else {
        return out.len();
    }

    // Registers
    if let Some(iframe) = log.iframe {
        let regs = format_registers(iframe);
        if pos + regs.len() <= out.len() {
            out[pos..pos + regs.len()].copy_from_slice(regs.as_bytes());
            pos += regs.len();
        } else {
            return out.len();
        }
    }

    // Backtrace header
    let bt_header = "BACKTRACE (up to 16 calls)\n";
    if pos + bt_header.len() <= out.len() {
        out[pos..pos + bt_header.len()].copy_from_slice(bt_header.as_bytes());
        pos += bt_header.len();
    } else {
        return out.len();
    }

    // Backtrace (placeholder)
    let bt = format_backtrace();
    if pos + bt.len() <= out.len() {
        out[pos..pos + bt.len()].copy_from_slice(bt.as_bytes());
        pos += bt.len();
    } else {
        return out.len();
    }

    // Footer
    let footer = "\n";
    if pos + footer.len() <= out.len() {
        out[pos..pos + footer.len()].copy_from_slice(footer.as_bytes());
        pos += footer.len();
    }

    pos
}

/// Format registers for x86_64
#[cfg(target_arch = "x86_64")]
fn format_registers(frame: &InterruptFrame) -> String {
    format!(
        "REGISTERS\n\
         CS: {:#18x}\n\
         RIP: {:#18x}\n\
         EFL: {:#18x}\n\
         CR2: {:#18x}\n\
         RAX: {:#18x}\n\
         RBX: {:#18x}\n\
         RCX: {:#18x}\n\
         RDX: {:#18x}\n\
         RSI: {:#18x}\n\
         RDI: {:#18x}\n\
         RBP: {:#18x}\n\
         RSP: {:#18x}\n\
          R8: {:#18x}\n\
          R9: {:#18x}\n\
         R10: {:#18x}\n\
         R11: {:#18x}\n\
         R12: {:#18x}\n\
         R13: {:#18x}\n\
         R14: {:#18x}\n\
         R15: {:#18x}\n\
        errc: {:#18x}\n\
        \n",
        frame.cs,
        frame.ip,
        frame.flags,
        x86_get_cr2(),
        frame.rax,
        frame.rbx,
        frame.rcx,
        frame.rdx,
        frame.rsi,
        frame.rdi,
        frame.rbp,
        frame.user_sp,
        frame.r8,
        frame.r9,
        frame.r10,
        frame.r11,
        frame.r12,
        frame.r13,
        frame.r14,
        frame.r15,
        frame.err_code
    )
}

/// Format registers for aarch64
#[cfg(target_arch = "aarch64")]
fn format_registers(frame: &InterruptFrame) -> String {
    format!(
        "REGISTERS\n\
          x0: {:#18x}\n\
          x1: {:#18x}\n\
          x2: {:#18x}\n\
          x3: {:#18x}\n\
          x4: {:#18x}\n\
          x5: {:#18x}\n\
          x6: {:#18x}\n\
          x7: {:#18x}\n\
          x8: {:#18x}\n\
          x9: {:#18x}\n\
         x10: {:#18x}\n\
         x11: {:#18x}\n\
         x12: {:#18x}\n\
         x13: {:#18x}\n\
         x14: {:#18x}\n\
         x15: {:#18x}\n\
         x16: {:#18x}\n\
         x17: {:#18x}\n\
         x18: {:#18x}\n\
         x19: {:#18x}\n\
         x20: {:#18x}\n\
         x21: {:#18x}\n\
         x22: {:#18x}\n\
         x23: {:#18x}\n\
         x24: {:#18x}\n\
         x25: {:#18x}\n\
         x26: {:#18x}\n\
         x27: {:#18x}\n\
         x28: {:#18x}\n\
         x29: {:#18x}\n\
          lr: {:#18x}\n\
         usp: {:#18x}\n\
         elr: {:#18x}\n\
        spsr: {:#18x}\n\
        \n",
        frame.r[0], frame.r[1], frame.r[2], frame.r[3], frame.r[4], frame.r[5],
        frame.r[6], frame.r[7], frame.r[8], frame.r[9], frame.r[10], frame.r[11],
        frame.r[12], frame.r[13], frame.r[14], frame.r[15], frame.r[16], frame.r[17],
        frame.r[18], frame.r[19], frame.r[20], frame.r[21], frame.r[22], frame.r[23],
        frame.r[24], frame.r[25], frame.r[26], frame.r[27], frame.r[28], frame.r[29],
        frame.lr, frame.usp, frame.elr, frame.spsr
    )
}

/// Stub for non-x86 architectures
#[cfg(not(target_arch = "x86_64"))]
fn x86_get_cr2() -> u64 {
    0
}

/// Format backtrace (placeholder)
fn format_backtrace() -> String {
    // TODO: Implement proper backtrace capture
    String::from("<backtrace not yet implemented>\n")
}

/// Set the interrupt frame for crash logging
pub fn crashlog_set_iframe(iframe: &'static InterruptFrame) {
    let mut log = CRASHLOG.lock();
    log.iframe = Some(iframe);
}

/// Set the kernel base address
pub fn crashlog_set_base_address(addr: u64) {
    let mut log = CRASHLOG.lock();
    log.base_address = addr;
}

/// Print crash log to console
pub fn crashlog_print() {
    let mut buffer = [0u8; MAX_CRASHLOG_SIZE];
    let len = crashlog_to_string(&mut buffer);
    let s = core::str::from_utf8(&buffer[..len]).unwrap_or("<invalid utf8>");
    println!("{}", s);
}

/// Initialize crash log system
pub fn init() {
    println!("CrashLog: initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crashlog_constants() {
        assert_eq!(MAX_CRASHLOG_SIZE, 4096);
        assert!(!ARCH_NAME.is_empty());
    }

    #[test]
    fn test_crashlog_format() {
        let mut buffer = [0u8; 1024];
        let len = crashlog_to_string(&mut buffer);
        assert!(len > 0);
        assert!(len < buffer.len());

        let s = core::str::from_utf8(&buffer[..len]).unwrap();
        assert!(s.contains("RUSTUX KERNEL PANIC"));
        assert!(s.contains("UPTIME"));
        assert!(s.contains("VERSION"));
        assert!(s.contains("BACKTRACE"));
    }

    #[test]
    fn test_base_address() {
        crashlog_set_base_address(0xffffffff80100000);
        let log = CRASHLOG.lock();
        assert_eq!(log.base_address, 0xffffffff80100000);
    }
}
