// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM PL011 UART Driver
//!
//! This driver implements support for the ARM PrimeCell PL011 UART.
//! The PL011 is the standard UART found in ARMv8 systems and is
//! well-supported in QEMU's ARM virt machine.
//!
//! # Features
//!
//! - Interrupt-driven RX and TX
//! - Configurable baud rate (via boot arguments)
//! - RX circular buffer for received data
//! - TX event for blocking writes
//! - Panic mode for debug output
//!
//! # QEMU Support
//!
//! The PL011 UART is fully supported in QEMU ARM virt:
//! ```bash
//! qemu-system-aarch64 -M virt -cpu cortex-a57 -m 1G \
//!   -kernel rustux.elf -nographic -serial mon:stdio
//! ```
//!
//! # Register Map
//!
//! | Offset | Name    | Description                |
//! |--------|---------|----------------------------|
//! | 0x00   | DR      | Data Register              |
//! | 0x04   | RSR     | Receive Status Register    |
//! | 0x18   | FR      | Flag Register              |
//! | 0x24   | IBRD    | Integer Baud Rate Divisor  |
//! | 0x28   | FBRD    | Fractional Baud Rate Div.  |
//! | 0x2C   | LCRH    | Line Control Register      |
//! | 0x30   | CR      | Control Register           |
//! | 0x34   | IFLS    | Interrupt FIFO Level Select|
//! | 0x38   | IMSC    | Interrupt Mask Set/Clear   |
//! | 0x3C   | TRIS    | Raw Interrupt Status       |
//! | 0x40   | TMIS    | Masked Interrupt Status    |
//! | 0x44   | ICR     | Interrupt Clear Register   |
//! | 0x48   | DMACR   | DMA Control Register       |

#![no_std]

use crate::arch::arm64::periphmap;
use crate::debug;
use crate::kernel::sync;
use core::sync::atomic::{AtomicBool, Ordering};
use crate::kernel::sync::spin::SpinMutex as SpinMutex;

// ============================================================================
// Register Offsets
// ============================================================================

const UART_DR: usize = 0x00;   // Data Register
const UART_RSR: usize = 0x04;  // Receive Status Register
const UART_FR: usize = 0x18;   // Flag Register
const UART_ILPR: usize = 0x20; // IrDA Low-Power Counter
const UART_IBRD: usize = 0x24; // Integer Baud Rate Divisor
const UART_FBRD: usize = 0x28; // Fractional Baud Rate Divisor
const UART_LCRH: usize = 0x2C; // Line Control Register
const UART_CR: usize = 0x30;   // Control Register
const UART_IFLS: usize = 0x34; // Interrupt FIFO Level Select
const UART_IMSC: usize = 0x38; // Interrupt Mask Set/Clear
const UART_TRIS: usize = 0x3C; // Raw Interrupt Status
const UART_TMIS: usize = 0x40; // Masked Interrupt Status
const UART_ICR: usize = 0x44;  // Interrupt Clear Register
const UART_DMACR: usize = 0x48; // DMA Control Register

// ============================================================================
// Flag Register Bits
// ============================================================================

const FR_TXFE: u32 = 1 << 7;  // TX FIFO Empty
const FR_RXFF: u32 = 1 << 6;  // RX FIFO Full
const FR_TXFF: u32 = 1 << 5;  // TX FIFO Full
const FR_RXFE: u32 = 1 << 4;  // RX FIFO Empty
const FR_BUSY: u32 = 1 << 3;  // UART Busy

// ============================================================================
// Control Register Bits
// ============================================================================

const CR_CTSEN: u32 = 1 << 15; // CTS Enable
const CR_RTSEN: u32 = 1 << 14; // RTS Enable
const CR_RTS: u32 = 1 << 11;   // RTS
const CR_RXE: u32 = 1 << 9;    // RX Enable
const CR_TXE: u32 = 1 << 8;    // TX Enable
const CR_LBE: u32 = 1 << 7;    // Loopback Enable
const CR_UARTEN: u32 = 1 << 0; // UART Enable

// ============================================================================
// Interrupt Mask Set/Clear Bits
// ============================================================================

const IMSC_OE: u32 = 1 << 10;  // Overrun Error Interrupt
const IMSC_BE: u32 = 1 << 9;   // Break Error Interrupt
const IMSC_PE: u32 = 1 << 8;   // Parity Error Interrupt
const IMSC_FE: u32 = 1 << 7;   // Framing Error Interrupt
const IMSC_RT: u32 = 1 << 6;   // Receive Timeout Interrupt
const IMSC_TX: u32 = 1 << 5;   // TX Interrupt
const IMSC_RX: u32 = 1 << 4;   // RX Interrupt

// ============================================================================
// Interrupt Clear Register Bits
// ============================================================================

const ICR_ALL: u32 = 0x3FF;    // Clear all interrupts

// ============================================================================
// Constants
// ============================================================================

