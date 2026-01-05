// Copyright 2025 The Rustux Authors
// Copyright (c) 2014 Travis Geiselbrecht
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

// This file contains assembly macros for ARM64 architecture
// It should be included in assembly files that need these macros

#![allow(unused_macros)]

// Assembly macro definitions for ARM64 architecture

/// Moves a literal value into a register, optimizing the number of instructions used
macro_rules! movlit {
    ($reg:expr, $literal:expr) => {
        concat!(
            "mov ", $reg, ", #((", $literal, ") & 0xffff)\n",
            ".ifne ((($literal) >> 16) & 0xffff)\n",
            "movk ", $reg, ", #((($literal) >> 16) & 0xffff), lsl #16\n",
            ".endif\n",
            ".ifne ((($literal) >> 32) & 0xffff)\n",
            "movk ", $reg, ", #((($literal) >> 32) & 0xffff), lsl #32\n",
            ".endif\n",
            ".ifne ((($literal) >> 48) & 0xffff)\n",
            "movk ", $reg, ", #((($literal) >> 48) & 0xffff), lsl #48\n",
            ".endif\n"
        )
    };
}

/// Pushes two registers onto the stack with proper CFI directives
macro_rules! push_regs {
    ($ra:expr, $rb:expr) => {
        concat!(
            "stp ", $ra, ", ", $rb, ", [sp, #-16]!\n",
            ".cfi_adjust_cfa_offset 16\n",
            ".ifnes \"", $ra, "\", \"xzr\"\n",
            ".cfi_rel_offset ", $ra, ", 0\n",
            ".endif\n",
            ".ifnes \"", $rb, "\", \"xzr\"\n",
            ".cfi_rel_offset ", $rb, ", 8\n",
            ".endif\n"
        )
    };
}

/// Pops two registers from the stack with proper CFI directives
macro_rules! pop_regs {
    ($ra:expr, $rb:expr) => {
        concat!(
            "ldp ", $ra, ", ", $rb, ", [sp], #16\n",
            ".cfi_adjust_cfa_offset -16\n",
            ".ifnes \"", $ra, "\", \"xzr\"\n",
            ".cfi_same_value ", $ra, "\n",
            ".endif\n",
            ".ifnes \"", $rb, "\", \"xzr\"\n",
            ".cfi_same_value ", $rb, "\n",
            ".endif\n"
        )
    };
}

/// Subtracts a value from the stack pointer with proper CFI directives
macro_rules! sub_from_sp {
    ($value:expr) => {
        concat!(
            "sub sp, sp, #", $value, "\n",
            ".cfi_adjust_cfa_offset ", $value, "\n"
        )
    };
}

/// Adds a value to the stack pointer with proper CFI directives
macro_rules! add_to_sp {
    ($value:expr) => {
        concat!(
            "add sp, sp, #", $value, "\n",
            ".cfi_adjust_cfa_offset -", $value, "\n"
        )
    };
}

/// Loads a global address into a register using ADRP and ADD
macro_rules! adr_global {
    ($reg:expr, $symbol:expr) => {
        concat!(
            "adrp ", $reg, ", ", $symbol, "\n",
            "add ", $reg, ", ", $reg, ", #:lo12:", $symbol, "\n"
        )
    };
}

/// Loads an absolute address into a register
macro_rules! movabs {
    ($reg:expr, $symbol:expr) => {
        concat!(
            "#ifdef __clang__\n",
            "ldr ", $reg, ", =", $symbol, "\n",
            "#else\n",
            "movz ", $reg, ", #:abs_g0_nc:", $symbol, "\n",
            "movk ", $reg, ", #:abs_g1_nc:", $symbol, "\n",
            "movk ", $reg, ", #:abs_g2_nc:", $symbol, "\n",
            "movk ", $reg, ", #:abs_g3:", $symbol, "\n",
            "#endif\n"
        )
    };
}

/// Test bit and branch if zero with mask
macro_rules! tbzmask {
    ($reg:expr, $mask:expr, $label:expr, $shift:expr) => {
        concat!(
            ".if ", $shift, " >= 64\n",
            "    .error \"tbzmask: unsupported mask, ", $mask, "\"\n",
            ".elseif ", $mask, " == 1 << ", $shift, "\n",
            "    tbz ", $reg, ", #", $shift, ", ", $label, "\n",
            ".else\n",
            "    tbzmask ", $reg, ", ", $mask, ", ", $label, ", \"(", $shift, " + 1)\"\n",
            ".endif\n"
        )
    };
    ($reg:expr, $mask:expr, $label:expr) => {
        tbzmask!($reg, $mask, $label, "0")
    };
}

/// Test bit and branch if not zero with mask
macro_rules! tbnzmask {
    ($reg:expr, $mask:expr, $label:expr, $shift:expr) => {
        concat!(
            ".if ", $shift, " >= 64\n",
            "    .error \"tbnzmask: unsupported mask, ", $mask, "\"\n",
            ".elseif ", $mask, " == 1 << ", $shift, "\n",
            "    tbnz ", $reg, ", #", $shift, ", ", $label, "\n",
            ".else\n",
            "    tbnzmask ", $reg, ", ", $mask, ", ", $label, ", \"(", $shift, " + 1)\"\n",
            ".endif\n"
        )
    };
    ($reg:expr, $mask:expr, $label:expr) => {
        tbnzmask!($reg, $mask, $label, "0")
    };
}

/// Mark all previous frame registers as having the same value
/// For "functions" that are not normal functions in the ABI sense
pub const ALL_CFI_SAME_VALUE: &str = "
    .cfi_same_value x0
    .cfi_same_value x1
    .cfi_same_value x2
    .cfi_same_value x3
    .cfi_same_value x4
    .cfi_same_value x5
    .cfi_same_value x6
    .cfi_same_value x7
    .cfi_same_value x8
    .cfi_same_value x9
    .cfi_same_value x10
    .cfi_same_value x11
    .cfi_same_value x12
    .cfi_same_value x13
    .cfi_same_value x14
    .cfi_same_value x15
    .cfi_same_value x16
    .cfi_same_value x17
    .cfi_same_value x18
    .cfi_same_value x19
    .cfi_same_value x20
    .cfi_same_value x21
    .cfi_same_value x22
    .cfi_same_value x23
    .cfi_same_value x24
    .cfi_same_value x25
    .cfi_same_value x26
    .cfi_same_value x27
    .cfi_same_value x28
    .cfi_same_value x29
";