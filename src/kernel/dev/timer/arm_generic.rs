// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM Generic Timer Driver
//!
//! This driver implements support for the ARM Generic Timer, which is part of
//! the ARMv8-A architecture. The timer provides:
//!
//! - A global counter (CNTVCT or CNTPCT) that increments at a fixed frequency
//! - Per-CPU virtual timers (CNTV) for use by guest OSes
//! - Per-CPU physical timers (CNTP) for secure/non-secure state
//! - Per-CPU secure physical timers (CNTPS) for secure state only
//!
//! # Features
//!
//! - High-resolution monotonic counter (nanosecond precision)
//! - One-shot timer configuration
//! - Interrupt-driven timer expiration
//! - Tick conversion between counter and nanoseconds
//!
//! # QEMU Support
//!
//! QEMU ARM virt fully supports the ARM Generic Timer:
//! ```bash
//! qemu-system-aarch64 -M virt -cpu cortex-a57 -m 1G \
//!   -kernel rustux.elf -nographic
//! ```
//!
//! # Register Access
//!
//! The timer registers are accessed via system registers (MRS/MSR instructions):
//!
//! | Register | Description | Access |
//! |----------|-------------|--------|
//! | `cntfrq_el0` | Counter frequency | R/W |
//! | `cntpct_el0` | Physical counter value | R |
//! | `cntvct_el0` | Virtual counter value | R |
//! | `cntp_cval_el0` | Physical timer compare value | R/W |
//! | `cntp_tval_el0` | Physical timer timer value | R/W |
//! | `cntp_ctl_el0` | Physical timer control | R/W |
//!
//! # Timer Selection
//!
//! The driver can use any of the following timer types:
//! - **Physical timer (CNTP)**: Used in EL1/EL2, typically for non-secure state
//! - **Virtual timer (CNTV)**: Used by guest OSes in virtualized environments
//! - **Secure physical (CNTPS)**: Used in EL3 for secure state
//!
//! By default, the physical timer is used unless booting at EL1 without virtualization.


use crate::{log_info, log_error, log_debug};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Timer Selection
// ============================================================================

/// Timer type selection
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerType {
    Physical = 0,
    Virtual = 1,
    SecurePhysical = 2,
}

/// Currently selected timer type
static TIMER_TYPE: AtomicU32 = AtomicU32::new(TimerType::Physical as u32);

// ============================================================================
// Register Access - Physical Timer (CNTP)
// ============================================================================

/// Read counter frequency register (CNTFRQ_EL0)
#[inline]
pub fn read_cntfrq() -> u32 {
    let freq: u32;
    unsafe {
        core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) freq);
    }
    freq
}

/// Read physical counter register (CNTPCT_EL0)
#[inline]
pub fn read_cntpct() -> u64 {
    let cnt: u64;
    unsafe {
        core::arch::asm!("mrs {0}, cntpct_el0", out(reg) cnt);
    }
    cnt
}

/// Read physical timer control register (CNTP_CTL_EL0)
#[inline]
fn read_cntp_ctl() -> u32 {
    let ctl: u32;
    unsafe {
        core::arch::asm!("mrs {0}, cntp_ctl_el0", out(reg) ctl);
    }
    ctl
}

