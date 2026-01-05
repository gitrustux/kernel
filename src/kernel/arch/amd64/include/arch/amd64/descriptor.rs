// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 GDT and TSS descriptor management
//!
//! This module provides functionality for managing x86 GDT (Global Descriptor Table)
//! and TSS (Task State Segment) descriptors.

use crate::arch::aspace::*;
use crate::arch::amd64::ioport::IoBitmap;
use crate::rustux::types::*;

/// Null selector
pub const NULL_SELECTOR: u16 = 0x00;

/// Kernel code selector
pub const CODE_SELECTOR: u16 = 0x08;
/// Kernel 64-bit code selector
pub const CODE_64_SELECTOR: u16 = 0x10;
/// Kernel data selector
pub const DATA_SELECTOR: u16 = 0x18;

/// User code selector (ring 3)
pub const USER_CODE_SELECTOR: u16 = 0x20 | 3;
/// User data selector (ring 3)
pub const USER_DATA_SELECTOR: u16 = 0x28 | 3;
/// User 64-bit code selector (ring 3)
pub const USER_CODE_64_SELECTOR: u16 = 0x30 | 3;

/// TSS selector for a given CPU index
pub fn tss_selector(i: usize) -> u16 {
    (0x38 + 16 * i) as u16
}
// Note: 0x40 is used by the second half of the first TSS descriptor

/// Extract privilege level from a selector
pub fn selector_pl(s: u16) -> u16 {
    s & 0x3
}

/// TSS segment type
pub const SEG_TYPE_TSS: u8 = 0x9;
/// Busy TSS segment type
pub const SEG_TYPE_TSS_BUSY: u8 = 0xb;
/// Task gate segment type
pub const SEG_TYPE_TASK_GATE: u8 = 0x5;
/// 32-bit interrupt gate segment type
pub const SEG_TYPE_INT_GATE: u8 = 0xe;
/// Data read/write segment type
pub const SEG_TYPE_DATA_RW: u8 = 0x2;
/// Code read/write segment type
pub const SEG_TYPE_CODE_RW: u8 = 0xa;

/// Segment selector type
pub type SegSel = u16;

/// Fill in a descriptor in the GDT
///
/// # Arguments
///
/// * `sel` - Selector value
/// * `base` - Base address
/// * `limit` - Limit value
/// * `present` - Present flag
/// * `ring` - Privilege ring level
/// * `sys` - System flag
/// * `type_` - Type value
/// * `gran` - Granularity
/// * `bits` - Bit size flag
///
/// # Safety
///
/// This function is unsafe because it modifies the GDT directly.
pub unsafe fn set_global_desc_64(
    sel: SegSel,
    base: u64,
    limit: u32,
    present: u8,
    ring: u8,
    sys: u8,
    type_: u8,
    gran: u8,
    bits: u8,
) {
    sys_set_global_desc_64(sel, base, limit, present, ring, sys, type_, gran, bits);
}

/// Initialize the per-CPU TSS
///
/// # Safety
///
/// This function is unsafe because it modifies CPU state.
pub unsafe fn x86_initialize_percpu_tss() {
    sys_x86_initialize_percpu_tss();
}

/// Set the TSS stack pointer
///
/// # Arguments
///
/// * `sp` - Stack pointer value
///
/// # Safety
///
/// This function is unsafe because it modifies the TSS directly.
pub unsafe fn x86_set_tss_sp(sp: VAddr) {
    sys_x86_set_tss_sp(sp);
}

/// Clear the busy flag in a TSS descriptor
///
/// # Arguments
///
/// * `sel` - Selector value
///
/// # Safety
///
/// This function is unsafe because it modifies the GDT directly.
pub unsafe fn x86_clear_tss_busy(sel: SegSel) {
    sys_x86_clear_tss_busy(sel);
}

/// Set the TSS IO bitmap
///
/// # Arguments
///
/// * `bitmap` - The IO bitmap to set
///
/// # Safety
///
/// This function is unsafe because it modifies the TSS directly.
pub unsafe fn x86_set_tss_io_bitmap(bitmap: &mut IoBitmap) {
    sys_x86_set_tss_io_bitmap(bitmap);
}

/// Clear the TSS IO bitmap
///
/// # Arguments
///
/// * `bitmap` - The IO bitmap to clear
///
/// # Safety
///
/// This function is unsafe because it modifies the TSS directly.
pub unsafe fn x86_clear_tss_io_bitmap(bitmap: &mut IoBitmap) {
    sys_x86_clear_tss_io_bitmap(bitmap);
}

/// Reset the TSS IO bitmap to default state
///
/// # Safety
///
/// This function is unsafe because it modifies the TSS directly.
pub unsafe fn x86_reset_tss_io_bitmap() {
    sys_x86_reset_tss_io_bitmap();
}

/// Load the GDT
///
/// # Arguments
///
/// * `base` - Base address of the GDT
///
/// # Safety
///
/// This function is unsafe because it modifies CPU state.
#[inline]
pub unsafe fn gdt_load(base: usize) {
    // During VM exit GDTR limit is always set to 0xffff and instead of
    // trying to maintain the limit aligned with the actual GDT size we
    // decided to just keep it 0xffff all the time and instead of relying
    // on the limit just map GDT in the way that accesses beyond GDT cause
    // page faults. This allows us to avoid calling LGDT on every VM exit.
    let gdtr = Gdtr {
        limit: 0xffff,
        address: base,
    };
    
    x86_lgdt(&gdtr as *const Gdtr as usize);
}

/// Setup the GDT
///
/// # Safety
///
/// This function is unsafe because it modifies CPU state.
pub unsafe fn gdt_setup() {
    sys_gdt_setup();
}

/// Get the GDT base address
///
/// # Returns
///
/// The base address of the GDT
///
/// # Safety
///
/// This function is unsafe because it accesses CPU state.
pub unsafe fn gdt_get() -> usize {
    sys_gdt_get()
}

/// GDTR structure for loading the GDT
#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    address: usize,
}

// FFI declarations for the system functions
extern "C" {
    fn sys_set_global_desc_64(
        sel: SegSel,
        base: u64,
        limit: u32,
        present: u8,
        ring: u8,
        sys: u8,
        type_: u8,
        gran: u8,
        bits: u8,
    );
    
    fn sys_x86_initialize_percpu_tss();
    fn sys_x86_set_tss_sp(sp: VAddr);
    fn sys_x86_clear_tss_busy(sel: SegSel);
    fn sys_x86_set_tss_io_bitmap(bitmap: &mut IoBitmap);
    fn sys_x86_clear_tss_io_bitmap(bitmap: &mut IoBitmap);
    fn sys_x86_reset_tss_io_bitmap();
    fn sys_gdt_setup();
    fn sys_gdt_get() -> usize;
    fn x86_lgdt(gdtr: usize);
}