// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! ARM GICv2 (Generic Interrupt Controller version 2) Driver
//!
//! This driver implements support for the ARM GICv2 interrupt controller.
//! The GICv2 is the most widely deployed interrupt controller in ARMv8 systems
//! and has excellent QEMU support.
//!
//! # Features
//!
//! - Full GICv2 register support
//! - SGI (Software Generated Interrupt) for IPI
//! - Per-CPU interrupt handling
//! - SPI (Shared Peripheral Interrupt) routing
//! - Interrupt masking/unmasking
//! - Trigger mode configuration (edge/level)
//!
//! # QEMU Support
//!
//! QEMU ARM virt fully supports GICv2:
//! ```bash
//! qemu-system-aarch64 -M virt -cpu cortex-a57 -m 1G \
//!   -kernel rustux.elf -nographic
//! ```
//!
//! # Register Map
//!
//! ## Distributor (GICD) Registers
//! | Offset | Name          | Description                     |
//! |--------|---------------|---------------------------------|
//! | 0x000  | GICD_CTLR     | Distributor Control Register    |
//! | 0x004  | GICD_TYPER    | Distributor Type Register       |
//! | 0x008  | GICD_IIDR     | Distributor Implementer ID      |
//! | 0x080  | GICD_IGROUPR  | Interrupt Group Registers       |
//! | 0x100  | GICD_ISENABLER| Interrupt Set-Enable Registers  |
//! | 0x180  | GICD_ICENABLER| Interrupt Clear-Enable Registers|
//! | 0x200  | GICD_ISPENDR  | Interrupt Set-Pending Registers |
//! | 0x280  | GICD_ICPENDR  | Interrupt Clear-Pending Regs    |
//! | 0x300  | GICD_ISACTIVER| Interrupt Set-Active Registers  |
//! | 0x380  | GICD_ICACTIVER| Interrupt Clear-Active Regs     |
//! | 0x400  | GICD_IPRIORITYR| Interrupt Priority Registers   |
//! | 0x800  | GICD_ITARGETSR| Interrupt Processor Targets     |
//! | 0xC00  | GICD_ICFGR    | Interrupt Configuration Registers|
//! | 0xF00  | GICD_SGIR     | Software Generated Interrupt Reg |
//!
//! ## CPU Interface (GICC) Registers
//! | Offset | Name    | Description                     |
//! |--------|---------|---------------------------------|
//! | 0x0000 | GICC_CTLR| CPU Interface Control Register |
//! | 0x0004 | GICC_PMR | Interrupt Priority Mask Register|
//! | 0x000C | GICC_IAR | Interrupt Acknowledge Register  |
//! | 0x0010 | GICC_EOIR| End of Interrupt Register      |
//! | 0x1000 | GICC_DIR | Deactivate Interrupt Register    |

#![no_std]

use crate::arch::arm64::periphmap;
use crate::kernel::mp::MpIpiType;
use crate::{log_info, log_error, log_debug};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::kernel::sync::spin::SpinMutex as SpinMutex;

// ============================================================================
// Re-export parent module types
// ============================================================================

pub use super::arm_gic::*;

// ============================================================================
// Register Offsets
// ============================================================================

// Distributor registers
const GICD_CTLR: usize = 0x000;
const GICD_TYPER: usize = 0x004;
const GICD_IIDR: usize = 0x008;
const GICD_IGROUPR: usize = 0x080;
const GICD_ISENABLER: usize = 0x100;
const GICD_ICENABLER: usize = 0x180;
const GICD_ISPENDR: usize = 0x200;
const GICD_ICPENDR: usize = 0x280;
const GICD_ISACTIVER: usize = 0x300;
const GICD_ICACTIVER: usize = 0x380;
const GICD_IPRIORITYR: usize = 0x400;
const GICD_ITARGETSR: usize = 0x800;
const GICD_ICFGR: usize = 0xC00;
const GICD_SGIR: usize = 0xF00;
const GICD_CPENDSGIR: usize = 0xF10;
const GICD_SPENDSGIR: usize = 0xF20;

// CPU interface registers
const GICC_CTLR: usize = 0x0000;
const GICC_PMR: usize = 0x0004;
const GICC_BPR: usize = 0x0008;
const GICC_IAR: usize = 0x000C;
const GICC_EOIR: usize = 0x0010;
const GICC_RPR: usize = 0x0014;
const GICC_HPPIR: usize = 0x0018;
const GICC_AIAR: usize = 0x0020;
const GICC_AEOIR: usize = 0x0024;
const GICC_AHPPIR: usize = 0x0028;
const GICC_APR: usize = 0x00D0;
const GICC_NSAPR: usize = 0x00E0;
const GICC_IIDR: usize = 0x00FC;
const GICC_DIR: usize = 0x1000;

// Identification registers
const GICD_PIDR2: usize = 0xFF8;

// ============================================================================
// Global State
// ============================================================================

/// GIC base virtual address
static GIC_BASE: AtomicU64 = AtomicU64::new(0);

/// GICD offset
static GICD_OFFSET: AtomicU64 = AtomicU64::new(0);

