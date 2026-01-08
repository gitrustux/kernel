// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Kernel Virtual Address Layout
//!
//! This module defines the virtual memory address space layout for the Rustux kernel.
//! It provides consistent semantics across ARM64, AMD64, and RISC-V architectures.
//!
//! # Design Principles
//!
//! 1. **User space in low half, kernel in high half** - Consistent split across architectures
//! 2. **Canonical addressing** - All VAs must be canonical for their architecture
//! 3. **Fixed regions** - Kernel regions have predictable offsets from KERNEL_BASE
//! 4. **Guard pages** - Stack and critical regions have guard pages for overflow detection
//!
//! # Architecture Differences
//!
//! ## ARM64 (AArch64)
//! - 48-bit virtual address space (256 TB user + 256 TB kernel)
//! - TTBR0_EL1 for user, TTBR1_EL1 for kernel
//! - Page sizes: 4KB, 16KB, 64KB
//!
//! ## AMD64 (x86-64)
//! - 48-bit virtual address space (canonical addressing required)
//! - CR3 register points to single page table for both user and kernel
//! - Page sizes: 4KB, 2MB, 1GB
//!
//! ## RISC-V (RV64GC)
//! - Sv39: 39-bit VA (512 GB user + 512 GB kernel)
//! - Sv48: 48-bit VA (256 TB user + 256 TB kernel)
//! - Page sizes: 4KB, 2MB, 1GB

#![no_std]

use core::fmt;

/// Virtual address type
pub type VAddr = usize;

/// Physical address type
pub type PAddr = usize;

/// Page size (4KB is standard across all architectures)
pub const PAGE_SIZE: usize = 4096;

/// Page size shift for quick division/multiplication
pub const PAGE_SIZE_SHIFT: u8 = 12;

/// Mask for page-aligned addresses
pub const PAGE_MASK: usize = PAGE_SIZE - 1;

/// ============================================================================
/// Architecture-Agnostic Layout Constants
/// ============================================================================

/// User virtual address space size (48-bit = 256 TB)
pub const USER_ASPACE_SIZE: usize = 1 << 48;

/// Kernel virtual address space size
pub const KERNEL_ASPACE_SIZE: usize = 1 << 48;

/// Maximum user address (exclusive)
pub const USER_MAX_VADDR: VAddr = USER_ASPACE_SIZE - 1;

/// Base of user space
pub const USER_BASE: VAddr = 0x1_0000;

/// Stack guard page size
pub const STACK_GUARD_SIZE: usize = PAGE_SIZE;

/// Default stack size for kernel threads
pub const KERNEL_STACK_SIZE: usize = 16 * PAGE_SIZE; // 64KB

/// ============================================================================
/// ARM64 Virtual Address Layout
/// ============================================================================

#[cfg(target_arch = "aarch64")]
pub mod arm64 {
    use super::{VAddr, PAddr, PAGE_SIZE};

    /// Kernel base address (top half of 48-bit VA space)
    pub const KERNEL_BASE: VAddr = 0xFFFF_0000_0000_0000;

    /// End of kernel address space
    pub const KERNEL_END: VAddr = 0xFFFF_FFFF_FFFF_FFFF;

    /// Size of kernel address space
    pub const KERNEL_SIZE: usize = 256 * 1024 * 1024 * 1024 * 1024; // 256 TB

    /// Kernel code/text segment
    pub const KERNEL_TEXT_BASE: VAddr = KERNEL_BASE + 0x0000_0000;
    pub const KERNEL_TEXT_SIZE: usize = 256 * 1024 * 1024; // 256 MB

    /// Kernel data segment (RW)
    pub const KERNEL_DATA_BASE: VAddr = KERNEL_BASE + 0x1000_0000;
    pub const KERNEL_DATA_SIZE: usize = 256 * 1024 * 1024; // 256 MB

    /// Per-CPU data area
    pub const KERNEL_PERCPU_BASE: VAddr = KERNEL_BASE + 0x2000_0000;
    pub const KERNEL_PERCPU_SIZE: usize = 64 * 1024; // 64KB per CPU, max 256 CPUs