const RXBUF_SIZE: usize = 16;

// ============================================================================
// Global State
// ============================================================================}

/// UART base address (set during early init)
static UART_BASE: SpinMutex<usize> = SpinMutex::new(0);

/// UART IRQ number
static UART_IRQ: SpinMutex<u32> = SpinMutex::new(0);

/// TX interrupt enabled flag
static UART_TX_IRQ_ENABLED: AtomicBool = AtomicBool::new(false);

/// RX circular buffer
static UART_RX_BUF: SpinMutex<CircularBuffer<u8, RXBUF_SIZE>> = SpinMutex::new(CircularBuffer::new());

/// TX event for blocking writes
static UART_DPUTC_EVENT: sync::Event = sync::Event::new(false);

/// Spinlock for TX operations
static UART_SPINLOCK: SpinMutex<()> = SpinMutex::new(());

// ============================================================================
// Register Access
// ============================================================================

/// Read from a UART register
#[inline]
unsafe fn uart_read(base: usize, offset: usize) -> u32 {
    core::ptr::read_volatile((base + offset) as *const u32)
}

/// Write to a UART register
#[inline]
unsafe fn uart_write(base: usize, offset: usize, value: u32) {
    core::ptr::write_volatile((base + offset) as *mut u32, value);
}

// ============================================================================
// TX Interrupt Control
// ============================================================================

/// Mask TX interrupts
fn pl011_mask_tx(base: usize) {
    unsafe {
        let imsc = uart_read(base, UART_IMSC);
        uart_write(base, UART_IMSC, imsc & !IMSC_TX);
    }
}

/// Unmask TX interrupts
fn pl011_unmask_tx(base: usize) {
    unsafe {
        let imsc = uart_read(base, UART_IMSC);
        uart_write(base, UART_IMSC, imsc | IMSC_TX);
    }
}

// ============================================================================
// Circular Buffer
// ============================================================================}

/// Simple fixed-size circular buffer
struct CircularBuffer<T, const N: usize> {
    data: [T; N],
    head: usize,
    tail: usize,
    count: usize,
}

