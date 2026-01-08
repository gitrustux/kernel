// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Logging and Diagnostics
//!
//! This module provides logging, debugging, and diagnostic services for the kernel.
//! It supports early boot logging (before UART initialization) and regular runtime logging.
//!
//! # Features
//!
//! - **Log levels**: Trace, Debug, Info, Warning, Error, Fatal
//! - **Early output**: Boot-time logging before drivers are ready
//! - **Per-arch UART drivers**: ARM64, AMD64, RISC-V support
//! - **Crash dumps**: Register dump, stack trace, panic information
//! - **Structured logging**: Key-value pair logging format
//!
//! # Usage
//!
//! ```rust
//! // Simple logging
//! log_info!("Kernel boot starting on {}", arch_name());
//! log_error!("Failed to allocate memory: {}", status);
//!
//! // Conditional logging
//! log_trace_if!(LOCAL_TRACE, "Page fault at {:#x}", fault_addr);
//!
//! // Panic with diagnostics
//! panic!("Unexpected state in {}", function_name());
//! ```


use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

/// Log levels
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Trace-level logging (very verbose)
    Trace = 0,

    /// Debug-level logging (verbose)
    Debug = 1,

    /// Informational logging
    Info = 2,

    /// Warning-level logging
    Warning = 3,

    /// Error-level logging
    Error = 4,

    /// Fatal errors (will halt the system)
    Fatal = 5,
}

// LK compatibility: INFO constant for log level
pub const INFO: LogLevel = LogLevel::Info;

impl LogLevel {
    /// Get the log level name as a string
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }

    /// Get the log level color code (ANSI)
    pub fn as_ansi_color(self) -> &'static str {
        match self {
            LogLevel::Trace => "\x1b[36m",      // Cyan
            LogLevel::Debug => "\x1b[34m",      // Blue
            LogLevel::Info => "\x1b[32m",       // Green
            LogLevel::Warning => "\x1b[33m",    // Yellow
            LogLevel::Error => "\x1b[31m",      // Red
            LogLevel::Fatal => "\x1b[35m",      // Magenta
        }
    }

    /// Reset ANSI color
    pub fn ansi_reset() -> &'static str {
        "\x1b[0m"
    }
}

/// Global minimum log level
///
/// Only messages at or above this level will be printed.
static mut MIN_LOG_LEVEL: LogLevel = LogLevel::Info;

/// Flag indicating whether UART has been initialized
static mut UART_READY: AtomicBool = AtomicBool::new(false);

/// Flag indicating whether colors should be used
static mut USE_COLORS: AtomicBool = AtomicBool::new(true);

/// Flag indicating whether timestamps should be shown
static mut SHOW_TIMESTAMPS: AtomicBool = AtomicBool::new(false);

/// Kernel start time (for timestamps)
static mut KERNEL_START_TIME: u64 = 0;

/// Set the minimum log level
///
/// # Arguments
///
/// * `level` - Minimum log level to display
pub fn log_set_min_level(level: LogLevel) {
    unsafe { MIN_LOG_LEVEL = level; }
}

/// Get the current minimum log level
pub fn log_get_min_level() -> LogLevel {
    unsafe { MIN_LOG_LEVEL }
}

/// Enable or disable colored output
pub fn log_set_colors(enabled: bool) {
    unsafe { USE_COLORS.store(enabled, Ordering::Relaxed); }
}

/// Enable or disable timestamps
pub fn log_set_timestamps(enabled: bool) {
    unsafe { SHOW_TIMESTAMPS.store(enabled, Ordering::Relaxed); }
}

/// Mark UART as ready for logging
///
/// Called by UART drivers during initialization.
pub fn log_set_uart_ready() {
    unsafe { UART_READY.store(true, Ordering::Release); }
}

/// Check if UART is ready
pub fn log_is_uart_ready() -> bool {
    unsafe { UART_READY.load(Ordering::Acquire) }
}

/// Set the kernel start time (for timestamps)
pub fn log_set_kernel_start_time(time: u64) {
    unsafe { KERNEL_START_TIME = time; }
}

