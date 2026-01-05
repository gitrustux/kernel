// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Interrupt Descriptor Table (IDT) definitions and management
//!
//! This module provides types and functions for managing the x86_64 Interrupt
//! Descriptor Table, which is used to specify handler functions for different
//! types of interrupts and exceptions.

use crate::rustux::compiler::*;
use core::mem::size_of;

/// A single entry in the Interrupt Descriptor Table
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IdtEntry {
    /// First 32 bits of the entry
    pub w0: u32,
    /// Second 32 bits of the entry
    pub w1: u32,
    /// Third 32 bits of the entry
    pub w2: u32,
    /// Fourth 32 bits of the entry
    pub w3: u32,
}

/// The complete Interrupt Descriptor Table
#[repr(C, align(16))]
#[derive(Debug, Clone)]
pub struct Idt {
    /// Array of 256 IDT entries
    pub entries: [IdtEntry; 256],
}

/// The IDT Register (IDTR) structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Idtr {
    /// Size of the IDT minus 1
    pub limit: u16,
    /// Base address of the IDT
    pub address: usize,
}

/// Interrupt gate types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdtEntryType {
    /// 64-bit interrupt gate
    InterruptGate64 = 0xe,
    /// 64-bit trap gate
    TrapGate64 = 0xf,
}

/// Descriptor Privilege Levels
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdtDpl {
    /// Kernel privilege level (ring 0)
    Dpl0 = 0,
    /// Ring 1 privilege level
    Dpl1 = 1,
    /// Ring 2 privilege level
    Dpl2 = 2,
    /// User privilege level (ring 3)
    Dpl3 = 3,
}

// Ensure our structures have the expected sizes
const _: () = assert!(size_of::<IdtEntry>() == 16);
const _: () = assert!(size_of::<Idt>() == 16 * 256);

impl Idt {
    /// Create a new IDT with all entries zeroed
    #[inline]
    pub const fn new() -> Self {
        const ZERO_ENTRY: IdtEntry = IdtEntry { w0: 0, w1: 0, w2: 0, w3: 0 };
        Self {
            entries: [ZERO_ENTRY; 256],
        }
    }

    /// Change an IDT entry
    ///
    /// # Safety
    ///
    /// Caution: Interrupts should probably be disabled when this is called
    ///
    /// # Arguments
    ///
    /// * `vec` - The vector to replace
    /// * `code_segment_sel` - The code segment selector to use on taking this interrupt
    /// * `entry_point_offset` - The offset of the code to begin executing (relative to the segment)
    /// * `dpl` - The desired privilege level of the handler
    /// * `typ` - The type of interrupt handler
    pub unsafe fn set_vector(
        &mut self,
        vec: u8,
        code_segment_sel: u16,
        entry_point_offset: usize,
        dpl: IdtDpl,
        typ: IdtEntryType,
    ) {
        sys_idt_set_vector(
            self as *mut Idt,
            vec,
            code_segment_sel,
            entry_point_offset,
            dpl,
            typ,
        );
    }

    /// Set the Interrupt Stack Table index to use
    ///
    /// # Safety
    ///
    /// Caution: Interrupts should probably be disabled when this is called
    ///
    /// # Arguments
    ///
    /// * `vec` - The vector to change
    /// * `ist_idx` - A value in the range [0, 8) indicating which stack to use.
    ///               If ist_idx == 0, use the normal stack for the target privilege level.
    pub unsafe fn set_ist_index(&mut self, vec: u8, ist_idx: u8) {
        debug_assert!(ist_idx < 8, "IST index must be less than 8");
        sys_idt_set_ist_index(self as *mut Idt, vec, ist_idx);
    }

    /// Initialize this IDT with default values
    ///
    /// # Safety
    ///
    /// This function initializes critical system structures and should only be
    /// called during system initialization.
    pub unsafe fn setup(&mut self) {
        sys_idt_setup(self as *mut Idt);
    }

    /// Load this IDT into the CPU
    ///
    /// # Safety
    ///
    /// This function modifies CPU state directly and should be used with caution.
    /// It should typically only be called during system initialization or when
    /// switching between execution contexts.
    #[inline]
    pub unsafe fn load(&self) {
        // After VM exit IDT limit is always set to 0xffff, so in order to avoid
        // calling LIDT in hypervisor to restore the proper IDT limit after every
        // VM exit in hypervisor we decided to use 0xffff all the time. There is
        // no harm in doing that because IDT limit is only relevant if it's smaller
        // than sizeof(struct idt) - 1 and doesn't affect anything otherwise.
        let idtr = Idtr {
            limit: 0xffff,
            address: self as *const Idt as usize,
        };
        x86_lidt(&idtr as *const Idtr as usize);
    }
}

/// Setup the read-only remapping of the IDT
///
/// # Safety
///
/// This function initializes critical system structures and should only be
/// called during system initialization.
pub unsafe fn idt_setup_readonly() {
    sys_idt_setup_readonly();
}

/// Get the read-only IDT
///
/// # Returns
///
/// A reference to the read-only IDT
///
/// # Safety
///
/// This function accesses system state directly and should be used with caution.
pub unsafe fn idt_get_readonly() -> &'static Idt {
    &*sys_idt_get_readonly()
}

// External functions defined in the system
extern "C" {
    fn sys_idt_set_vector(
        idt: *mut Idt,
        vec: u8,
        code_segment_sel: u16,
        entry_point_offset: usize,
        dpl: IdtDpl,
        typ: IdtEntryType,
    );
    
    fn sys_idt_set_ist_index(
        idt: *mut Idt,
        vec: u8,
        ist_idx: u8,
    );
    
    fn sys_idt_setup(idt: *mut Idt);
    fn sys_idt_setup_readonly();
    fn sys_idt_get_readonly() -> *const Idt;
    fn x86_lidt(idtr: usize);
}