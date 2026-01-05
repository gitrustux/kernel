// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! APIC (Advanced Programmable Interrupt Controller) support for Rustux
//!
//! This module provides functions for interacting with both local APICs and IO APICs
//! on x86 systems, including sending IPIs, configuring interrupts, and timer management.

use crate::dev::interrupt::{InterruptPolarity, InterruptTriggerMode};
use crate::rustux::types::*;

/// Represents an invalid APIC ID
pub const INVALID_APIC_ID: u32 = 0xffffffff;
/// Physical base address of the APIC
pub const APIC_PHYS_BASE: u32 = 0xfee00000;
/// Bootstrap Processor flag in IA32_APIC_BASE MSR
pub const IA32_APIC_BASE_BSP: u32 = 1 << 8;
/// x2APIC mode enable flag in IA32_APIC_BASE MSR
pub const IA32_APIC_BASE_X2APIC_ENABLE: u32 = 1 << 10;
/// xAPIC mode enable flag in IA32_APIC_BASE MSR
pub const IA32_APIC_BASE_XAPIC_ENABLE: u32 = 1 << 11;
/// Number of ISA IRQs
pub const NUM_ISA_IRQS: usize = 16;

// LVT Timer bitmasks
/// Mask for timer vector field in LVT Timer register
pub const LVT_TIMER_VECTOR_MASK: u32 = 0x000000ff;
/// Mask for timer mode field in LVT Timer register
pub const LVT_TIMER_MODE_MASK: u32 = 0x00060000;
/// One-shot timer mode
pub const LVT_TIMER_MODE_ONESHOT: u32 = 0 << 17;
/// Periodic timer mode
pub const LVT_TIMER_MODE_PERIODIC: u32 = 1 << 17;
/// TSC deadline timer mode
pub const LVT_TIMER_MODE_TSC_DEADLINE: u32 = 2 << 17;
/// Reserved timer mode
pub const LVT_TIMER_MODE_RESERVED: u32 = 3 << 17;
/// LVT masked bit
pub const LVT_MASKED: u32 = 1 << 16;

/// APIC interrupt delivery modes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApicInterruptDeliveryMode {
    /// Fixed delivery mode (use this unless you know what you're doing)
    Fixed = 0,
    /// Lowest priority delivery
    LowestPri = 1,
    /// System Management Interrupt
    Smi = 2,
    /// Non-Maskable Interrupt
    Nmi = 4,
    /// INIT
    Init = 5,
    /// Startup IPI
    Startup = 6,
    /// External Interrupt
    ExtInt = 7,
}

/// APIC interrupt destination modes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApicInterruptDstMode {
    /// Physical destination mode
    Physical = 0,
    /// Logical destination mode
    Logical = 1,
}

/// IO APIC register offsets and constants
pub mod io_apic {
    /// IO register select register offset
    pub const IOREGSEL: u32 = 0x00;
    /// IO register window offset
    pub const IOWIN: u32 = 0x10;
    /// ID register
    pub const REG_ID: u32 = 0x00;
    /// Version register
    pub const REG_VER: u32 = 0x01;
    /// Mask an IRQ
    pub const IRQ_MASK: bool = true;
    /// Unmask an IRQ
    pub const IRQ_UNMASK: bool = false;
}

/// Descriptor for an IO APIC in the system
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoApicDescriptor {
    /// APIC ID
    pub apic_id: u8,
    /// Virtual IRQ base for ACPI
    pub global_irq_base: u32,
    /// Physical address of the base of this IOAPIC's MMIO
    pub paddr: PAddr,
}

/// Information describing an ISA override
///
/// An override can change the global IRQ number and/or change bus signaling 
/// characteristics for the specified ISA IRQ.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoApicIsaOverride {
    /// The ISA IRQ number
    pub isa_irq: u8,
    /// Whether this IRQ is remapped
    pub remapped: bool,
    /// Trigger mode
    pub tm: InterruptTriggerMode,
    /// Polarity
    pub pol: InterruptPolarity,
    /// Global IRQ number
    pub global_irq: u32,
}

