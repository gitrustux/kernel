// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::hypervisor;

// MSR constants
pub const X86_MSR_IA32_FEATURE_CONTROL: u32 = 0x003a;        // Feature control
pub const X86_MSR_IA32_VMX_BASIC: u32 = 0x0480;              // Basic info
pub const X86_MSR_IA32_VMX_CR0_FIXED0: u32 = 0x0486;         // CR0 bits that must be 0 to enter VMX
pub const X86_MSR_IA32_VMX_CR0_FIXED1: u32 = 0x0487;         // CR0 bits that must be 1 to enter VMX
pub const X86_MSR_IA32_VMX_CR4_FIXED0: u32 = 0x0488;         // CR4 bits that must be 0 to enter VMX
pub const X86_MSR_IA32_VMX_CR4_FIXED1: u32 = 0x0489;         // CR4 bits that must be 1 to enter VMX
pub const X86_MSR_IA32_VMX_EPT_VPID_CAP: u32 = 0x048c;       // VPID and EPT Capabilities
pub const X86_MSR_IA32_VMX_MISC: u32 = 0x0485;               // Miscellaneous info

// X86_MSR_IA32_VMX_BASIC flags
pub const VMX_MEMORY_TYPE_WRITE_BACK: u64 = 0x06;            // Write back

// X86_MSR_IA32_FEATURE_CONTROL flags
pub const X86_MSR_IA32_FEATURE_CONTROL_LOCK: u64 = 1u64 << 0;  // Locked
pub const X86_MSR_IA32_FEATURE_CONTROL_VMXON: u64 = 1u64 << 2; // Enable VMXON

// Stores VMX info from the IA32_VMX_BASIC MSR.
#[derive(Debug, Clone, Copy)]
pub struct VmxInfo {
    pub revision_id: u32,
    pub region_size: u16,
    pub write_back: bool,
    pub io_exit_info: bool,
    pub vmx_controls: bool,
}

// Stores EPT info from the IA32_VMX_EPT_VPID_CAP MSR.
#[derive(Debug, Clone, Copy)]
pub struct EptInfo {
    pub page_walk_4: bool,
    pub write_back: bool,
    pub invept: bool,
}

// VMX region to be used with both VMXON and VMCS.
#[repr(C)]
pub struct VmxRegion {
    pub revision_id: u32,
}

// VmxPage represents a page of memory allocated for VMX operations
pub struct VmxPage {
    // Implementation details would go here
}

impl VmxPage {
    pub fn new() -> Self {
        Self { /* Initialize fields */ }
    }

    pub fn physical_address(&self) -> paddr_t {
        // Implementation would go here
        0
    }
    
    pub fn virtual_address<T>(&self) -> *mut T {
        // Implementation would go here
        std::ptr::null_mut()
    }
    
    pub fn is_allocated(&self) -> bool {
        // Implementation would go here
        false
    }
}

pub fn alloc_vmx_state() -> rx_status_t;
pub fn free_vmx_state() -> rx_status_t;
pub fn cr_is_invalid(cr_value: u64, fixed0_msr: u32, fixed1_msr: u32) -> bool;