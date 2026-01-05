// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::vmx_cpu_state_priv::*;

use core::assert;
use crate::bits;
use crate::string;

use crate::hypervisor::cpu;
use crate::kernel::auto_lock;
use crate::kernel::mp;

use crate::fbl::mutex::Mutex;
use crate::fbl::array::Array;

static GUEST_MUTEX: Mutex<()> = Mutex::new(());
static mut NUM_GUESTS: usize = 0;
static mut VMXON_PAGES: Option<Array<VmxPage>> = None;

fn vmxon(pa: paddr_t) -> rx_status_t {
    let mut err: u8 = 0;

    unsafe {
        asm!(
            "vmxon {pa}",
            "setc {err}",
            pa = in(mem) pa,
            err = out(reg_byte) err,
            options(nostack, preserves_flags)
        );
    }

    if err != 0 { rx_ERR_INTERNAL } else { rx_OK }
}

fn vmxoff() -> rx_status_t {
    let mut err: u8 = 0;

    unsafe {
        asm!(
            "vmxoff",
            "setc {err}",
            err = out(reg_byte) err,
            options(nostack, preserves_flags)
        );
    }

    if err != 0 { rx_ERR_INTERNAL } else { rx_OK }
}

impl VmxInfo {
    pub fn new() -> Self {
        // From Volume 3, Appendix A.1.
        let basic_info = read_msr(X86_MSR_IA32_VMX_BASIC);
        Self {
            revision_id: bits::BITS(basic_info, 30, 0) as u32,
            region_size: bits::BITS_SHIFT(basic_info, 44, 32) as u16,
            write_back: bits::BITS_SHIFT(basic_info, 53, 50) == VMX_MEMORY_TYPE_WRITE_BACK,
            io_exit_info: bits::BIT_SHIFT(basic_info, 54) != 0,
            vmx_controls: bits::BIT_SHIFT(basic_info, 55) != 0,
        }
    }
}

impl EptInfo {
    pub fn new() -> Self {
        // From Volume 3, Appendix A.10.
        let ept_info = read_msr(X86_MSR_IA32_VMX_EPT_VPID_CAP);
        Self {
            page_walk_4: bits::BIT_SHIFT(ept_info, 6) != 0,
            write_back: bits::BIT_SHIFT(ept_info, 14) != 0,
            invept: 
                // INVEPT instruction is supported.
                bits::BIT_SHIFT(ept_info, 20) != 0 &&
                // Single-context INVEPT type is supported.
                bits::BIT_SHIFT(ept_info, 25) != 0 &&
                // All-context INVEPT type is supported.
                bits::BIT_SHIFT(ept_info, 26) != 0,
        }
    }
}

impl VmxPage {
    pub fn alloc(&mut self, vmx_info: &VmxInfo, fill: u8) -> rx_status_t {
        // From Volume 3, Appendix A.1: Bits 44:32 report the number of bytes that
        // software should allocate for the VMXON region and any VMCS region. It is
        // a value greater than 0 and at most 4096 (bit 44 is set if and only if
        // bits 43:32 are clear).
        if vmx_info.region_size > PAGE_SIZE as u16 {
            return rx_ERR_NOT_SUPPORTED;
        }

        // Check use of write-back memory for VMX regions is supported.
        if !vmx_info.write_back {
            return rx_ERR_NOT_SUPPORTED;
        }

        // The maximum size for a VMXON or VMCS region is 4096, therefore
        // unconditionally allocating a page is adequate.
        hypervisor::Page::alloc(self, fill)
    }
}