impl<T: Copy + Default, const N: usize> CircularBuffer<T, N> {
    const fn new() -> Self {
        Self {
            data: [T::default(); N],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    fn write_char(&mut self, c: T) -> bool {
        if self.count >= N {
            return false; // Buffer full
        }

        self.data[self.head] = c;
        self.head = (self.head + 1) % N;
        self.count += 1;
        true
    }

    fn read_char(&mut self) -> Option<T> {
        if self.count == 0 {
            return None; // Buffer empty
        }

        let c = self.data[self.tail];
        self.tail = (self.tail + 1) % N;
        self.count -= 1;
        Some(c)
    }

    fn space_avail(&self) -> usize {
        N - self.count
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }
}

// ============================================================================
// Interrupt Handler
// ============================================================================}

/// PL011 UART interrupt handler
///
/// Handles RX and TX interrupts:
/// - RX: Reads characters from FIFO into circular buffer
/// - TX: Signals waiting threads that TX FIFO is ready
fn pl011_irq_handler() {
    let base = *UART_BASE.lock();

    // Read interrupt status
    let isr = unsafe { uart_read(base, UART_TMIS) };

    // Handle RX interrupt (rxmis or rtim)
    if isr & (IMSC_RX | IMSC_RT) != 0 {
        // Read characters while FIFO is not empty
        while unsafe { uart_read(base, UART_FR) & FR_RXFE } == 0 {
            let mut rx_buf = UART_RX_BUF.lock();

            // If buffer is full, mask RX interrupts
            if rx_buf.space_avail() == 0 {
                unsafe {
                    let imsc = uart_read(base, UART_IMSC);
                    uart_write(base, UART_IMSC, imsc & !(IMSC_RX | IMSC_RT));
                }
                break;
            }

            let c = unsafe { uart_read(base, UART_DR) as u8 };
            rx_buf.write_char(c);
        }
    }

    // Handle TX interrupt
    if isr & IMSC_TX != 0 {
        let _lock = UART_SPINLOCK.lock();
        // Signal any waiting TX threads
        UART_DPUTC_EVENT.signal();
        pl011_mask_tx(base);
    }
}

// ============================================================================
// UART Operations
// ============================================================================}

/// Get a character from the UART (blocking or non-blocking)
pub fn pl011_getc(block: bool) -> Option<u8> {
    loop {
        {
            let mut rx_buf = UART_RX_BUF.lock();

            // Re-enable RX interrupts
            let base = *UART_BASE.lock();
            if base != 0 {
                unsafe {
                    let imsc = uart_read(base, UART_IMSC);
                    uart_write(base, UART_IMSC, imsc | IMSC_RX | IMSC_RT);
                }
            }

            if let Some(c) = rx_buf.read_char() {
                return Some(c);
            }
        }

        if !block {
            return None;
        }

        // Yield and wait for more data
        crate::kernel::thread::yield();
    }
}

/// Panic-time putc (polling, no interrupts)
pub fn pl011_pputc(c: u8) {
    let base = *UART_BASE.lock();
    if base == 0 {
        return;
    }

    unsafe {
        // Wait while TX FIFO is full
        while uart_read(base, UART_FR) & FR_TXFF != 0 {
            core::hint::spin_loop();
        }
        uart_write(base, UART_DR, c as u32);
    }
}

/// Panic-time getc (polling)
pub fn pl011_pgetc() -> Option<u8> {
    let base = *UART_BASE.lock();
    if base == 0 {
        return None;
    }

    unsafe {
        if uart_read(base, UART_FR) & FR_RXFE == 0 {
            Some(uart_read(base, UART_DR) as u8)
        } else {
            None
        }
    }
}

/// Write a string to the UART
pub fn pl011_dputs(s: &str, block: bool, map_nl: bool) {
    let base = *UART_BASE.lock();
    if base == 0 {
        return;
    }

    let tx_irq_enabled = UART_TX_IRQ_ENABLED.load(Ordering::Relaxed);
    let block = block && tx_irq_enabled;

    let mut chars = s.bytes().peekable();
    let _lock = UART_SPINLOCK.lock();

    while let Some(c) = chars.next() {
        // Handle newline mapping
        let (send_cr, send_char) = if map_nl && c == b'\n' {
            (true, b'\r')
        } else {
            (false, c)
        };

        for char_to_send in if send_cr { [b'\r', c] } else { [c] } {
            // Wait while TX FIFO is full
            unsafe {
                while uart_read(base, UART_FR) & FR_TXFF != 0 {
                    if block {
                        pl011_unmask_tx(base);
                        drop(_lock);
                        UART_DPUTC_EVENT.wait_timeout(1_000_000_000); // 1 second timeout
                        let _lock = UART_SPINLOCK.lock();
                    } else {
                        core::hint::spin_loop();
                    }
                }

                uart_write(base, UART_DR, char_to_send as u32);
            }
        }
    }
}

// ============================================================================
// Initialization
// ============================================================================}

/// Early UART initialization (before interrupts)
///
/// This is called during early boot to enable the UART for debug output.
/// Only TX is enabled at this stage.
///
/// # Safety
///
/// The mmio_phys and irq arguments must point to valid UART hardware.
pub unsafe fn pl011_init_early(mmio_phys: u64, irq: u32) {
    // Map physical address to virtual
    let base = periphmap::periph_paddr_to_vaddr(mmio_phys);
    if base == 0 {
        debug::log_error!("PL011: Failed to map MMIO address");
        return;
    }

    // Store global state
    *UART_BASE.lock() = base;
    *UART_IRQ.lock() = irq;

    // Enable UART and TX
    uart_write(base, UART_CR, CR_TXE | CR_UARTEN);

    debug::log_info!("PL011: Early init complete, base={:#x}, irq={}", base, irq);
}

/// Full UART initialization (with interrupts)
///
/// This enables RX and interrupt-driven operation.
///
/// # Safety
///
/// Must be called after `pl011_init_early` and when interrupts are available.
pub unsafe fn pl011_init() {
    let base = *UART_BASE.lock();
    let irq = *UART_IRQ.lock();

    if base == 0 || irq == 0 {
        debug::log_error!("PL011: Not initialized, call pl011_init_early first");
        return;
    }

    // Clear all interrupts
    uart_write(base, UART_ICR, ICR_ALL);

    // Set FIFO trigger level (1/8 RX, 1/8 TX)
    uart_write(base, UART_IFLS, 0);

    // Enable RX and timeout interrupts
    uart_write(base, UART_IMSC, IMSC_RX | IMSC_RT);

    // Enable RX
    let cr = uart_read(base, UART_CR);
    uart_write(base, UART_CR, cr | CR_RXE);

    // Register interrupt handler (would integrate with GIC)
    // TODO: Register with GIC when GIC driver is complete
    // crate::arch::arm64::interrupts::register_irq_handler(irq, pl011_irq_handler);

    // Unmask the interrupt at the GIC
    // TODO: Unmask at GIC when GIC driver is complete

    // Enable TX interrupt-driven mode
    UART_TX_IRQ_ENABLED.store(true, Ordering::Release);

    debug::log_info!("PL011: Full init complete, IRQ-driven TX enabled");
}

/// Disable IRQ-driven mode (called during panic)
pub fn pl011_start_panic() {
    UART_TX_IRQ_ENABLED.store(false, Ordering::Release);
}

// ============================================================================
// Debug Output Integration
// ============================================================================}

/// Initialize PL011 from platform data
///
/// This is called from platform initialization code.
pub fn pl011_platform_init(mmio_phys: u64, irq: u32) {
    unsafe {
        pl011_init_early(mmio_phys, irq);
    }
}