/// Initialize the VM APIC support
pub fn apic_vm_init() {
    unsafe { sys_apic_vm_init() }
}

/// Initialize the local APIC
pub fn apic_local_init() {
    unsafe { sys_apic_local_init() }
}

/// Get the local APIC ID
pub fn apic_local_id() -> u8 {
    unsafe { sys_apic_local_id() }
}

/// Get the APIC ID of the bootstrap processor
pub fn apic_bsp_id() -> u8 {
    unsafe { sys_apic_bsp_id() }
}

/// Enable or disable a specific interrupt vector
pub fn apic_irq_set(vector: u32, enable: bool) {
    unsafe { sys_apic_irq_set(vector, enable) }
}

/// Send an Inter-Processor Interrupt (IPI)
pub fn apic_send_ipi(
    vector: u8,
    dst_apic_id: u32,
    dm: ApicInterruptDeliveryMode,
) {
    unsafe { sys_apic_send_ipi(vector, dst_apic_id, dm) }
}

/// Send an IPI to self
pub fn apic_send_self_ipi(vector: u8, dm: ApicInterruptDeliveryMode) {
    unsafe { sys_apic_send_self_ipi(vector, dm) }
}

/// Send a broadcast IPI to all processors
pub fn apic_send_broadcast_ipi(vector: u8, dm: ApicInterruptDeliveryMode) {
    unsafe { sys_apic_send_broadcast_ipi(vector, dm) }
}

/// Send a broadcast IPI to all processors except self
pub fn apic_send_broadcast_self_ipi(vector: u8, dm: ApicInterruptDeliveryMode) {
    unsafe { sys_apic_send_broadcast_self_ipi(vector, dm) }
}

/// Issue an End-Of-Interrupt to the local APIC
pub fn apic_issue_eoi() {
    unsafe { sys_apic_issue_eoi() }
}

/// Configure APIC timer in one-shot mode
///
/// # Returns
///
/// `Ok(())` on success, or an error code on failure
pub fn apic_timer_set_oneshot(count: u32, divisor: u8, masked: bool) -> RxStatus {
    unsafe { sys_apic_timer_set_oneshot(count, divisor, masked) }
}

/// Configure APIC timer in TSC deadline mode
pub fn apic_timer_set_tsc_deadline(deadline: u64, masked: bool) {
    unsafe { sys_apic_timer_set_tsc_deadline(deadline, masked) }
}

/// Configure APIC timer in periodic mode
///
/// # Returns
///
/// `Ok(())` on success, or an error code on failure
pub fn apic_timer_set_periodic(count: u32, divisor: u8) -> RxStatus {
    unsafe { sys_apic_timer_set_periodic(count, divisor) }
}

/// Get the current APIC timer count
pub fn apic_timer_current_count() -> u32 {
    unsafe { sys_apic_timer_current_count() }
}

/// Mask the APIC timer
pub fn apic_timer_mask() {
    unsafe { sys_apic_timer_mask() }
}

/// Unmask the APIC timer
pub fn apic_timer_unmask() {
    unsafe { sys_apic_timer_unmask() }
}

/// Stop the APIC timer
pub fn apic_timer_stop() {
    unsafe { sys_apic_timer_stop() }
}

/// Mask PMI interrupts
pub fn apic_pmi_mask() {
    unsafe { sys_apic_pmi_mask() }
}

/// Unmask PMI interrupts
pub fn apic_pmi_unmask() {
    unsafe { sys_apic_pmi_unmask() }
}

/// APIC error interrupt handler
pub fn apic_error_interrupt_handler() {
    unsafe { sys_apic_error_interrupt_handler() }
}

/// APIC timer interrupt handler
pub fn apic_timer_interrupt_handler() {
    unsafe { sys_apic_timer_interrupt_handler() }
}