    /// Physical memory direct map window
    /// Maps physical memory 1:1 into kernel VA space
    pub const KERNEL_PHYSMAP_BASE: VAddr = 0xFFFF_FFFF_F000_0000;
    pub const KERNEL_PHYSMAP_SIZE: usize = 256 * 1024 * 1024 * 1024; // 256 GB

    /// Device MMIO region
    pub const KERNEL_MMIO_BASE: VAddr = 0xFFFF_FFFF_F000_0000;
    pub const KERNEL_MMIO_SIZE: usize = 64 * 1024 * 1024 * 1024; // 64 GB

    /// Heap region
    pub const KERNEL_HEAP_BASE: VAddr = KERNEL_BASE + 0x3000_0000;
    pub const KERNEL_HEAP_SIZE: usize = 1024 * 1024 * 1024; // 1 GB

    /// User address space (lower half)
    pub const USER_BASE: VAddr = 0x0000_0000_0000_0000;
    pub const USER_MAX: VAddr = 0x0000_FFFF_FFFF_FFFF;

    /// Stack location for user processes (top of user space)
    pub const USER_STACK_TOP: VAddr = 0x0000_FFFF_FFFF_F000;

    /// Page table entry bits
    pub const PTE_VALID: u64 = 1 << 0;
    pub const PTE_TABLE: u64 = 1 << 1;
    pub const PTE_BLOCK: u64 = 0 << 1;
    pub const PTE_USER: u64 = 1 << 6;      // EL0 access
    pub const PTE_RW: u64 = 1 << 7;        // Read/Write
    pub const PTE_XN: u64 = 1 << 54;       // Execute Never
    pub const PTE_AF: u64 = 1 << 10;       // Access Flag
    pub const PTE_SH: u64 = 3 << 8;        // Inner Shareable
    pub const PTE_WB: u64 = 0 << 2;        // Write-Back

    /// Check if a virtual address is in kernel space
    #[inline]
    pub const fn is_kernel_vaddr(vaddr: VAddr) -> bool {
        vaddr >= KERNEL_BASE
    }

    /// Check if a virtual address is in user space
    #[inline]
    pub const fn is_user_vaddr(vaddr: VAddr) -> bool {
        vaddr < KERNEL_BASE
    }

    /// Check if a virtual address is canonical
    #[inline]
    pub const fn is_canonical(vaddr: VAddr) -> bool {
        // For ARM64 48-bit: bits [63:48] must be all 0 or all 1
        let top_bits = vaddr >> 48;
        top_bits == 0 || top_bits == 0xFFFF
    }

    /// MMU ASID constants
    pub const MMU_ARM64_ASID_BITS: u32 = 16;
    pub const MMU_ARM64_GLOBAL_ASID: u32 = 0;
    pub const MMU_ARM64_MAX_USER_ASID: u16 = (1 << 16) - 2;
}

/// ============================================================================
/// AMD64 (x86-64) Virtual Address Layout
/// ============================================================================

#[cfg(target_arch = "x86_64")]
pub mod amd64 {
    use super::{VAddr, PAddr, PAGE_SIZE};

    /// Kernel base address (canonical upper half)
    pub const KERNEL_BASE: VAddr = 0xFFFF_8000_0000_0000;

    /// End of kernel address space
    pub const KERNEL_END: VAddr = 0xFFFF_FFFF_FFFF_FFFF;

    /// Size of kernel address space
    pub const KERNEL_SIZE: usize = 256 * 1024 * 1024 * 1024 * 1024; // 256 TB

    /// Kernel code/text segment
    pub const KERNEL_TEXT_BASE: VAddr = KERNEL_BASE + 0x0010_0000;
    pub const KERNEL_TEXT_SIZE: usize = 256 * 1024 * 1024; // 256 MB

    /// Kernel data segment (RW)
    pub const KERNEL_DATA_BASE: VAddr = KERNEL_BASE + 0x0200_0000;
    pub const KERNEL_DATA_SIZE: usize = 256 * 1024 * 1024; // 256 MB