/// GICC offset
static GICC_OFFSET: AtomicU64 = AtomicU64::new(0);

/// GICH offset (for virtualization)
static GICH_OFFSET: AtomicU64 = AtomicU64::new(0);

/// GICV offset (for virtualization)
static GICV_OFFSET: AtomicU64 = AtomicU64::new(0);

/// IPI base interrupt number
static IPI_BASE: AtomicU32 = AtomicU32::new(0);

/// Maximum number of interrupts
static MAX_IRQS: AtomicU32 = AtomicU32::new(0);

/// GIC SPI interrupt base (Shared Peripheral Interrupts)
const GIC_BASE_SPI: u32 = 32;

/// Distributor lock
static GICD_LOCK: SpinMutex<()> = SpinMutex::new(());

// ============================================================================
// Register Access
// ============================================================================

/// Get GICD base address
#[inline]
fn gicd_base() -> usize {
    (GIC_BASE.load(Ordering::Acquire) + GICD_OFFSET.load(Ordering::Acquire)) as usize
}

/// Get GICC base address
#[inline]
fn gicc_base() -> usize {
    (GIC_BASE.load(Ordering::Acquire) + GICC_OFFSET.load(Ordering::Acquire)) as usize
}

/// Read from a GICD register
#[inline]
unsafe fn gicd_read(offset: usize) -> u32 {
    let base = gicd_base();
    core::ptr::read_volatile((base + offset) as *const u32)
}

/// Write to a GICD register
#[inline]
unsafe fn gicd_write(offset: usize, value: u32) {
    let base = gicd_base();
    core::ptr::write_volatile((base + offset) as *mut u32, value);
}

/// Read from a GICC register
#[inline]
unsafe fn gicc_read(offset: usize) -> u32 {
    let base = gicc_base();
    core::ptr::read_volatile((base + offset) as *const u32)
}

/// Write to a GICC register
#[inline]
unsafe fn gicc_write(offset: usize, value: u32) {
    let base = gicc_base();
    core::ptr::write_volatile((base + offset) as *mut u32, value);
}

// ============================================================================
// Public API
// ============================================================================

/// Initialize GICv2 from platform data
///
/// # Arguments
///
/// * `mmio_phys` - Physical base address of GIC registers
/// * `gicd_offset` - Offset to GICD registers
/// * `gicc_offset` - Offset to GICC registers
/// * `gich_offset` - Offset to GICH registers (for virtualization)
/// * `gicv_offset` - Offset to GICV registers (for virtualization)
/// * `ipi_base` - Base interrupt number for IPIs
///
/// # Safety
///
/// The mmio_phys must point to valid GIC hardware registers
pub unsafe fn platform_init(
    mmio_phys: u64,
    gicd_offset: u64,
    gicc_offset: u64,
    gich_offset: u64,
    gicv_offset: u64,
    ipi_base: u32,
) -> Result<(), &'static str> {
    // Map physical address to virtual
    let base = periphmap::periph_paddr_to_vaddr(mmio_phys);
    if base == 0 {
        return Err("Failed to map GIC MMIO address");
    }

    // Store configuration
    GIC_BASE.store(base as u64, Ordering::Release);
    GICD_OFFSET.store(gicd_offset, Ordering::Release);
    GICC_OFFSET.store(gicc_offset, Ordering::Release);
    GICH_OFFSET.store(gich_offset, Ordering::Release);
    GICV_OFFSET.store(gicv_offset, Ordering::Release);
    IPI_BASE.store(ipi_base, Ordering::Release);

    // Initialize GIC
    if let Err(e) = init() {
        return Err(e);
    }

    log_info!("GICv2: Initialized, base={:#x}, max_irqs={}",
                     base, MAX_IRQS.load(Ordering::Acquire));

    Ok(())
}

/// Initialize the GICv2
///
/// This function:
/// 1. Detects GIC version
/// 2. Reads maximum number of IRQs
/// 3. Disables all interrupts
/// 4. Sets all SPIs to target CPU 0
/// 5. Configures all SPIs as edge-triggered
/// 6. Enables the distributor
/// 7. Initializes per-CPU interface
unsafe fn init() -> Result<(), &'static str> {
    // Check if this is GICv2
    let pidr2 = gicd_read(GICD_PIDR2);
    if pidr2 != 0 {
        let rev = (pidr2 >> 4) & 0xF;
        if rev != 2 {
            return Err("Not a GICv2");
        }
    }

    // Read max IRQs from TYPER
    let typer = gicd_read(GICD_TYPER);
    let it_lines_number = typer & 0x1F;
    let max_irqs = ((it_lines_number + 1) * 32) as u32;
    MAX_IRQS.store(max_irqs, Ordering::Release);

    log_debug!("GICv2: max_irqs={}", max_irqs);

    // Disable all interrupts
    for i in (0..max_irqs).step_by(32) {
        gicd_write(GICD_ICENABLER + ((i as usize) / 32) * 4, 0xFFFFFFFF);
        gicd_write(GICD_ICPENDR + ((i as usize) / 32) * 4, 0xFFFFFFFF);
    }

    // Set SPI targets to CPU 0 (for CPU 0-7)
    let max_cpu = ((typer >> 5) & 0x7) as u32;
    if max_cpu > 0 {
        for i in (32..max_irqs).step_by(4) {
            gicd_write(GICD_ITARGETSR + ((i as usize) / 4) * 4, 0x01010101);
        }
    }

    // Configure all SPIs as edge-triggered
    for i in GIC_BASE_SPI..max_irqs {
        let _ = configure_interrupt(i, InterruptTriggerMode::Edge, InterruptPolarity::ActiveHigh);
    }

    // Enable distributor
    gicd_write(GICD_CTLR, 1);

    // Initialize per-CPU interface
    init_percpu_early();

    Ok(())
}

