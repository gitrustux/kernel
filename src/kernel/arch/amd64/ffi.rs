// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86-64 FFI Bridge to C
//!
//! This module provides Foreign Function Interface (FFI) declarations
//! for calling C-implemented architecture-specific functions from Rust.
//!
//! These functions are implemented in sys_x86.c and provide low-level
//! operations that require special CPU instructions or are easier in C.


use crate::rustux::types::*;

// ============= Type Definitions =============

/// CPUID leaf structure (matching C struct x86_cpuid_leaf)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86CpuidLeaf {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

/// Status codes matching Rust's RxStatus
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86Status {
    Ok = 0,
    ErrNoMemory = 1,
    ErrInvalidArgs = 3,
    ErrBadState = 9,
    ErrNotSupported = 2,
    ErrNotFound = 4,
}

// ============= Page Table Functions =============

extern "C" {
    /// Check if virtual address is canonical (x86-64)
    ///
    /// In x86-64, bits [63:48] must be all 0 or all 1
    pub fn sys_x86_is_vaddr_canonical(vaddr: u64) -> bool;

    /// Check if physical address is valid
    ///
    /// x86-64 supports up to 52-bit physical addresses
    pub fn sys_x86_mmu_check_paddr(paddr: u64) -> bool;

    /// Get kernel CR3 register value
    pub fn sys_x86_kernel_cr3() -> u64;
}

// ============= Per-CPU Functions =============

extern "C" {
    /// Initialize per-CPU data for given CPU number
    pub fn sys_x86_init_percpu(cpu_num: u32);

    /// Set local APIC ID
    pub fn sys_x86_set_local_apic_id(apic_id: u32);

    /// Convert APIC ID to CPU number
    pub fn sys_x86_apic_id_to_cpu_num(apic_id: u32) -> i32;
}

// ============= Descriptor/TSS Functions =============

extern "C" {
    /// Initialize per-CPU TSS
    pub fn sys_x86_initialize_percpu_tss();

    /// Set TSS SP0 (kernel stack pointer)
    pub fn sys_x86_set_tss_sp(sp: u64);

    /// Clear TSS busy bit for task switch
    pub fn sys_x86_clear_tss_busy(sel: u16);

    /// Reset TSS I/O bitmap
    pub fn sys_x86_reset_tss_io_bitmap();
}

// ============= Extended Register Functions =============

extern "C" {
    /// Initialize extended register state (SSE/AVX)
    pub fn sys_x86_extended_register_init();

    /// Get extended register size
    pub fn sys_x86_extended_register_size() -> usize;
}

// ============= Feature Detection Functions =============

extern "C" {
    /// Initialize CPU feature detection
    pub fn sys_x86_feature_init();

    /// Get CPUID subleaf
    ///
    /// Returns true if the leaf is valid and data was retrieved
    pub fn sys_x86_get_cpuid_subleaf(
        leaf: u32,
        subleaf: u32,
        out: *mut X86CpuidLeaf,
    ) -> bool;
}

// ============= Bootstrap Functions =============

extern "C" {
    /// Initialize bootstrap16 subsystem
    pub fn sys_x86_bootstrap16_init(bootstrap_base: u64);

    /// Acquire bootstrap16 memory region
    ///
    /// Returns 0 on success, negative on error
    pub fn sys_x86_bootstrap16_acquire(
        entry64: u64,
        temp_aspace: *mut *mut u8,
        bootstrap_aperture: *mut *mut u8,
        instr_ptr: *mut u64,
    ) -> i32;

    /// Release bootstrap16 memory region
    pub fn sys_x86_bootstrap16_release(bootstrap_aperture: *mut u8);
}

// ============= Memory Barrier Functions =============

extern "C" {
    /// Full memory barrier (mfence)
    pub fn sys_x86_mb();

    /// Read memory barrier (lfence)
    pub fn sys_x86_rmb();

    /// Write memory barrier (sfence)
    pub fn sys_x86_wmb();

    /// Acquire barrier
    pub fn sys_x86_acquire();

    /// Release barrier
    pub fn sys_x86_release();
}

// ============= HLT/Pause Functions =============

extern "C" {
    /// Halt the CPU (hlt instruction)
    pub fn sys_x86_halt();

    /// Pause CPU (pause instruction)
    pub fn sys_x86_pause();

    /// Serialize execution (cpuid)
    pub fn sys_x86_serialize();
}

// ============= TSC Functions =============

extern "C" {
    /// Adjust TSC
    pub fn sys_x86_tsc_adjust();

    /// Store TSC adjustment
    pub fn sys_x86_tsc_store_adjustment();
}

// ============ MMU Init Functions ============

extern "C" {
    /// Early MMU initialization
    pub fn sys_x86_mmu_early_init();

    /// Per-CPU MMU initialization
    pub fn sys_x86_mmu_percpu_init();

    /// Main MMU initialization
    pub fn sys_x86_mmu_init();
}

// ============ TLB Flush Functions ============

extern "C" {
    /// Flush entire TLB
    pub fn sys_x86_tlb_flush_global();

    /// Flush single page from TLB
    pub fn sys_x86_tlb_flush_one(vaddr: u64);
}

// ============ User Copy Functions ============

extern "C" {
    /// Copy data to/from user space with fault handling
    ///
    /// Returns number of bytes copied, or negative on error
    pub fn sys_x86_copy_to_or_from_user(
        dst: *mut u8,
        src: *const u8,
        len: usize,
        fault_return: u64,
    ) -> isize;
}

// ============ APIC/MP Functions ============