    /// Per-CPU data area
    pub const KERNEL_PERCPU_BASE: VAddr = KERNEL_BASE + 0x0400_0000;
    pub const KERNEL_PERCPU_SIZE: usize = 64 * 1024; // 64KB per CPU

    /// Physical memory direct map window
    pub const KERNEL_PHYSMAP_BASE: VAddr = 0xFFFF_8800_0000_0000;
    pub const KERNEL_PHYSMAP_SIZE: usize = 64 * 1024 * 1024 * 1024; // 64 GB

    /// Device MMIO region
    pub const KERNEL_MMIO_BASE: VAddr = 0xFFFF_F000_0000_0000;
    pub const KERNEL_MMIO_SIZE: usize = 64 * 1024 * 1024 * 1024; // 64 GB

    /// Heap region
    pub const KERNEL_HEAP_BASE: VAddr = KERNEL_BASE + 0x0500_0000;
    pub const KERNEL_HEAP_SIZE: usize = 1024 * 1024 * 1024; // 1 GB

    /// User address space (lower half)
    pub const USER_BASE: VAddr = 0x0000_0000_0000_0000;
    pub const USER_MAX: VAddr = 0x0000_7FFF_FFFF_FFFF;

    /// Stack location for user processes (top of user space)
    pub const USER_STACK_TOP: VAddr = 0x0000_7FFF_FFFF_F000;

    /// Canonical address mask
    pub const CANONICAL_MASK: u64 = 0xFFFF_F800_0000_0000;

    /// Page table entry bits
    pub const PTE_PRESENT: u64 = 1 << 0;
    pub const PTE_WRITE: u64 = 1 << 1;
    pub const PTE_USER: u64 = 1 << 2;
    pub const PTE_WRITETHROUGH: u64 = 1 << 3;
    pub const PTE_NOCACHE: u64 = 1 << 4;
    pub const PTE_ACCESSED: u64 = 1 << 5;
    pub const PTE_DIRTY: u64 = 1 << 6;
    pub const PTE_GLOBAL: u64 = 1 << 8;
    pub const PTE_NX: u64 = 1u64 << 63;

    /// Check if a virtual address is in kernel space
    #[inline]
    pub const fn is_kernel_vaddr(vaddr: VAddr) -> bool {
        (vaddr as u64) >= KERNEL_BASE as u64
    }

    /// Check if a virtual address is in user space
    #[inline]
    pub const fn is_user_vaddr(vaddr: VAddr) -> bool {
        (vaddr as u64) < KERNEL_BASE as u64
    }

    /// Check if a virtual address is canonical
    #[inline]
    pub const fn is_canonical(vaddr: VAddr) -> bool {
        // For x86-64 48-bit: bits [63:48] must be sign-extended from bit 47
        let vaddr_u64 = vaddr as u64;
        let sign_extended = (vaddr_u64 & CANONICAL_MASK) == 0 || (vaddr_u64 & CANONICAL_MASK) == CANONICAL_MASK;
        sign_extended
    }
}

/// ============================================================================
/// RISC-V Virtual Address Layout
/// ============================================================================

#[cfg(target_arch = "riscv64")]
pub mod riscv {
    use super::{VAddr, PAddr, PAGE_SIZE};

    /// Sv39: 39-bit virtual address space
    pub const SV39_VA_BITS: u8 = 39;

    /// Sv48: 48-bit virtual address space
    pub const SV48_VA_BITS: u8 = 48;

    /// Kernel base address (using Sv39 for now)
    pub const KERNEL_BASE: VAddr = 0xFFFF_FFFF_C000_0000;

    /// End of kernel address space
    pub const KERNEL_END: VAddr = 0xFFFF_FFFF_FFFF_FFFF;

    /// Size of kernel address space (512 GB for Sv39)
    pub const KERNEL_SIZE: usize = 512 * 1024 * 1024 * 1024;

    /// Kernel code/text segment
    pub const KERNEL_TEXT_BASE: VAddr = KERNEL_BASE + 0x0000_0000;
    pub const KERNEL_TEXT_SIZE: usize = 256 * 1024 * 1024; // 256 MB

