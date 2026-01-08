// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Bootstrap16 support for x86 architecture
//!
//! This module provides functionality for bootstrapping 16-bit mode on x86
//! processors, primarily used for SMP initialization and suspend/resume support.

use crate::arch::amd64::mmu::*;
use crate::kernel::vm::layout::PAGE_SIZE;
use crate::kernel::vm::VmAspace;
use crate::rustux::types::*;
use crate::SMP_MAX_CPUS;
use crate::kernel::thread::Thread;
use core::sync::atomic::AtomicI32;
use alloc::sync::Arc;

/// Offset of physical bootstrap PML4 in bootstrap data structure
pub const BCD_PHYS_BOOTSTRAP_PML4_OFFSET: usize = 0;
/// Offset of physical kernel PML4 in bootstrap data structure
pub const BCD_PHYS_KERNEL_PML4_OFFSET: usize = 4;
/// Offset of physical GDTR in bootstrap data structure
pub const BCD_PHYS_GDTR_OFFSET: usize = 8;
/// Offset of long mode entry point in bootstrap data structure
pub const BCD_PHYS_LM_ENTRY_OFFSET: usize = 20;
/// Offset of long mode code segment in bootstrap data structure
pub const BCD_LM_CS_OFFSET: usize = 24;
/// Offset of CPU counter in AP bootstrap data structure
pub const BCD_CPU_COUNTER_OFFSET: usize = 28;
/// Offset of CPU waiting flag in AP bootstrap data structure
pub const BCD_CPU_WAITING_OFFSET: usize = 32;
/// Offset of per-CPU data in AP bootstrap data structure
pub const BCD_PER_CPU_BASE_OFFSET: usize = 40;

/// Offset of registers pointer in realmode entry data structure
pub const RED_REGISTERS_OFFSET: usize = 28;

/// Base data structure for x86 16-bit bootstrap
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct X86Bootstrap16Data {
    /// Physical address of identity PML4
    pub phys_bootstrap_pml4: u32,
    /// Physical address of the kernel PML4
    pub phys_kernel_pml4: u32,
    /// Physical address of GDTR (limit and base)
    pub phys_gdtr_limit: u16,
    pub phys_gdtr_base: u64,
    pub _pad: u16,

    // Ordering of these two matter; they should be usable by retfl
    /// Physical address of long mode entry point
    pub phys_long_mode_entry: u32,
    /// 64-bit code segment to use
    pub long_mode_cs: u32,
}

/// Data structure for real mode entry operations
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct X86RealModeEntryData {
    /// Base bootstrap data
    pub hdr: X86Bootstrap16Data,

    /// Virtual address of the register dump
    pub registers_ptr: u64,
}

/// Register state for real mode entry operations
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct X86RealModeEntryDataRegisters {
    pub rdi: u64, pub rsi: u64, pub rbp: u64, pub rbx: u64, pub rdx: u64, pub rcx: u64, pub rax: u64,
    pub r8: u64, pub r9: u64, pub r10: u64, pub r11: u64, pub r12: u64, pub r13: u64, pub r14: u64, pub r15: u64,
    pub rsp: u64, pub rip: u64,
}

/// Data structure for AP bootstrap operations
#[repr(C, packed)]
#[derive(Debug)]
pub struct X86ApBootstrapData {
    /// Base bootstrap data
    pub hdr: X86Bootstrap16Data,

    /// Counter for APs to use to determine which stack to take
    pub cpu_id_counter: u32,
    /// Pointer to value to use to determine when APs are done with boot
    pub cpu_waiting_mask: *const AtomicI32,

    /// Per-cpu data for each AP
    pub per_cpu: [X86ApPerCpuData; (SMP_MAX_CPUS - 1) as usize],
}

/// Per-CPU data for each AP during bootstrap
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct X86ApPerCpuData {
    /// Virtual address of base of initial kstack
    pub kstack_base: VAddr,
    /// Virtual address of initial thread_t
    pub thread: *mut Thread,
}

// Extern declarations for assembly entry points
extern "C" {
    /// Start of bootstrap16 code
    pub fn x86_bootstrap16_start();
    
    /// End of bootstrap16 code
    pub fn x86_bootstrap16_end();
    
    /// Entry point used for secondary CPU initialization
    pub fn _x86_secondary_cpu_long_mode_entry();
    
    /// Entry point used for suspend-to-RAM resume vector.
    /// Note that this does not restore %rdi, and it touches below the saved %rsp.
    pub fn _x86_suspend_wakeup();
}

/// Initialize the bootstrap16 subsystem
///
/// # Arguments
///
/// * `bootstrap_base` - Physical address of two consecutive pages of RAM with addresses
///                     less than 1M that are available for the OS to use.
///
/// # Safety
///
/// This function is unsafe because it manipulates physical memory directly
/// and requires the caller to ensure the provided memory region is valid.
pub unsafe fn x86_bootstrap16_init(bootstrap_base: PAddr) {
    sys_x86_bootstrap16_init(bootstrap_base);
}