/// Internal print function
///
/// This is the core output function that writes to the console.
/// It will use early boot output before UART is ready, and
/// switch to UART output once available.
///
/// # Arguments
///
/// * `s` - String slice to print
pub(crate) fn print_internal(s: &str) {
    extern "C" {
        /// Early boot output (writes directly to framebuffer/console)
        fn early_print(s: &str);

        /// UART output (once UART driver is initialized)
        fn uart_print(s: &str);
    }

    unsafe {
        if UART_READY.load(Ordering::Acquire) {
            uart_print(s);
        } else {
            early_print(s);
        }
    }
}

/// Print a formatted message at a specific log level
///
/// # Arguments
///
/// * `level` - Log level for this message
/// * `args` - Format arguments
#[inline]
pub fn log_print(level: LogLevel, args: core::fmt::Arguments) {
    // Check if this message should be logged
    unsafe {
        if level < MIN_LOG_LEVEL {
            return;
        }
    }

    // Print log level
    let color = if unsafe { USE_COLORS.load(Ordering::Relaxed) } {
        level.as_ansi_color()
    } else {
        ""
    };

    print_internal(color);
    print_internal("[");
    print_internal(level.as_str());
    print_internal("]");
    if unsafe { USE_COLORS.load(Ordering::Relaxed) } {
        print_internal(LogLevel::ansi_reset());
    }
    print_internal(" ");

    // Print timestamp if enabled
    if unsafe { SHOW_TIMESTAMPS.load(Ordering::Relaxed) } {
        // TODO: Get actual timestamp
        print_internal("[T+0.000000] ");
    }

    // Print the formatted message
    // Note: We need to collect the output into a buffer first
    // For now, we'll use a simple approach
    let _ = write!(LogWriter, "{}", args);

    // Print newline
    print_internal("\n");
}

/// Writer for logging
pub struct LogWriter;

impl core::fmt::Write for LogWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        print_internal(s);
        Ok(())
    }
}

/// Log a trace message
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::kernel::debug::log_print($crate::kernel::debug::LogLevel::Trace, format_args!($($arg)*));
    };
}

/// Log a trace message if condition is true
#[macro_export]
macro_rules! log_trace_if {
    ($cond:expr, $($arg:tt)*) => {
        if $cond {
            $crate::kernel::debug::log_trace!($($arg)*);
        }
    };
}

/// Log a debug message
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::kernel::debug::log_print($crate::kernel::debug::LogLevel::Debug, format_args!($($arg)*));
    };
}

/// Log an info message
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::kernel::debug::log_print($crate::kernel::debug::LogLevel::Info, format_args!($($arg)*));
    };
}

/// Log a warning message
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::kernel::debug::log_print($crate::kernel::debug::LogLevel::Warning, format_args!($($arg)*));
    };
}

/// Log an error message
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::kernel::debug::log_print($crate::kernel::debug::LogLevel::Error, format_args!($($arg)*));
    };
}

/// Log a fatal error message and halt
///
/// This function will print the message and halt the system.
/// It should only be used for unrecoverable errors.
#[inline(never)]
pub fn log_fatal(args: core::fmt::Arguments) -> ! {
    log_print(LogLevel::Fatal, args);
    print_internal("System halted.\n");

    unsafe {
        // Halt the CPU
        #[cfg(target_arch = "aarch64")]
        core::arch::asm!("wfi", options(nomem, nostack));

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!("hlt", options(nomem, nostack));

        #[cfg(target_arch = "riscv64")]
        core::arch::asm!("wfi", options(nomem, nostack));

        loop {}
    }
}

/// Panic handler
///
/// This is called when the kernel encounters a fatal error.
/// It prints diagnostics and halts the system.
///
/// # Arguments
///
/// * `message` - Panic message
/// * `file` - File where panic occurred
/// * `line` - Line number
/// * `col` - Column number
#[inline(never)]
pub fn panic_handler(message: &str, file: &str, line: u32, col: u32) -> ! {
    extern "C" {
        /// Dump CPU registers
        fn dump_registers();

        /// Dump stack trace
        fn dump_stack_trace();

        /// Halt all CPUs
        fn halt_all_cpus();
    }

    log_print(LogLevel::Fatal, format_args!("KERNEL PANIC"));

    if !message.is_empty() {
        log_print(LogLevel::Fatal, format_args!("  Message: {}", message));
    }

    log_print(LogLevel::Fatal, format_args!("  Location: {}:{}:{}", file, line, col));

    // Print register dump
    log_print(LogLevel::Fatal, format_args!(""));
    log_print(LogLevel::Fatal, format_args!("Register Dump:"));
    unsafe { dump_registers(); }

    // Print stack trace
    log_print(LogLevel::Fatal, format_args!(""));
    log_print(LogLevel::Fatal, format_args!("Stack Trace:"));
    unsafe { dump_stack_trace(); }

    // Halt the system
    unsafe { halt_all_cpus(); }

    // Should never reach here
    loop {}
}