    /// Kernel data segment (RW)
    pub const KERNEL_DATA_BASE: VAddr = KERNEL_BASE + 0x1000_0000;
    pub const KERNEL_DATA_SIZE: usize = 256 * 1024 * 1024; // 256 MB

    /// Per-CPU data area
    pub const KERNEL_PERCPU_BASE: VAddr = KERNEL_BASE + 0x2000_0000;
    pub const KERNEL_PERCPU_SIZE: usize = 64 * 1024; // 64KB per CPU

    /// Physical memory direct map window
    pub const KERNEL_PHYSMAP_BASE: VAddr = 0xFFFF_FFFF_C000_0000;
    pub const KERNEL_PHYSMAP_SIZE: usize = 16 * 1024 * 1024 * 1024; // 16 GB

    /// Device MMIO region
    pub const KERNEL_MMIO_BASE: VAddr = 0xFFFF_FFFF_F000_0000;
    pub const KERNEL_MMIO_SIZE: usize = 64 * 1024 * 1024 * 1024; // 64 GB

    /// Heap region
    pub const KERNEL_HEAP_BASE: VAddr = KERNEL_BASE + 0x3000_0000;
    pub const KERNEL_HEAP_SIZE: usize = 1024 * 1024 * 1024; // 1 GB

    /// User address space (Sv39 lower half)
    pub const USER_BASE: VAddr = 0x0000_0000_0000_0000;
    pub const USER_MAX: VAddr = 0x0000_003F_FFFF_FFFF;

    /// Stack location for user processes (top of user space)
    pub const USER_STACK_TOP: VAddr = 0x0000_003F_FFFF_F000;

    /// Page table entry bits
    pub const PTE_VALID: u64 = 1 << 0;
    pub const PTE_READ: u64 = 1 << 1;
    pub const PTE_WRITE: u64 = 1 << 2;
    pub const PTE_EXEC: u64 = 1 << 3;
    pub const PTE_USER: u64 = 1 << 4;
    pub const PTE_GLOBAL: u64 = 1 << 5;
    pub const PTE_ACCESSED: u64 = 1 << 6;
    pub const PTE_DIRTY: u64 = 1 << 7;

    /// Check if a virtual address is in kernel space
    #[inline]
    pub const fn is_kernel_vaddr(vaddr: VAddr) -> bool {
        vaddr >= KERNEL_BASE
    }

    /// Check if a virtual address is in user space
    #[inline]
    pub const fn is_user_vaddr(vaddr: VAddr) -> bool {
        vaddr < KERNEL_BASE
    }

    /// Check if a virtual address is canonical (Sv39)
    #[inline]
    pub const fn is_canonical_sv39(vaddr: VAddr) -> bool {
        // For Sv39: bits [63:39] must be sign-extended from bit 38
        let vaddr_u64 = vaddr as u64;
        let sign_bit = (vaddr_u64 >> 38) & 1;
        let upper_bits = (vaddr_u64 >> 39) as i64;
        sign_bit == 0 && upper_bits == 0 || sign_bit == 1 && upper_bits == -1
    }

    /// Check if a virtual address is canonical (Sv48)
    #[inline]
    pub const fn is_canonical_sv48(vaddr: VAddr) -> bool {
        // For Sv48: bits [63:48] must be sign-extended from bit 47
        let vaddr_u64 = vaddr as u64;
        let sign_bit = (vaddr_u64 >> 47) & 1;
        let upper_bits = (vaddr_u64 >> 48) as i64;
        sign_bit == 0 && upper_bits == 0 || sign_bit == 1 && upper_bits == -1
    }
}

/// ============================================================================
/// Cross-Architecture Helper Functions
/// ============================================================================

/// Architecture-agnostic check: Is this a kernel virtual address?
#[inline]
pub fn is_kernel_vaddr(vaddr: VAddr) -> bool {
    #[cfg(target_arch = "aarch64")]
    return arm64::is_kernel_vaddr(vaddr);

    #[cfg(target_arch = "x86_64")]
    return amd64::is_kernel_vaddr(vaddr);

    #[cfg(target_arch = "riscv64")]
    return riscv::is_kernel_vaddr(vaddr);
}