/// Platform-specific handler for APIC timer ticks
///
/// This must be implemented by platform code
pub fn platform_handle_apic_timer_tick() {
    unsafe { sys_platform_handle_apic_timer_tick() }
}

/// Initialize IO APICs
pub fn apic_io_init(
    io_apics_descs: &[IoApicDescriptor],
    overrides: &[IoApicIsaOverride],
) {
    unsafe {
        sys_apic_io_init(
            io_apics_descs.as_ptr(),
            io_apics_descs.len() as u32,
            overrides.as_ptr(),
            overrides.len() as u32,
        )
    }
}

/// Check if a global IRQ is valid
pub fn apic_io_is_valid_irq(global_irq: u32) -> bool {
    unsafe { sys_apic_io_is_valid_irq(global_irq) }
}

/// Mask or unmask an IO APIC IRQ
pub fn apic_io_mask_irq(global_irq: u32, mask: bool) {
    unsafe { sys_apic_io_mask_irq(global_irq, mask) }
}

/// Configure an IO APIC IRQ
pub fn apic_io_configure_irq(
    global_irq: u32,
    trig_mode: InterruptTriggerMode,
    polarity: InterruptPolarity,
    del_mode: ApicInterruptDeliveryMode,
    mask: bool,
    dst_mode: ApicInterruptDstMode,
    dst: u8,
    vector: u8,
) {
    unsafe {
        sys_apic_io_configure_irq(
            global_irq,
            trig_mode,
            polarity,
            del_mode,
            mask,
            dst_mode,
            dst,
            vector,
        )
    }
}

/// Fetch IRQ configuration
///
/// # Returns
///
/// `Ok((trigger_mode, polarity))` on success, or an error on failure
pub fn apic_io_fetch_irq_config(
    global_irq: u32,
) -> Result<(InterruptTriggerMode, InterruptPolarity), RxStatus> {
    let mut trig_mode = InterruptTriggerMode::Edge;
    let mut polarity = InterruptPolarity::ActiveHigh;
    
    let status = unsafe {
        sys_apic_io_fetch_irq_config(global_irq, &mut trig_mode, &mut polarity)
    };
    
    if status.is_ok() {
        Ok((trig_mode, polarity))
    } else {
        Err(status)
    }
}

/// Configure an IO APIC IRQ vector
pub fn apic_io_configure_irq_vector(global_irq: u32, vector: u8) {
    unsafe { sys_apic_io_configure_irq_vector(global_irq, vector) }
}

/// Fetch an IO APIC IRQ vector
pub fn apic_io_fetch_irq_vector(global_irq: u32) -> u8 {
    unsafe { sys_apic_io_fetch_irq_vector(global_irq) }
}

/// Mask or unmask an ISA IRQ
pub fn apic_io_mask_isa_irq(isa_irq: u8, mask: bool) {
    unsafe { sys_apic_io_mask_isa_irq(isa_irq, mask) }
}

/// Configure an ISA IRQ
///
/// For ISA configuration, we don't need to specify the trigger mode
/// and polarity since we initialize these to match the ISA bus or
/// any overrides we've been told about.
pub fn apic_io_configure_isa_irq(
    isa_irq: u8,
    del_mode: ApicInterruptDeliveryMode,
    mask: bool,
    dst_mode: ApicInterruptDstMode,
    dst: u8,
    vector: u8,
) {
    unsafe {
        sys_apic_io_configure_isa_irq(
            isa_irq,
            del_mode,
            mask,
            dst_mode,
            dst,
            vector,
        )
    }
}

/// Issue an End-Of-Interrupt to the IO APIC
pub fn apic_io_issue_eoi(global_irq: u32, vec: u8) {
    unsafe { sys_apic_io_issue_eoi(global_irq, vec) }
}

/// Convert an ISA IRQ to a global IRQ number
pub fn apic_io_isa_to_global(isa_irq: u8) -> u32 {
    unsafe { sys_apic_io_isa_to_global(isa_irq) }
}