extern "C" {
    /// IPI halt handler - never returns
    pub fn sys_x86_ipi_halt_handler() -> !;

    /// Secondary CPU entry point
    pub fn sys_x86_secondary_entry(
        aps_still_booting: *mut i32,
        thread: *mut u8,
    );

    /// Force all CPUs except local and BSP to halt
    pub fn sys_x86_force_halt_all_but_local_and_bsp();

    /// Allocate AP structures
    ///
    /// Returns 0 on success, negative on error
    pub fn sys_x86_allocate_ap_structures(
        apic_ids: *const u32,
        cpu_count: u8,
    ) -> i32;
}

// ============ CPU Topology Functions ============

extern "C" {
    /// Initialize CPU topology detection
    pub fn sys_x86_cpu_topology_init();

    /// Decode CPU topology for given APIC ID
    ///
    /// Returns 0 on success, negative on error
    pub fn sys_x86_cpu_topology_decode(
        apic_id: u32,
        topo: *mut u8,
    ) -> i32;
}

// ============ Timer Functions ============

extern "C" {
    /// Look up TSC frequency from CPUID or platform
    ///
    /// Returns frequency in Hz, or 0 if unknown
    pub fn sys_x86_lookup_tsc_freq() -> u64;

    /// Look up core crystal frequency
    ///
    /// Returns frequency in Hz
    pub fn sys_x86_lookup_core_crystal_freq() -> u64;
}

// ============ Page Table MMU Functions ============

extern "C" {
    /// Get terminal flags for MMU page tables
    pub fn sys_x86_page_table_mmu_terminal_flags(
        level: i32,
        flags: u32,
    ) -> u64;

    /// Get intermediate flags for MMU page tables
    pub fn sys_x86_page_table_mmu_intermediate_flags() -> u64;

    /// Check if large pages are supported at given level
    pub fn sys_x86_page_table_mmu_supports_page_size(level: i32) -> bool;

    /// Get split flags for large pages
    pub fn sys_x86_page_table_mmu_split_flags(level: i32, flags: u64) -> u64;

    /// Convert PTE flags to MMU flags
    pub fn sys_x86_page_table_mmu_pt_flags_to_mmu_flags(
        flags: u64,
        level: i32,
    ) -> u32;
}

// ============ EPT Functions ============

extern "C" {
    /// Check if EPT flags are allowed
    pub fn sys_x86_page_table_ept_allowed_flags(flags: u32) -> bool;

    /// Check if physical address is valid for EPT
    pub fn sys_x86_page_table_ept_check_paddr(paddr: u64) -> bool;

    /// Check if virtual address is valid for EPT
    pub fn sys_x86_page_table_ept_check_vaddr(vaddr: u64) -> bool;

    /// Check if large pages are supported at given level for EPT
    pub fn sys_x86_page_table_ept_supports_page_size(level: i32) -> bool;

    /// Get intermediate flags for EPT
    pub fn sys_x86_page_table_ept_intermediate_flags() -> u64;

    /// Get terminal flags for EPT
    pub fn sys_x86_page_table_ept_terminal_flags(level: i32, flags: u32) -> u64;

    /// Get split flags for EPT
    pub fn sys_x86_page_table_ept_split_flags(level: i32, flags: u64) -> u64;

    /// Convert EPT flags to MMU flags
    pub fn sys_x86_page_table_ept_pt_flags_to_mmu_flags(
        flags: u64,
        level: i32,
    ) -> u32;
}

// ============ Address Space Functions ============

extern "C" {
    /// Map contiguous physical memory region
    pub fn sys_x86_arch_vm_aspace_map_contiguous(
        aspace: *mut u8,
        vaddr: u64,
        paddr: u64,
        count: usize,
        mmu_flags: u32,
        addrs: u64,
    ) -> i32;

    /// Map pages
    pub fn sys_x86_arch_vm_aspace_map(
        aspace: *mut u8,
        vaddr: u64,
        phys: *const u64,
        count: usize,
        mmu_flags: u32,
        addrs: u64,
    ) -> i32;

    /// Unmap pages
    pub fn sys_x86_arch_vm_aspace_unmap(
        aspace: *mut u8,
        vaddr: u64,
        count: usize,
    ) -> i32;

    /// Change page protections
    pub fn sys_x86_arch_vm_aspace_protect(
        aspace: *mut u8,
        vaddr: u64,
        count: usize,
        mmu_flags: u32,
    ) -> i32;

    /// Query mapping
    pub fn sys_x86_arch_vm_aspace_query(
        aspace: *mut u8,
        vaddr: u64,
    ) -> i32;

    /// Find free spot in address space
    pub fn sys_x86_arch_vm_aspace_pick_spot(
        aspace: *mut u8,
        base: u64,
        prev_region_mmu_flags: u64,
        out_vaddr: *mut u64,
        out_size: *mut u64,
    ) -> i32;

    /// Switch address spaces
    pub fn sys_x86_arch_vm_aspace_context_switch(
        from_aspace: *mut u8,
        to_aspace: *mut u8,
    ) -> i32;
}

// ============ PAT/Memory Type Functions ============

extern "C" {
    /// Initialize Page Attribute Table
    pub fn sys_x86_mmu_mem_type_init();

    /// Sync PAT configuration across CPUs
    pub fn sys_x86_pat_sync(targets: u64);
}

// ============ Processor Trace Functions ============

extern "C" {
    /// Initialize Intel Processor Trace
    pub fn sys_x86_processor_trace_init();
}

// ============ I/O Port Functions ============

extern "C" {
    /// Set TSS I/O bitmap
    pub fn sys_x86_set_tss_io_bitmap(bitmap: *mut u8);

    /// Clear TSS I/O bitmap
    pub fn sys_x86_clear_tss_io_bitmap(bitmap: *mut u8);
}