/// Architecture-agnostic check: Is this a user virtual address?
#[inline]
pub fn is_user_vaddr(vaddr: VAddr) -> bool {
    #[cfg(target_arch = "aarch64")]
    return arm64::is_user_vaddr(vaddr);

    #[cfg(target_arch = "x86_64")]
    return amd64::is_user_vaddr(vaddr);

    #[cfg(target_arch = "riscv64")]
    return riscv::is_user_vaddr(vaddr);
}

/// Architecture-agnostic check: Is this a canonical virtual address?
#[inline]
pub fn is_canonical_vaddr(vaddr: VAddr) -> bool {
    #[cfg(target_arch = "aarch64")]
    return arm64::is_canonical(vaddr);

    #[cfg(target_arch = "x86_64")]
    return amd64::is_canonical(vaddr);

    #[cfg(target_arch = "riscv64")]
    return riscv::is_canonical_sv39(vaddr);
}

/// Align an address down to page boundary
#[inline]
pub const fn page_align_down(addr: usize) -> usize {
    addr & !PAGE_MASK
}

/// Align an address up to page boundary
#[inline]
pub const fn page_align_up(addr: usize) -> usize {
    (addr + PAGE_MASK) & !PAGE_MASK
}

/// Check if an address is page-aligned
#[inline]
pub const fn is_page_aligned(addr: usize) -> bool {
    (addr & PAGE_MASK) == 0
}

/// Convert virtual address to page index
#[inline]
pub const fn virt_to_page_index(vaddr: VAddr) -> usize {
    vaddr / PAGE_SIZE
}

/// Convert page index to virtual address
#[inline]
pub const fn page_index_to_virt(page_index: usize) -> VAddr {
    page_index * PAGE_SIZE
}

/// ============================================================================
/// Memory Protection Flags (Cross-Architecture)
/// ============================================================================

/// Memory protection flags that are consistent across architectures
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemProt {
    /// No access
    None = 0,

    /// Read-only
    Read = 1 << 0,

    /// Write (implies read)
    Write = 1 << 1,

    /// Read + Write
    ReadWrite = 1 << 0 | 1 << 1,

    /// Execute (Write-Xor-Execute enforced)
    Execute = 1 << 2,
}

impl MemProt {
    /// No access
    pub const NONE: u8 = 0;

    /// Read-only
    pub const READ: u8 = 1 << 0;

    /// Write access
    pub const WRITE: u8 = 1 << 1;

    /// Execute access
    pub const EXEC: u8 = 1 << 2;

    /// Read + Write
    pub const RW: u8 = Self::READ as u8 | Self::WRITE as u8;

    /// Read + Execute
    pub const RX: u8 = Self::READ as u8 | Self::EXEC as u8;

    /// Read + Write + Execute (should be disallowed by W^X)
    pub const RWX: u8 = Self::READ as u8 | Self::WRITE as u8 | Self::EXEC as u8;

    /// Check if read is enabled
    #[inline]
    pub const fn can_read(self) -> bool {
        (self as u8 & Self::READ) != 0
    }

    /// Check if write is enabled
    #[inline]
    pub const fn can_write(self) -> bool {
        (self as u8 & Self::WRITE) != 0
    }

    /// Check if execute is enabled
    #[inline]
    pub const fn can_execute(self) -> bool {
        (self as u8 & Self::EXEC) != 0
    }

    /// Validate that W^X is enforced (no W + X together)
    #[inline]
    pub const fn is_valid_wxorx(self) -> bool {
        // W + X together is invalid
        !self.can_write() || !self.can_execute()
    }
}

impl fmt::Display for MemProt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.can_read() {
            write!(f, "R")?;
        }
        if self.can_write() {
            write!(f, "W")?;
        }
        if self.can_execute() {
            write!(f, "X")?;
        }
        if !self.can_read() && !self.can_write() && !self.can_execute() {
            write!(f, "NONE")?;
        }
        Ok(())
    }
}