fn vmxon_task(context: *mut libc::c_void, cpu_num: cpu_num_t) -> rx_status_t {
    let pages = unsafe { &mut *(context as *mut Array<VmxPage>) };
    let page = &mut pages[cpu_num as usize];

    // Check that we have instruction information when we VM exit on IO.
    let vmx_info = VmxInfo::new();
    if !vmx_info.io_exit_info {
        return rx_ERR_NOT_SUPPORTED;
    }

    // Check that full VMX controls are supported.
    if !vmx_info.vmx_controls {
        return rx_ERR_NOT_SUPPORTED;
    }

    // Check that a page-walk length of 4 is supported.
    let ept_info = EptInfo::new();
    if !ept_info.page_walk_4 {
        return rx_ERR_NOT_SUPPORTED;
    }

    // Check use write-back memory for EPT is supported.
    if !ept_info.write_back {
        return rx_ERR_NOT_SUPPORTED;
    }

    // Check that the INVEPT instruction is supported.
    if !ept_info.invept {
        return rx_ERR_NOT_SUPPORTED;
    }

    // Enable VMXON, if required.
    let mut feature_control = read_msr(X86_MSR_IA32_FEATURE_CONTROL);
    if !(feature_control & X86_MSR_IA32_FEATURE_CONTROL_LOCK != 0) ||
       !(feature_control & X86_MSR_IA32_FEATURE_CONTROL_VMXON != 0) {
        if (feature_control & X86_MSR_IA32_FEATURE_CONTROL_LOCK != 0) &&
           !(feature_control & X86_MSR_IA32_FEATURE_CONTROL_VMXON != 0) {
            return rx_ERR_NOT_SUPPORTED;
        }
        feature_control |= X86_MSR_IA32_FEATURE_CONTROL_LOCK;
        feature_control |= X86_MSR_IA32_FEATURE_CONTROL_VMXON;
        write_msr(X86_MSR_IA32_FEATURE_CONTROL, feature_control);
    }

    // Check control registers are in a VMX-friendly state.
    let cr0 = x86_get_cr0();
    if cr_is_invalid(cr0, X86_MSR_IA32_VMX_CR0_FIXED0, X86_MSR_IA32_VMX_CR0_FIXED1) {
        return rx_ERR_BAD_STATE;
    }
    let cr4 = x86_get_cr4() | X86_CR4_VMXE;
    if cr_is_invalid(cr4, X86_MSR_IA32_VMX_CR4_FIXED0, X86_MSR_IA32_VMX_CR4_FIXED1) {
        return rx_ERR_BAD_STATE;
    }

    // Enable VMX using the VMXE bit.
    x86_set_cr4(cr4);

    // Setup VMXON page.
    let region = unsafe { &mut *(page.virtual_address::<VmxRegion>()) };
    region.revision_id = vmx_info.revision_id;

    // Execute VMXON.
    let status = vmxon(page.physical_address());
    if status != rx_OK {
        dprintf!(CRITICAL, "Failed to turn on VMX on CPU {}\n", cpu_num);
        return status;
    }

    rx_OK
}

fn vmxoff_task(_arg: *mut libc::c_void) {
    // Execute VMXOFF.
    let status = vmxoff();
    if status != rx_OK {
        dprintf!(CRITICAL, "Failed to turn off VMX on CPU {}\n", arch_curr_cpu_num());
        return;
    }

    // Disable VMX.
    x86_set_cr4(x86_get_cr4() & !X86_CR4_VMXE);
}

pub fn alloc_vmx_state() -> rx_status_t {
    let _lock = GUEST_MUTEX.lock();
    unsafe {
        if NUM_GUESTS == 0 {
            let num_cpus = arch_max_num_cpus();
            let mut pages = Array::<VmxPage>::new_with_size(num_cpus)?;
            
            let vmx_info = VmxInfo::new();
            for page in &mut pages {
                let status = page.alloc(&vmx_info, 0);
                if status != rx_OK {
                    return status;
                }
            }

            // Enable VMX for all online CPUs.
            let cpu_mask = percpu_exec(vmxon_task, &mut pages as *mut _ as *mut libc::c_void);
            if cpu_mask != mp_get_online_mask() {
                mp_sync_exec(MP_IPI_TARGET_MASK, cpu_mask, vmxoff_task, core::ptr::null_mut());
                return rx_ERR_NOT_SUPPORTED;
            }

            VMXON_PAGES = Some(pages);
        }
        NUM_GUESTS += 1;
        rx_OK
    }
}

pub fn free_vmx_state() -> rx_status_t {
    let _lock = GUEST_MUTEX.lock();
    unsafe {
        NUM_GUESTS -= 1;
        if NUM_GUESTS == 0 {
            mp_sync_exec(MP_IPI_TARGET_ALL, 0, vmxoff_task, core::ptr::null_mut());
            VMXON_PAGES = None;
        }
        rx_OK
    }
}

pub fn cr_is_invalid(cr_value: u64, fixed0_msr: u32, fixed1_msr: u32) -> bool {
    let fixed0 = read_msr(fixed0_msr);
    let fixed1 = read_msr(fixed1_msr);
    !(cr_value | !fixed0) != 0 || !(!cr_value | fixed1) != 0
}