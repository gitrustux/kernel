// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit Memory Management Unit
//!
//! Supports both Sv39 and Sv48 page table formats.

use crate::arch::riscv64;
use crate::arch::riscv64::registers;
use crate::rustux::types::*;

/// Page table entry flags for RISC-V
pub mod pte_flags {
    pub const VALID: u64 = 1 << 0;
    pub const READ: u64 = 1 << 1;
    pub const WRITE: u64 = 1 << 2;
    pub const EXECUTE: u64 = 1 << 3;
    pub const USER: u64 = 1 << 4;
    pub const GLOBAL: u64 = 1 << 5;
    pub const ACCESSED: u64 = 1 << 6;
    pub const DIRTY: u64 = 1 << 7;
    pub const RSW: u64 = 0x3 << 8;      // Reserved for software
    pub const PBMT: u64 = 0x3 << 59;     // Physical memory attributes
}

/// RISC-V Sv39/Sv48 Page Table Entry
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PageTableEntry {
    pub entry: u64,
}

impl PageTableEntry {
    pub const fn new() -> Self {
        Self { entry: 0 }
    }

    pub fn is_valid(&self) -> bool {
        self.entry & pte_flags::VALID != 0
    }

    pub fn is_leaf(&self) -> bool {
        self.entry & (pte_flags::READ | pte_flags::EXECUTE) != 0
    }

    pub fn ppn(&self) -> u64 {
        self.entry >> 10
    }

    pub fn set_ppn(&mut self, ppn: u64) {
        self.entry = (self.entry & 0x3FF) | (ppn << 10);
    }

    pub fn set_flags(&mut self, flags: u64) {
        self.entry |= flags;
    }
}

/// Address space identifiers (ASID)
pub type Asid = u16;

/// SATP register value
#[repr(C)]
pub struct SatpValue {
    pub mode: u8,
    pub asid: Asid,
    pub ppn: u64,
}

impl SatpValue {
    pub const fn new_sv39(asid: Asid, ppn: u64) -> Self {
        Self {
            mode: 8,  // Sv39
            asid,
            ppn,
        }
    }

    pub const fn new_sv48(asid: Asid, ppn: u64) -> Self {
        Self {
            mode: 9,  // Sv48
            asid,
            ppn,
        }
    }

    pub fn to_u64(&self) -> u64 {
        ((self.mode as u64) << 60) |
        ((self.asid as u64) << 32) |
        (self.ppn & 0xFFF_FFFF_FFFF)
    }

    pub fn from_u64(val: u64) -> Self {
        Self {
            mode: (val >> 60) as u8,
            asid: ((val >> 32) & 0xFFFF) as Asid,
            ppn: val & 0xFFF_FFFF_FFFF,
        }
    }
}

/// Flush the entire TLB
#[inline(always)]
pub fn tlb_flush() {
    unsafe {
        core::arch::asm!("sfence.vma", options(nostack));
    }
}

/// Flush TLB entries for a specific ASID
#[inline(always)]
pub fn tlb_flush_asid(asid: Asid) {
    unsafe {
        core::arch::asm!("sfence.vma zero, {asid}",
            asid = in(reg) asid,
            // clobber register
            out(reg) _,
        );
    }
}

/// Flush a specific TLB entry
#[inline(always)]
pub fn tlb_flush_page(vaddr: usize) {
    unsafe {
        core::arch::asm!("sfence.vma {vaddr}",
            vaddr = in(reg) vaddr,
            options(nostack),
        );
    }
}

/// Set the SATP register (switch address space)
#[inline(always)]
pub fn set_satp(satp: u64) {
    unsafe {
        core::arch::asm!("csrw satp, {satp}", satp = in(reg) satp);
        core::arch::asm!("sfence.vma");  // Flush TLB after SATP change
    }
}

/// Get the current SATP register value
#[inline(always)]
pub fn get_satp() -> u64 {
    let satp: u64;
    unsafe {
        core::arch::asm!("csrr {satp}, satp", satp = out(reg) satp);
    }
    satp
}

/// Enable paging
pub fn enable_paging() {
    // Set SATP to enable Sv39/Sv48 paging
    // The actual root page table PPN should be set by the caller
    tlb_flush();
}
