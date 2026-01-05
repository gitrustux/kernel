// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! RISC-V 64-bit register definitions

/// RISC-V CSR addresses
pub mod csr {
    // Machine-level registers
    pub const MHARTID: usize = 0xF14;
    pub const MSTATUS: usize = 0x300;
    pub const MISA: usize = 0x301;
    pub const MTVEC: usize = 0x305;
    pub const MEPC: usize = 0x341;
    pub const MCAUSE: usize = 0x342;
    pub const MTVAL: usize = 0x343;
    pub const MIP: usize = 0x344;

    // Supervisor-level registers
    pub const SSTATUS: usize = 0x100;
    pub const SIE: usize = 0x104;
    pub const STVEC: usize = 0x105;
    pub const SSCRATCH: usize = 0x140;
    pub const SEPC: usize = 0x141;
    pub const SCAUSE: usize = 0x142;
    pub const STVAL: usize = 0x143;
    pub const SIP: usize = 0x144;
    pub const SATP: usize = 0x180;

    // Timer registers
    pub const STIMECMP: usize = 0x14D;
}

/// sstatus register fields
pub mod sstatus {
    pub const SIE: u64 = 1 << 1;   // Supervisor interrupt enable
    pub const SPIE: u64 = 1 << 5;  // Supervisor previous interrupt enable
    pub const SPP: u64 = 1 << 8;   // Supervisor previous privilege mode
    pub const FS: u64 = 0x3 << 13; // Floating-point status
    pub const XS: u64 = 0x3 << 15; // Extension status
    pub const SUM: u64 = 1 << 18;  // Supervisor user memory access
    pub const MXR: u64 = 1 << 19;  // Make executable readable
}

/// scause register exception codes
pub mod scause {
    pub const INSTRUCTION_ADDRESS_MISALIGNED: u64 = 0;
    pub const INSTRUCTION_ACCESS_FAULT: u64 = 1;
    pub const ILLEGAL_INSTRUCTION: u64 = 2;
    pub const BREAKPOINT: u64 = 3;
    pub const LOAD_ADDRESS_MISALIGNED: u64 = 4;
    pub const LOAD_ACCESS_FAULT: u64 = 5;
    pub const STORE_AMO_ADDRESS_MISALIGNED: u64 = 6;
    pub const STORE_AMO_ACCESS_FAULT: u64 = 7;
    pub const ENV_CALL_FROM_U_MODE: u64 = 8;
    pub const ENV_CALL_FROM_S_MODE: u64 = 9;
    pub const INSTRUCTION_PAGE_FAULT: u64 = 12;
    pub const LOAD_PAGE_FAULT: u64 = 13;
    pub const STORE_AMO_PAGE_FAULT: u64 = 15;

    pub const INTERRUPT_BIT: u64 = 1 << 63;

    pub const SUPERVISOR_SOFTWARE_INTERRUPT: u64 = INTERRUPT_BIT | 1;
    pub const SUPERVISOR_TIMER_INTERRUPT: u64 = INTERRUPT_BIT | 5;
    pub const SUPERVISOR_EXTERNAL_INTERRUPT: u64 = INTERRUPT_BIT | 9;
}

/// satp register fields (Sv39/Sv48)
pub mod satp {
    pub const MODE_OFF: u64 = 0 << 60;
    pub const MODE_SV39: u64 = 8 << 60;
    pub const MODE_SV48: u64 = 9 << 60;
    pub const MODE_SV57: u64 = 10 << 60;
    pub const PPN_SHIFT: u64 = 12;
    pub const ASID_SHIFT: u64 = 44;
    pub const ASID_BITS: u64 = 16;
}
