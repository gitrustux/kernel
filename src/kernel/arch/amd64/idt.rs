// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86-64 Interrupt Descriptor Table (IDT)

#![no_std]

/// IDT entry structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct IdtEntry {
    pub offset_low: u16,
    pub selector: u16,
    pub ist: u8,
    pub type_attr: u8,
    pub offset_mid: u16,
    pub offset_high: u32,
    pub reserved: u32,
}

/// IDT pointer structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct IdtPointer {
    pub limit: u16,
    pub base: u64,
}

/// Task State Segment (TSS) placeholder
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Tss {
    pub data: [u8; 0],
}

/// Initialize the IDT
pub fn idt_init() {
    // TODO: Implement IDT initialization
}