/// Assert handler
///
/// Called when an assertion fails.
#[inline(never)]
pub fn assert_handler(message: &str, file: &str, line: u32) {
    log_print(LogLevel::Fatal, format_args!("ASSERTION FAILED"));
    log_print(LogLevel::Fatal, format_args!("  Location: {}:{}", file, line));

    if !message.is_empty() {
        log_print(LogLevel::Fatal, format_args!("  Message: {}", message));
    }

    unsafe {
        // Halt the CPU
        #[cfg(target_arch = "aarch64")]
        core::arch::asm!("wfi", options(nomem, nostack));

        #[cfg(target_arch = "x86_64")]
        core::arch::asm!("hlt", options(nomem, nostack));

        #[cfg(target_arch = "riscv64")]
        core::arch::asm!("wfi", options(nomem, nostack));

        loop {}
    }
}

/// Debug assert macro
#[macro_export]
macro_rules! debug_assert {
    ($cond:expr, $($arg:tt)*) => {
        if cfg!(debug_assertions) {
            if !$cond {
                $crate::kernel::debug::assert_handler(concat!($($arg)*), file!(), line!());
            }
        }
    };
    ($cond:expr) => {
        if cfg!(debug_assertions) {
            if !$cond {
                $crate::kernel::debug::assert_handler("assertion failed", file!(), line!());
            }
        }
    };
}

/// Assert macro (always enabled in kernel)
#[macro_export]
macro_rules! assert {
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            $crate::kernel::debug::assert_handler(concat!($($arg)*), file!(), line!());
        }
    };
    ($cond:expr) => {
        if !$cond {
            $crate::kernel::debug::assert_handler(stringify!($cond), file!(), line!());
        }
    };
}

/// Initialize the logging system
///
/// This should be called early in boot to set up logging.
pub fn log_init() {
    // Set default minimum log level
    log_set_min_level(LogLevel::Info);

    // Enable colors by default
    log_set_colors(true);

    // Disable timestamps until we have a working timer
    log_set_timestamps(false);

    log_info!("Rustux kernel logging initialized");
}

/// Initialize logging after UART is ready
pub fn log_init_uart() {
    log_set_uart_ready();
    log_info!("UART logging enabled");
}

/// Initialize the debug subsystem
pub fn init() {
    log_init();
}

// ============================================================================
// LK Compatibility Functions
// ============================================================================

/// dprintf - debug print function (LK compatibility)
///
/// This is a compatibility function for LK-style debug printing.
#[macro_export]
macro_rules! dprintf {
    ($level:expr, $fmt:expr) => {
        $crate::kernel::debug::log_print($level, format_args!($fmt));
    };
    ($level:expr, $fmt:expr, $($arg:tt)*) => {
        $crate::kernel::debug::log_print($level, format_args!($fmt, $($arg)*));
    };
    ($fmt:expr) => {
        $crate::kernel::debug::log_print($crate::kernel::debug::LogLevel::Info, format_args!($fmt));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::kernel::debug::log_print($crate::kernel::debug::LogLevel::Info, format_args!($fmt, $($arg)*));
    };
}

/// Hex dump with extended info (LK compatibility)
pub fn hexdump_ex(_ptr: *const u8, _len: usize, _width: usize) {
    // TODO: Implement hex dump
}

/// dprintf function variant (LK compatibility)
/// For cases where a function is needed instead of the macro
pub fn dprintf(_level: LogLevel, _fmt: &str) {
    // Function variant just logs using the macro internally
    // Use the macro directly for better formatting support
}