/// Write physical timer control register (CNTP_CTL_EL0)
#[inline]
fn write_cntp_ctl(val: u32) {
    unsafe {
        core::arch::asm!("msr cntp_ctl_el0, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

/// Write physical timer compare value register (CNTP_CVAL_EL0)
#[inline]
fn write_cntp_cval(val: u64) {
    unsafe {
        core::arch::asm!("msr cntp_cval_el0, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

/// Write physical timer value register (CNTP_TVAL_EL0)
#[inline]
fn write_cntp_tval(val: i32) {
    unsafe {
        core::arch::asm!("msr cntp_tval_el0, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

// ============================================================================
// Register Access - Virtual Timer (CNTV)
// ============================================================================

/// Read virtual counter register (CNTVCT_EL0)
#[inline]
fn read_cntvct() -> u64 {
    let cnt: u64;
    unsafe {
        core::arch::asm!("mrs {0}, cntvct_el0", out(reg) cnt);
    }
    cnt
}

/// Read virtual timer control register (CNTV_CTL_EL0)
#[inline]
fn read_cntv_ctl() -> u32 {
    let ctl: u32;
    unsafe {
        core::arch::asm!("mrs {0}, cntv_ctl_el0", out(reg) ctl);
    }
    ctl
}

/// Write virtual timer control register (CNTV_CTL_EL0)
#[inline]
fn write_cntv_ctl(val: u32) {
    unsafe {
        core::arch::asm!("msr cntv_ctl_el0, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

/// Write virtual timer compare value register (CNTV_CVAL_EL0)
#[inline]
fn write_cntv_cval(val: u64) {
    unsafe {
        core::arch::asm!("msr cntv_cval_el0, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

/// Write virtual timer value register (CNTV_TVAL_EL0)
#[inline]
fn write_cntv_tval(val: i32) {
    unsafe {
        core::arch::asm!("msr cntv_tval_el0, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

// ============================================================================
// Register Access - Secure Physical Timer (CNTPS)
// ============================================================================

/// Read secure physical timer control register (CNTPS_CTL_EL1)
#[inline]
fn read_cntps_ctl() -> u32 {
    let ctl: u32;
    unsafe {
        core::arch::asm!("mrs {0}, cntps_ctl_el1", out(reg) ctl);
    }
    ctl
}

/// Write secure physical timer control register (CNTPS_CTL_EL1)
#[inline]
fn write_cntps_ctl(val: u32) {
    unsafe {
        core::arch::asm!("msr cntps_ctl_el1, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

/// Write secure physical timer compare value register (CNTPS_CVAL_EL1)
#[inline]
fn write_cntps_cval(val: u64) {
    unsafe {
        core::arch::asm!("msr cntps_cval_el1, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

/// Write secure physical timer value register (CNTPS_TVAL_EL1)
#[inline]
fn write_cntps_tval(val: i32) {
    unsafe {
        core::arch::asm!("msr cntps_tval_el1, {0}", in(reg) val);
        core::arch::asm!("isb", options(nostack));
    }
}

// ============================================================================
// Global State
// ============================================================================

/// Timer IRQ number
static TIMER_IRQ: AtomicU32 = AtomicU32::new(0);

/// Counter frequency in Hz
static CNTFRQ: AtomicU32 = AtomicU32::new(0);

/// Conversion factor: counter ticks per nanosecond (as 64.64 fixed point)
static CTPCT_PER_NS: AtomicU64 = AtomicU64::new(0);

/// Conversion factor: nanoseconds per counter tick (as 64.64 fixed point)
static NS_PER_CTPCT: AtomicU64 = AtomicU64::new(0);

// ============================================================================
// Timer Type Dispatch
// ============================================================================

/// Read counter value from currently selected timer
#[inline]
pub fn read_counter() -> u64 {
    match TIMER_TYPE.load(Ordering::Acquire) as u32 {
        x if x == TimerType::Virtual as u32 => read_cntvct(),
        _ => read_cntpct(), // Physical and SecurePhysical both use CNTPCT
    }
}

/// Stop the timer
#[inline]
pub fn stop_timer() {
    match TIMER_TYPE.load(Ordering::Acquire) as u32 {
        x if x == TimerType::Virtual as u32 => write_cntv_ctl(0),
        x if x == TimerType::SecurePhysical as u32 => write_cntps_ctl(0),
        _ => write_cntp_ctl(0),
    }
}

/// Write timer compare value
#[inline]
fn write_cval(val: u64) {
    match TIMER_TYPE.load(Ordering::Acquire) as u32 {
        x if x == TimerType::Virtual as u32 => write_cntv_cval(val),
        x if x == TimerType::SecurePhysical as u32 => write_cntps_cval(val),
        _ => write_cntp_cval(val),
    }
}

/// Write timer value (tval)
#[inline]
fn write_tval(val: i32) {
    match TIMER_TYPE.load(Ordering::Acquire) as u32 {
        x if x == TimerType::Virtual as u32 => write_cntv_tval(val),
        x if x == TimerType::SecurePhysical as u32 => write_cntps_tval(val),
        _ => write_cntp_tval(val),
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Convert nanoseconds to counter ticks
#[inline]
fn ns_to_ticks(ns: u64) -> u64 {
    let factor = CTPCT_PER_NS.load(Ordering::Acquire);
    // Multiply using 64.64 fixed point
    // Result = (ns * factor) >> 64
    let hi = (ns >> 32) * factor;
    let lo = (ns & 0xFFFFFFFF) * factor;
    ((hi << 32) + (lo >> 32) + ((lo & 0xFFFFFFFF) >> 31))
}

/// Convert counter ticks to nanoseconds
#[inline]
fn ticks_to_ns(ticks: u64) -> u64 {
    let factor = NS_PER_CTPCT.load(Ordering::Acquire);
    // Multiply using 64.64 fixed point
    // Result = (ticks * factor) >> 64
    let hi = (ticks >> 32) * factor;
    let lo = (ticks & 0xFFFFFFFF) * factor;
    ((hi << 32) + (lo >> 32) + ((lo & 0xFFFFFFFF) >> 31))
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize ARM Generic Timer
///
/// # Arguments
///
/// * `irq_phys` - Physical timer IRQ number
/// * `irq_virt` - Virtual timer IRQ number
/// * `irq_sphys` - Secure physical timer IRQ number
/// * `freq_override` - Optional frequency override (0 = use hardware value)
/// * `use_virtual` - Use virtual timer instead of physical
///
/// # Safety
///
/// Must be called only once during platform initialization
pub unsafe fn init(
    irq_phys: u32,
    irq_virt: u32,
    irq_sphys: u32,
    freq_override: u32,
    use_virtual: bool,
) -> Result<(), &'static str> {
    // Select timer type
    let (irq, timer_type) = if use_virtual && irq_virt != 0 {
        (irq_virt, TimerType::Virtual)
    } else if irq_phys != 0 {
        (irq_phys, TimerType::Physical)
    } else if irq_sphys != 0 {
        (irq_sphys, TimerType::SecurePhysical)
    } else {
        return Err("No timer IRQ configured");
    };

    TIMER_IRQ.store(irq, Ordering::Release);
    TIMER_TYPE.store(timer_type as u32, Ordering::Release);

    // Read or override frequency
    let freq = if freq_override != 0 {
        freq_override
    } else {
        read_cntfrq()
    };

    if freq == 0 {
        return Err("Timer frequency is zero");
    }

    CNTFRQ.store(freq, Ordering::Release);

    // Calculate conversion factors
    // CTPCT_PER_NS = freq / 1e9 (as 64.64 fixed point)
    // NS_PER_CTPCT = 1e9 / freq (as 64.64 fixed point)
    let freq_u64 = freq as u64;
    let ns_per_sec: u64 = 1_000_000_000;

    // Fixed-point division: CTPCT_PER_NS = (freq << 64) / 1e9
    // For simplicity, approximate with floating point
    let ctpct_per_ns = ((freq_u64 as u128) << 64) / (ns_per_sec as u128);
    CTPCT_PER_NS.store(ctpct_per_ns as u64, Ordering::Release);

    // Fixed-point division: NS_PER_CTPCT = (1e9 << 64) / freq
    let ns_per_ctpct = ((ns_per_sec as u128) << 64) / (freq_u64 as u128);
    NS_PER_CTPCT.store(ns_per_ctpct as u64, Ordering::Release);

    log_info!("ARM Generic Timer: freq={} Hz, timer_type={:?}, irq={}",
                     freq, timer_type, irq);

    // TODO: Register interrupt handler
    // TODO: Unmask timer IRQ

    Ok(())
}

/// Get current time in nanoseconds
pub fn current_time_ns() -> u64 {
    let ticks = read_counter();
    ticks_to_ns(ticks)
}

/// Get current time in microseconds
pub fn current_time_us() -> u64 {
    current_time_ns() / 1000
}

/// Get current time in milliseconds
pub fn current_time_ms() -> u64 {
    current_time_ns() / 1_000_000
}

/// Get current counter value (ticks)
pub fn current_ticks() -> u64 {
    read_counter()
}

/// Get timer frequency in Hz
pub fn timer_frequency() -> u32 {
    CNTFRQ.load(Ordering::Acquire)
}

/// Get ticks per second
pub fn ticks_per_second() -> u64 {
    CNTFRQ.load(Ordering::Acquire) as u64
}

/// Set a one-shot timer
///
/// # Arguments
///
/// * `deadline_ns` - Deadline in nanoseconds (absolute time)
///
/// The timer will fire an interrupt when the counter reaches or exceeds the deadline.
pub fn set_oneshot_timer(deadline_ns: u64) {
    let deadline_ticks = ns_to_ticks(deadline_ns) + 1;
    write_cval(deadline_ticks);

    // Enable timer
    match TIMER_TYPE.load(Ordering::Acquire) as u32 {
        x if x == TimerType::Virtual as u32 => write_cntv_ctl(1),
        x if x == TimerType::SecurePhysical as u32 => write_cntps_ctl(1),
        _ => write_cntp_ctl(1),
    }
}

/// Stop the timer
pub fn stop() {
    stop_timer();
}

/// Initialize timer for secondary CPU
pub fn init_secondary_cpu() {
    let irq = TIMER_IRQ.load(Ordering::Acquire);
    if irq != 0 {
        // TODO: Unmask timer IRQ on this CPU
        log_debug!("ARM Generic Timer: CPU init, irq={}", irq);
    }
}

/// Get timer IRQ number
pub fn timer_irq() -> u32 {
    TIMER_IRQ.load(Ordering::Acquire)
}

/// Platform-specific timer initialization from driver data
///
/// # Arguments
///
/// * `irq_phys` - Physical timer IRQ
/// * `irq_virt` - Virtual timer IRQ
/// * `irq_sphys` - Secure physical timer IRQ
/// * `freq_override` - Optional frequency override
pub fn platform_init(
    irq_phys: u32,
    irq_virt: u32,
    irq_sphys: u32,
    freq_override: u32,
) -> Result<(), &'static str> {
    // Determine if we should use virtual timer
    // Use virtual timer if available and not booting at EL2
    let use_virtual = irq_virt != 0 && !is_boot_el2();

    unsafe {
        init(irq_phys, irq_virt, irq_sphys, freq_override, use_virtual)
    }
}

/// Check if we booted at EL2 or higher
fn is_boot_el2() -> bool {
    let current_el: u64;
    unsafe {
        core::arch::asm!("mrs {0}, CurrentEL", out(reg) current_el);
    }
    // CurrentEL format: [3:2] contains EL
    let el = (current_el >> 2) & 0x3;
    el >= 2
}