impl core::ops::BitOr for MemProt {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        // Convert to u8, OR them together, and convert back
        let val = (self as u8) | (rhs as u8);
        // Map back to enum variant
        match val {
            0 => MemProt::None,
            1 => MemProt::Read,
            2 => MemProt::Write,
            3 => MemProt::ReadWrite,
            4 => MemProt::Execute,
            5 => MemProt::Read | MemProt::Execute, // R + X
            6 => MemProt::Write | MemProt::Execute, // W + X (invalid but possible)
            7 => MemProt::ReadWrite | MemProt::Execute, // R + W + X (invalid but possible)
            _ => MemProt::None,
        }
    }
}

impl core::ops::BitOrAssign for MemProt {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

/// ============================================================================
/// Address Space Types
/// ============================================================================

/// Address space identifier (ASID)
pub type Asid = u16;

/// Invalid ASID value
pub const ASID_INVALID: Asid = 0;

/// Global kernel ASID
#[cfg(target_arch = "aarch64")]
pub const ASID_GLOBAL: Asid = arm64::MMU_ARM64_GLOBAL_ASID;

/// Maximum number of address spaces
#[cfg(target_arch = "aarch64")]
pub const ASID_MAX: Asid = arm64::MMU_ARM64_MAX_USER_ASID;

/// ============================================================================
/// Region Types
/// ============================================================================

/// Memory region types for address space management
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionType {
    /// Reserved region (not yet allocated)
    Reserved = 0,

    /// Kernel code/text region
    KernelText = 1,

    /// Kernel data region
    KernelData = 2,

    /// Kernel heap region
    KernelHeap = 3,

    /// Per-CPU data region
    PerCpu = 4,

    /// Physical memory direct map
    PhysMap = 5,

    /// Device MMIO region
    Mmio = 6,

    /// User stack region
    UserStack = 7,

    /// User heap region
    UserHeap = 8,

    /// User code region
    UserText = 9,

    /// User data region
    UserData = 10,

    /// Guard page (no access)
    GuardPage = 11,
}

/// Memory region descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemRegion {
    /// Region base virtual address
    pub base: VAddr,

    /// Region size in bytes
    pub size: usize,

    /// Region type
    pub region_type: RegionType,

    /// Memory protection flags
    pub prot: MemProt,

    /// Region name (for debugging)
    pub name: &'static str,
}

impl MemRegion {
    /// Create a new memory region
    pub const fn new(base: VAddr, size: usize, region_type: RegionType, prot: MemProt, name: &'static str) -> Self {
        Self {
            base,
            size,
            region_type,
            prot,
            name,
        }
    }

    /// Get the end address (exclusive)
    pub const fn end(&self) -> VAddr {
        self.base + self.size
    }

    /// Check if a virtual address is within this region
    pub fn contains(&self, vaddr: VAddr) -> bool {
        vaddr >= self.base && vaddr < self.end()
    }

    /// Check if this region overlaps with another
    pub fn overlaps(&self, other: &MemRegion) -> bool {
        self.base < other.end() && self.end() > other.base
    }
}

/// Validate the memory layout
///
/// This function checks that the kernel memory layout is correctly configured.
/// It should be called during kernel initialization.
pub fn validate_layout() {
    #[cfg(target_arch = "aarch64")]
    {
        // ARM64-specific validation
        assert!(arm64::KERNEL_BASE <= 0xFFFF_0000_0000_0000);
    }

    #[cfg(target_arch = "x86_64")]
    {
        // AMD64-specific validation
        assert!(amd64::KERNEL_BASE >= 0xFFFF_8000_0000_0000);
    }

    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V-specific validation
        assert!(riscv::KERNEL_BASE >= 0xFFFF_0000_0000_0000);
    }

    // Common validation
    assert!(PAGE_SIZE == 4096, "Page size must be 4096");
    assert!(PAGE_SIZE_SHIFT == 12, "Page size shift must be 12");
}
