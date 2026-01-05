// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::amd64::apic;
use crate::arch::amd64::feature;
use crate::rustux::syscalls::hypervisor;

use crate::vmx_cpu_state_priv::*;

fn ignore_msr(msr_bitmaps_page: &mut VmxPage, ignore_writes: bool, msr: u32) {
    // From Volume 3, Section 24.6.9.
    let mut msr_bitmaps = msr_bitmaps_page.virtual_address::<u8>();
    if msr >= 0xc0000000 {
        msr_bitmaps = unsafe { msr_bitmaps.add(1 << 10) };
    }

    let msr_low = msr & 0x1fff;
    let msr_byte = (msr_low / 8) as u16;
    let msr_bit = (msr_low % 8) as u8;

    // Ignore reads to the MSR.
    unsafe {
        let ptr = msr_bitmaps.add(msr_byte as usize);
        *ptr &= !(1 << msr_bit);
    }

    if ignore_writes {
        // Ignore writes to the MSR.
        let write_ptr = unsafe { msr_bitmaps.add((2 << 10) + msr_byte as usize) };
        unsafe {
            *write_ptr &= !(1 << msr_bit);
        }
    }
}

pub struct Guest {
    gpas: Option<hypervisor::GuestPhysicalAddressSpace>,
    msr_bitmaps_page: VmxPage,
    vpid_allocator: IdAllocator,
    vcpu_mutex: Mutex<()>,
    traps: TrapMap,
}

impl Guest {
    pub fn create() -> Result<Box<Guest>, rx_status_t> {
        // Check that the CPU supports VMX.
        if !feature::x86_feature_test(feature::X86_FEATURE_VMX) {
            return Err(rx_ERR_NOT_SUPPORTED);
        }

        let status = alloc_vmx_state();
        if status != rx_OK {
            return Err(status);
        }

        let mut guest = Box::new(Guest {
            gpas: None,
            msr_bitmaps_page: VmxPage::new(),
            vpid_allocator: IdAllocator::new(),
            vcpu_mutex: Mutex::new(()),
            traps: TrapMap::new(),
        });

        let gpas = hypervisor::GuestPhysicalAddressSpace::create()?;
        guest.gpas = Some(gpas);

        // Setup common MSR bitmaps.
        let vmx_info = VmxInfo::new();
        guest.msr_bitmaps_page.alloc(&vmx_info, u8::MAX)?;

        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_PAT);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_EFER);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_FS_BASE);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_GS_BASE);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_KERNEL_GS_BASE);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_STAR);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_LSTAR);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_FMASK);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_TSC_ADJUST);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_TSC_AUX);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_SYSENTER_CS);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_SYSENTER_ESP);
        ignore_msr(&mut guest.msr_bitmaps_page, true, X86_MSR_IA32_SYSENTER_EIP);

        // Setup VPID allocator
        {
            let _lock = guest.vcpu_mutex.lock();
            guest.vpid_allocator.init()?;
        }

        Ok(guest)
    }

    pub fn set_trap(
        &mut self,
        kind: u32,
        addr: rx_vaddr_t,
        len: usize,
        port: Option<Arc<PortDispatcher>>,
        key: u64,
    ) -> rx_status_t {
        if len == 0 {
            return rx_ERR_INVALID_ARGS;
        } else if usize::MAX - len < addr {
            return rx_ERR_OUT_OF_RANGE;
        }

        match kind {
            rx_GUEST_TRAP_MEM => {
                if port.is_some() {
                    return rx_ERR_INVALID_ARGS;
                }
            }
            rx_GUEST_TRAP_BELL => {
                if port.is_none() {
                    return rx_ERR_INVALID_ARGS;
                }
            }
            rx_GUEST_TRAP_IO => {
                if port.is_some() {
                    return rx_ERR_INVALID_ARGS;
                } else if addr + len > u16::MAX as usize {
                    return rx_ERR_OUT_OF_RANGE;
                }
                return self.traps.insert_trap(kind, addr, len, port, key);
            }
            _ => return rx_ERR_INVALID_ARGS,
        }

        // Common logic for memory-based traps.
        if !is_page_aligned(addr) || !is_page_aligned(len) {
            return rx_ERR_INVALID_ARGS;
        }
        
        if let Some(gpas) = &mut self.gpas {
            let status = gpas.unmap_range(addr, len);
            if status != rx_OK {
                return status;
            }
        }
        
        self.traps.insert_trap(kind, addr, len, port, key)
    }

    pub fn alloc_vpid(&self, vpid: &mut u16) -> rx_status_t {
        let _lock = self.vcpu_mutex.lock();
        self.vpid_allocator.alloc_id(vpid)
    }

    pub fn free_vpid(&self, vpid: u16) -> rx_status_t {
        let _lock = self.vcpu_mutex.lock();
        self.vpid_allocator.free_id(vpid)
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        free_vmx_state();
    }
}

fn is_page_aligned(addr: usize) -> bool {
    addr & (PAGE_SIZE - 1) == 0
}