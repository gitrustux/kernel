// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 interrupt vector definitions
//!
//! This module defines the standard x86 interrupt vectors used by the system,
//! including both processor-defined exceptions and platform-specific interrupts.

/// x86 interrupt vector numbers
///
/// This enum defines all the interrupt vectors used in the x86 architecture,
/// including processor exceptions, platform interrupts, and local APIC interrupts.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum X86InterruptVector {
    /// Divide by zero exception
    DivideBy0 = 0,
    /// Debug exception
    Debug = 1,
    /// Non-maskable interrupt
    Nmi = 2,
    /// Breakpoint exception
    Breakpoint = 3,
    /// Overflow exception
    Overflow = 4,
    /// Bound range exceeded exception
    BoundRange = 5,
    /// Invalid opcode exception
    InvalidOp = 6,
    /// Device not available exception
    DeviceNa = 7,
    /// Double fault exception
    DoubleFault = 8,
    /// Invalid TSS exception
    InvalidTss = 0xa,
    /// Segment not present exception
    SegmentNotPresent = 0xb,
    /// Stack fault exception
    StackFault = 0xc,
    /// General protection fault
    GpFault = 0xd,
    /// Page fault exception
    PageFault = 0xe,
    /// Reserved vector
    Reserved = 0xf,
    /// x87 FPU floating-point error
    FpuFpError = 0x10,
    /// Alignment check exception
    AlignmentCheck = 0x11,
    /// Machine check exception
    MachineCheck = 0x12,
    /// SIMD floating-point exception
    SimdFpError = 0x13,
    /// Virtualization exception
    Virt = 0x14,
    /// Maximum Intel-defined exception
    MaxIntelDefined = 0x1f,

    /// Base for platform-specific interrupts
    PlatformBase = 0x20,
    /// Maximum platform-specific interrupt
    PlatformMax = 0xef,

    /// Base for local APIC interrupts
    LocalApicBase = 0xf0,
    /// APIC spurious interrupt
    ApicSpurious = 0xf0,
    /// APIC timer interrupt
    ApicTimer = 0xf1,
    /// APIC error interrupt
    ApicError = 0xf2,
    /// APIC PMI (Performance Monitoring Interrupt)
    ApicPmi = 0xf3,
    /// Inter-processor interrupt: generic
    IpiGeneric = 0xf4,
    /// Inter-processor interrupt: reschedule
    IpiReschedule = 0xf5,
    /// Inter-processor interrupt: regular interrupt
    IpiInterrupt = 0xf6,
    /// Inter-processor interrupt: halt
    IpiHalt = 0xf7,

    /// Maximum interrupt vector
    Max = 0xff,
}

/// Total number of interrupt vectors
pub const X86_INT_COUNT: usize = 0x100;

impl X86InterruptVector {
    /// Convert a raw vector number to an X86InterruptVector
    ///
    /// # Arguments
    ///
    /// * `vector` - The raw interrupt vector number
    ///
    /// # Returns
    ///
    /// The corresponding X86InterruptVector, or None if the vector is invalid
    pub fn from_raw(vector: u8) -> Option<Self> {
        if vector <= Self::Max as u8 {
            // This is safe because we're verifying the range
            Some(unsafe { core::mem::transmute(vector) })
        } else {
            None
        }
    }

    /// Convert to the raw vector number
    pub fn as_raw(self) -> u8 {
        self as u8
    }

    /// Check if this is a processor exception vector
    pub fn is_exception(self) -> bool {
        self as u8 <= Self::MaxIntelDefined as u8
    }

    /// Check if this is a platform-specific interrupt vector
    pub fn is_platform_interrupt(self) -> bool {
        let raw = self as u8;
        raw >= Self::PlatformBase as u8 && raw <= Self::PlatformMax as u8
    }

    /// Check if this is a local APIC interrupt vector
    pub fn is_apic_interrupt(self) -> bool {
        let raw = self as u8;
        raw >= Self::LocalApicBase as u8
    }

    /// Check if this is an inter-processor interrupt (IPI)
    pub fn is_ipi(self) -> bool {
        let raw = self as u8;
        raw >= Self::IpiGeneric as u8 && raw <= Self::IpiHalt as u8
    }
}