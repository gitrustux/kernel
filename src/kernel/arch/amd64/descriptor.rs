// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 Descriptor Tables
//!
//! This module provides GDT and IDT setup functions.


/// Setup the GDT (Global Descriptor Table)
pub fn gdt_setup() {
    // TODO: Implement GDT setup
}

/// Setup the IDT (Interrupt Descriptor Table)
pub fn idt_setup_readonly() {
    // TODO: Implement IDT setup
}

/// Extract the Requested Privilege Level (RPL) from a selector
///
/// # Arguments
///
/// * `selector` - Segment selector value
///
/// # Returns
///
/// The RPL value (0-3)
pub const fn SELECTOR_PL(selector: u16) -> u16 {
    selector & 3
}

/// Make a selector from RPL
///
/// # Arguments
///
/// * `rpl` - Requested Privilege Level (0-3)
///
/// # Returns
///
/// A selector value with the specified RPL
pub const fn SELECTOR_FROM_RPL(rpl: u16) -> u16 {
    rpl & 3
}