/// Save IO APIC state
///
/// This function must be invoked with interrupts disabled.
/// It saves the current redirection table entries to memory.
/// It is intended for use with suspend-to-RAM.
pub fn apic_io_save() {
    unsafe { sys_apic_io_save() }
}

/// Restore IO APIC state
///
/// This function must be invoked with interrupts disabled.
/// It restores the redirection table entries from memory.
/// It is intended for use with suspend-to-RAM.
pub fn apic_io_restore() {
    unsafe { sys_apic_io_restore() }
}

/// Print debug information about the local APIC
pub fn apic_local_debug() {
    unsafe { sys_apic_local_debug() }
}

/// Print debug information about the IO APICs
pub fn apic_io_debug() {
    unsafe { sys_apic_io_debug() }
}

// Foreign function declarations for the system implementations
extern "C" {
    fn sys_apic_vm_init();
    fn sys_apic_local_init();
    fn sys_apic_local_id() -> u8;
    fn sys_apic_bsp_id() -> u8;
    fn sys_apic_irq_set(vector: u32, enable: bool);
    fn sys_apic_send_ipi(vector: u8, dst_apic_id: u32, dm: ApicInterruptDeliveryMode);
    fn sys_apic_send_self_ipi(vector: u8, dm: ApicInterruptDeliveryMode);
    fn sys_apic_send_broadcast_ipi(vector: u8, dm: ApicInterruptDeliveryMode);
    fn sys_apic_send_broadcast_self_ipi(vector: u8, dm: ApicInterruptDeliveryMode);
    fn sys_apic_issue_eoi();
    fn sys_apic_timer_set_oneshot(count: u32, divisor: u8, masked: bool) -> RxStatus;
    fn sys_apic_timer_set_tsc_deadline(deadline: u64, masked: bool);
    fn sys_apic_timer_set_periodic(count: u32, divisor: u8) -> RxStatus;
    fn sys_apic_timer_current_count() -> u32;
    fn sys_apic_timer_mask();
    fn sys_apic_timer_unmask();
    fn sys_apic_timer_stop();
    fn sys_apic_pmi_mask();
    fn sys_apic_pmi_unmask();
    fn sys_apic_error_interrupt_handler();
    fn sys_apic_timer_interrupt_handler();
    fn sys_platform_handle_apic_timer_tick();
    fn sys_apic_io_init(
        io_apics_descs: *const IoApicDescriptor,
        num_io_apics: u32,
        overrides: *const IoApicIsaOverride,
        num_overrides: u32,
    );
    fn sys_apic_io_is_valid_irq(global_irq: u32) -> bool;
    fn sys_apic_io_mask_irq(global_irq: u32, mask: bool);
    fn sys_apic_io_configure_irq(
        global_irq: u32,
        trig_mode: InterruptTriggerMode,
        polarity: InterruptPolarity,
        del_mode: ApicInterruptDeliveryMode,
        mask: bool,
        dst_mode: ApicInterruptDstMode,
        dst: u8,
        vector: u8,
    );
    fn sys_apic_io_fetch_irq_config(
        global_irq: u32,
        trig_mode: *mut InterruptTriggerMode,
        polarity: *mut InterruptPolarity,
    ) -> RxStatus;
    fn sys_apic_io_configure_irq_vector(global_irq: u32, vector: u8);
    fn sys_apic_io_fetch_irq_vector(global_irq: u32) -> u8;
    fn sys_apic_io_mask_isa_irq(isa_irq: u8, mask: bool);
    fn sys_apic_io_configure_isa_irq(
        isa_irq: u8,
        del_mode: ApicInterruptDeliveryMode,
        mask: bool,
        dst_mode: ApicInterruptDstMode,
        dst: u8,
        vector: u8,
    );
    fn sys_apic_io_issue_eoi(global_irq: u32, vec: u8);
    fn sys_apic_io_isa_to_global(isa_irq: u8) -> u32;
    fn sys_apic_io_save();
    fn sys_apic_io_restore();
    fn sys_apic_local_debug();
    fn sys_apic_io_debug();
}