/// Initialize per-CPU interface (early, before IRQ handlers)
pub fn init_percpu_early() {
    unsafe {
        // Enable Group 1 and set EOImodeNS
        gicc_write(GICC_CTLR, 0x201);
        // Unmask all priority levels
        gicc_write(GICC_PMR, 0xFF);
    }
}

/// Initialize per-CPU (full initialization)
pub fn init_percpu() {
    // TODO: Mark CPU as online
    // TODO: Unmask IPIs
    let ipi_base = IPI_BASE.load(Ordering::Acquire);
    let _ = unmask_interrupt(MpIpiType::Generic as u32 + ipi_base);
    let _ = unmask_interrupt(MpIpiType::Reschedule as u32 + ipi_base);
    let _ = unmask_interrupt(MpIpiType::Interrupt as u32 + ipi_base);
    let _ = unmask_interrupt(MpIpiType::Halt as u32 + ipi_base);
}

/// Mask (disable) an interrupt
pub fn mask_interrupt(irq: u32) -> i32 {
    let max_irqs = MAX_IRQS.load(Ordering::Acquire);
    if irq >= max_irqs {
        return -1;
    }

    let _lock = GICD_LOCK.lock();
    unsafe {
        let reg = GICD_ICENABLER + ((irq as usize) / 32) * 4;
        let mask = 1u32 << (irq % 32);
        let current = gicd_read(reg);
        gicd_write(reg, current | mask);
    }

    0
}

/// Unmask (enable) an interrupt
pub fn unmask_interrupt(irq: u32) -> i32 {
    let max_irqs = MAX_IRQS.load(Ordering::Acquire);
    if irq >= max_irqs {
        return -1;
    }

    let _lock = GICD_LOCK.lock();
    unsafe {
        let reg = GICD_ISENABLER + ((irq as usize) / 32) * 4;
        let mask = 1u32 << (irq % 32);
        let current = gicd_read(reg);
        gicd_write(reg, current | mask);
    }

    0
}

/// Configure interrupt trigger mode
///
/// Only works for SPI interrupts (IRQ >= 32)
pub fn configure_interrupt(
    irq: u32,
    tm: InterruptTriggerMode,
    _pol: InterruptPolarity,
) -> i32 {
    let max_irqs = MAX_IRQS.load(Ordering::Acquire);
    if irq >= max_irqs || irq < GIC_BASE_SPI {
        return -1;
    }

    let _lock = GICD_LOCK.lock();
    unsafe {
        let reg = GICD_ICFGR + ((irq as usize) / 16) * 4;
        let bit_shift = ((irq % 16) * 2) + 1;
        let current = gicd_read(reg);
        let new_val = if tm == InterruptTriggerMode::Edge {
            current | (1 << bit_shift)
        } else {
            current & !(1 << bit_shift)
        };
        gicd_write(reg, new_val);
    }

    0
}

/// Send a Software Generated Interrupt (SGI)
pub fn send_sgi(sgi_num: u32, flags: u32, cpu_mask: u32) -> i32 {
    if sgi_num >= 16 {
        return -1;
    }

    let val = (flags & ARM_GIC_SGI_FLAG_TARGET_FILTER_MASK)
        | ((cpu_mask & 0xFF) << 16)
        | (sgi_num & 0xF);

    let _lock = GICD_LOCK.lock();
    unsafe {
        gicd_write(GICD_SGIR, val);
    }

    0
}

/// Get maximum interrupt ID
pub fn get_max_irq() -> u32 {
    MAX_IRQS.load(Ordering::Acquire)
}

/// Handle an interrupt
///
/// Returns the interrupt ID and whether it was spurious
pub fn handle_irq() -> (u32, bool) {
    unsafe {
        let iar = gicc_read(GICC_IAR);
        let vector = iar & 0x3FF;

        // Check for spurious interrupt (0x3FE or 0x3FF)
        if vector >= 0x3FE {
            return (vector, true);
        }

        // End of interrupt
        gicc_write(GICC_EOIR, iar);

        (vector, false)
    }
}

/// Shutdown the GIC (disable distributor)
pub fn shutdown() {
    unsafe {
        gicd_write(GICD_CTLR, 0);
    }
}

/// Shutdown per-CPU interface
pub fn shutdown_percpu() {
    unsafe {
        gicc_write(GICC_CTLR, 0);
    }
}

/// Get GIC version from IIDR register
pub fn get_iidr() -> u32 {
    unsafe {
        gicc_read(GICC_IIDR)
    }
}