/// Acquire bootstrap16 resources
///
/// Upon success, returns a pointer to the bootstrap aspace, a pointer to the
/// virtual address of the bootstrap data, and the physical address of the
/// first instruction that should be executed in 16-bit mode.
///
/// # Arguments
///
/// * `entry64` - 64-bit entry point address
///
/// # Returns
///
/// * `Ok((temp_aspace, bootstrap_aperture, instr_ptr))` - Success with bootstrap resources
/// * `Err(status)` - Failure with status code
///
/// # Safety
///
/// The caller is responsible for calling `x86_bootstrap16_release` when done
/// with the bootstrap resources.
pub unsafe fn x86_bootstrap16_acquire(
    entry64: usize,
) -> core::result::Result<(Arc<VmAspace>, *mut core::ffi::c_void, PAddr), RxStatus> {
    let mut temp_aspace: Arc<VmAspace> = Arc::new(VmAspace::new(
        crate::kernel::vm::aspace::AddressSpaceFlags::None,
        0,
        0,
    ).unwrap()); // Placeholder, will be replaced by FFI
    let mut bootstrap_aperture: *mut core::ffi::c_void = core::ptr::null_mut();
    let mut instr_ptr: PAddr = 0;

    let status = sys_x86_bootstrap16_acquire(
        entry64,
        &mut temp_aspace as *mut _,
        &mut bootstrap_aperture as *mut _,
        &mut instr_ptr as *mut _,
    );

    if status.is_ok() {
        Ok((temp_aspace, bootstrap_aperture, instr_ptr))
    } else {
        Err(status)
    }
}

/// Release bootstrap16 resources
///
/// To be called once the caller is done using the bootstrap16 module.
///
/// # Arguments
///
/// * `bootstrap_aperture` - Bootstrap aperture pointer from `x86_bootstrap16_acquire`
///
/// # Safety
///
/// This function is unsafe because it manipulates memory mappings.
pub unsafe fn x86_bootstrap16_release(bootstrap_aperture: *mut core::ffi::c_void) {
    sys_x86_bootstrap16_release(bootstrap_aperture);
}

// Static assertions to ensure memory layout matches constants
// These will be verified at compile time
const _: () = assert!(core::mem::size_of::<X86ApBootstrapData>() <= PAGE_SIZE);
const _: () = assert!(core::mem::size_of::<X86RealModeEntryData>() <= PAGE_SIZE);

const _: () = assert!(memoffset::offset_of!(X86Bootstrap16Data, phys_bootstrap_pml4) == BCD_PHYS_BOOTSTRAP_PML4_OFFSET);
const _: () = assert!(memoffset::offset_of!(X86Bootstrap16Data, phys_kernel_pml4) == BCD_PHYS_KERNEL_PML4_OFFSET);
const _: () = assert!(memoffset::offset_of!(X86Bootstrap16Data, phys_gdtr_limit) == BCD_PHYS_GDTR_OFFSET);
const _: () = assert!(memoffset::offset_of!(X86Bootstrap16Data, phys_gdtr_base) == BCD_PHYS_GDTR_OFFSET + 2);
const _: () = assert!(memoffset::offset_of!(X86Bootstrap16Data, phys_long_mode_entry) == BCD_PHYS_LM_ENTRY_OFFSET);
const _: () = assert!(memoffset::offset_of!(X86Bootstrap16Data, long_mode_cs) == BCD_LM_CS_OFFSET);

const _: () = assert!(memoffset::offset_of!(X86ApBootstrapData, hdr) == 0);
const _: () = assert!(memoffset::offset_of!(X86ApBootstrapData, cpu_id_counter) == BCD_CPU_COUNTER_OFFSET);
const _: () = assert!(memoffset::offset_of!(X86ApBootstrapData, cpu_waiting_mask) == BCD_CPU_WAITING_OFFSET);
const _: () = assert!(memoffset::offset_of!(X86ApBootstrapData, per_cpu) == BCD_PER_CPU_BASE_OFFSET);

const _: () = assert!(memoffset::offset_of!(X86RealModeEntryData, hdr) == 0);
const _: () = assert!(memoffset::offset_of!(X86RealModeEntryData, registers_ptr) == RED_REGISTERS_OFFSET);

// FFI declarations for the actual implementations
extern "C" {
    fn sys_x86_bootstrap16_init(bootstrap_base: PAddr);
    fn sys_x86_bootstrap16_acquire(
        entry64: usize,
        temp_aspace: *mut Arc<VmAspace>,
        bootstrap_aperture: *mut *mut core::ffi::c_void,
        instr_ptr: *mut PAddr,
    ) -> RxStatus;
    fn sys_x86_bootstrap16_release(bootstrap_aperture: *mut core::ffi::c_void);
}