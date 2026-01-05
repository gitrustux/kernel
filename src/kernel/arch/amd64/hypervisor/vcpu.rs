// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::bits;

use crate::arch::amd64::descriptor;
use crate::arch::amd64::feature;
use crate::arch::amd64::pvclock;
use crate::fbl::auto_call;
use crate::hypervisor::cpu;
use crate::hypervisor::ktrace;
use crate::kernel::mp;
use crate::lib::ktrace;
use crate::vm::fault;
use crate::vm::pmm;
use crate::vm::vm_object;
use crate::rustux::syscalls::hypervisor as rx_hypervisor;

use crate::pvclock_priv::*;
use crate::vcpu_priv::*;
use crate::vmexit_priv::*;
use crate::vmx_cpu_state_priv::*;
use core::sync::atomic::{AtomicBool, Ordering};

const INTERRUPT_INFO_VALID: u32 = 1u32 << 31;
const INTERRUPT_INFO_DELIVER_ERROR_CODE: u32 = 1u32 << 11;
const INTERRUPT_TYPE_NMI: u32 = 2u32 << 8;
const INTERRUPT_TYPE_HARDWARE_EXCEPTION: u32 = 3u32 << 8;
const INTERRUPT_TYPE_SOFTWARE_EXCEPTION: u32 = 6u32 << 8;
const BASE_PROCESSOR_VPID: u16 = 1;

fn invept(invalidation: InvEpt, eptp: u64) -> rx_status_t {
    let mut err: u8 = 0;
    let mut descriptor: [u64; 2] = [eptp, 0];

    unsafe {
        asm!(
            "invept {descriptor}, {invalidation}",
            "setc {err}",
            descriptor = in(mem) descriptor,
            invalidation = in(reg) invalidation as u64,
            err = out(reg_byte) err,
            options(nostack, preserves_flags)
        );
    }

    if err != 0 {
        rx_ERR_INTERNAL
    } else {
        rx_OK
    }
}

fn vmptrld(pa: paddr_t) -> rx_status_t {
    let mut err: u8 = 0;

    unsafe {
        asm!(
            "vmptrld {pa}",
            "setc {err}",
            pa = in(mem) pa,
            err = out(reg_byte) err,
            options(nostack, preserves_flags)
        );
    }

    if err != 0 {
        rx_ERR_INTERNAL
    } else {
        rx_OK
    }
}

fn vmclear(pa: paddr_t) -> rx_status_t {
    let mut err: u8 = 0;

    unsafe {
        asm!(
            "vmclear {pa}",
            "setc {err}",
            pa = in(mem) pa,
            err = out(reg_byte) err,
            options(nostack, preserves_flags)
        );
    }

    if err != 0 {
        rx_ERR_INTERNAL
    } else {
        rx_OK
    }
}

fn vmread(field: u64) -> u64 {
    let mut err: u8 = 0;
    let mut val: u64 = 0;

    unsafe {
        asm!(
            "vmread {field}, {val}",
            "setc {err}",
            field = in(reg) field,
            val = out(reg) val,
            err = out(reg_byte) err,
            options(nostack, preserves_flags)
        );
    }

    debug_assert!(err == 0, "vmread failed");
    val
}

fn vmwrite(field: u64, val: u64) {
    let mut err: u8 = 0;

    unsafe {
        asm!(
            "vmwrite {val}, {field}",
            "setc {err}",
            val = in(reg) val,
            field = in(reg) field,
            err = out(reg_byte) err,
            options(nostack, preserves_flags)
        );
    }

    debug_assert!(err == 0, "vmwrite failed");
}

pub struct AutoVmcs {
    vmcs_address: paddr_t,
}

impl AutoVmcs {
    pub fn new(vmcs_address: paddr_t) -> Self {
        debug_assert!(!arch_ints_disabled());
        unsafe { arch_disable_ints() };
        let status = vmptrld(vmcs_address);
        debug_assert!(status == rx_OK);
        
        Self {
            vmcs_address,
        }
    }

    pub fn invalidate(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.vmcs_address = 0;
        }
    }

    pub fn interrupt_window_exiting(&self, enable: bool) {
        debug_assert!(self.vmcs_address != 0);
        let mut controls = self.read(VmcsField32::PROCBASED_CTLS);
        if enable {
            controls |= PROCBASED_CTLS_INT_WINDOW_EXITING;
        } else {
            controls &= !PROCBASED_CTLS_INT_WINDOW_EXITING;
        }
        self.write(VmcsField32::PROCBASED_CTLS, controls);
    }

    pub fn issue_interrupt(&self, vector: u32) {
        debug_assert!(self.vmcs_address != 0);
        let mut interrupt_info = INTERRUPT_INFO_VALID | (vector & 0xFF);
        if vector == X86_INT_BREAKPOINT || vector == X86_INT_OVERFLOW {
            // From Volume 3, Section 24.8.3. A VMM should use type hardware exception for all
            // exceptions other than breakpoints and overflows, which should be software exceptions.
            interrupt_info |= INTERRUPT_TYPE_SOFTWARE_EXCEPTION;
        } else if vector == X86_INT_NMI {
            interrupt_info |= INTERRUPT_TYPE_NMI;
        } else if vector <= X86_INT_VIRT {
            // From Volume 3, Section 6.15. All other vectors from 0 to X86_INT_VIRT are exceptions.
            interrupt_info |= INTERRUPT_TYPE_HARDWARE_EXCEPTION;
        }
        if has_error_code(vector) {
            interrupt_info |= INTERRUPT_INFO_DELIVER_ERROR_CODE;
            self.write(VmcsField32::ENTRY_EXCEPTION_ERROR_CODE, 0);
        }

        debug_assert!((self.read(VmcsField32::ENTRY_INTERRUPTION_INFORMATION) & INTERRUPT_INFO_VALID) == 0);
        self.write(VmcsField32::ENTRY_INTERRUPTION_INFORMATION, interrupt_info);
    }

    pub fn read<T: VmcsFieldValue>(&self, field: T) -> T::Output {
        debug_assert!(self.vmcs_address != 0);
        T::read(field)
    }

    pub fn write<T: VmcsFieldValue>(&self, field: T, val: T::Output) {
        debug_assert!(self.vmcs_address != 0);
        T::write(field, val);
    }

    pub fn set_control(&self, controls: VmcsField32, true_msr: u64, old_msr: u64,
                     set: u32, clear: u32) -> rx_status_t {
        debug_assert!(self.vmcs_address != 0);
        let allowed_0 = bits::BITS(true_msr, 31, 0) as u32;
        let allowed_1 = bits::BITS_SHIFT(true_msr, 63, 32) as u32;
        if (allowed_1 & set) != set {
            dprintf!(INFO, "can not set vmcs controls {:#x}\n", controls as u32);
            return rx_ERR_NOT_SUPPORTED;
        }
        if (!allowed_0 & clear) != clear {
            dprintf!(INFO, "can not clear vmcs controls {:#x}\n", controls as u32);
            return rx_ERR_NOT_SUPPORTED;
        }
        if (set & clear) != 0 {
            dprintf!(INFO, "can not set and clear the same vmcs controls {:#x}\n",
                  controls as u32);
            return rx_ERR_INVALID_ARGS;
        }

        // See Volume 3, Section 31.5.1, Algorithm 3, Part C. If the control can be
        // either 0 or 1 (flexible), and the control is unknown, then refer to the
        // old MSR to find the default value.
        let flexible = allowed_0 ^ allowed_1;
        let unknown = flexible & !(set | clear);
        let defaults = unknown & (bits::BITS(old_msr, 31, 0) as u32);
        self.write(controls, allowed_0 | defaults | set);
        rx_OK
    }
}

impl Drop for AutoVmcs {
    fn drop(&mut self) {
        debug_assert!(arch_ints_disabled());
        unsafe { arch_enable_ints() };
    }
}

pub struct AutoPin {
    prev_cpu_mask: cpu_mask_t,
    thread: *mut thread_t,
}

impl AutoPin {
    pub fn new(vpid: u16) -> Self {
        let current_thread = get_current_thread();
        let prev_cpu_mask = unsafe { (*current_thread).cpu_affinity };
        let thread = cpu::hypervisor::pin_thread(vpid);
        
        Self {
            prev_cpu_mask,
            thread,
        }
    }
}

impl Drop for AutoPin {
    fn drop(&mut self) {
        thread_set_cpu_affinity(self.thread, self.prev_cpu_mask);
    }
}

fn ept_pointer(pml4_address: paddr_t) -> u64 {
    // Physical address of the PML4 page, page aligned.
    pml4_address |
    // Use write-back memory type for paging structures.
    (VMX_MEMORY_TYPE_WRITE_BACK << 0) |
    // Page walk length of 4 (defined as N minus 1).
    (3u64 << 3)
}

#[repr(C, packed)]
struct MsrListEntry {
    msr: u32,
    reserved: u32,
    value: u64,
}

fn edit_msr_list(msr_list_page: &mut VmxPage, index: usize, msr: u32, value: u64) {
    // From Volume 3, Section 24.7.2.

    // From Volume 3, Appendix A.6: Specifically, if the value bits 27:25 of
    // IA32_VMX_MISC is N, then 512 * (N + 1) is the recommended maximum number
    // of MSRs to be included in each list.
    //
    // From Volume 3, Section 24.7.2: This field specifies the number of MSRs to
    // be stored on VM exit. It is recommended that this count not exceed 512
    // bytes.
    //
    // Since these two statements conflict, we are taking the conservative
    // minimum and asserting that: index < (512 bytes / size of MsrListEntry).
    assert!(index < (512 / core::mem::size_of::<MsrListEntry>()));

    let entry = unsafe {
        let base = msr_list_page.virtual_address::<MsrListEntry>();
        &mut *base.add(index)
    };
    
    entry.msr = msr;
    entry.value = value;
}

fn has_error_code(vector: u32) -> bool {
    match vector {
        X86_INT_DOUBLE_FAULT | X86_INT_INVALID_TSS | X86_INT_SEGMENT_NOT_PRESENT | 
        X86_INT_STACK_FAULT | X86_INT_GP_FAULT | X86_INT_PAGE_FAULT | X86_INT_ALIGNMENT_CHECK => true,
        _ => false,
    }
}

// Injects an interrupt into the guest, if there is one pending.
fn local_apic_maybe_interrupt(vmcs: &AutoVmcs, local_apic_state: &mut LocalApicState) -> rx_status_t {
    // Since hardware generated exceptions are delivered to the guest directly, the only exceptions
    // we see here are those we generate in the VMM, e.g. GP faults in vmexit handlers. Therefore
    // we simplify interrupt priority to 1) NMIs, 2) interrupts, and 3) generated exceptions. See
    // Volume 3, Section 6.9, Table 6-2.
    let mut vector: u32 = 0;
    let mut type_ = local_apic_state.interrupt_tracker.try_pop(X86_INT_NMI);
    
    if type_ != hypervisor::InterruptType::INACTIVE {
        vector = X86_INT_NMI;
    } else {
        // Pop scans vectors from highest to lowest, which will correctly pop interrupts before
        // exceptions. All vectors <= X86_INT_VIRT except the NMI vector are exceptions.
        type_ = local_apic_state.interrupt_tracker.pop(&mut vector);
        
        if type_ == hypervisor::InterruptType::INACTIVE {
            return rx_OK;
        }
    }

    if vector > X86_INT_VIRT && vector < X86_INT_PLATFORM_BASE {
        dprintf!(INFO, "Invalid interrupt vector: {}\n", vector);
        return rx_ERR_NOT_SUPPORTED;
    } else if vector >= X86_INT_PLATFORM_BASE && 
        (vmcs.read(VmcsFieldXX::GUEST_RFLAGS) & X86_FLAGS_IF) == 0 {
        // Volume 3, Section 6.8.1: The IF flag does not affect non-maskable interrupts (NMIs),
        // [...] nor does it affect processor generated exceptions.
        local_apic_state.interrupt_tracker.track(vector, type_);
        // If interrupts are disabled, we set VM exit on interrupt enable.
        vmcs.interrupt_window_exiting(true);
        return rx_OK;
    }

    // If the vector is non-maskable or interrupts are enabled, we inject an interrupt.
    vmcs.issue_interrupt(vector);

    // Volume 3, Section 6.9: Lower priority exceptions are discarded; lower priority interrupts are
    // held pending. Discarded exceptions are re-generated when the interrupt handler returns
    // execution to the point in the program or task where the exceptions and/or interrupts
    // occurred.
    local_apic_state.interrupt_tracker.clear(0, X86_INT_NMI);
    local_apic_state.interrupt_tracker.clear(X86_INT_NMI + 1, X86_INT_VIRT + 1);

    rx_OK
}

pub struct Vcpu {
    guest: *mut Guest,
    vpid: u16,
    thread: *const thread_t,
    running: AtomicBool,
    vmx_state: VmxState,
    local_apic_state: LocalApicState,
    pvclock_state: PvClockState,
    vmcs_page: VmxPage,
    host_msr_page: VmxPage,
    guest_msr_page: VmxPage,
}

impl Vcpu {
    pub fn create(guest: &mut Guest, entry: rx_vaddr_t) -> Result<Box<Self>, rx_status_t> {
        let gpas = guest.address_space();
        if entry >= gpas.size() {
            return Err(rx_ERR_INVALID_ARGS);
        }

        let mut vpid: u16 = 0;
        let status = guest.alloc_vpid(&mut vpid);
        if status != rx_OK {
            return Err(status);
        }

        let auto_call = auto_call::make(|| {
            guest.free_vpid(vpid);
        });

        // When we create a VCPU, we bind it to the current thread and a CPU based
        // on the VPID. The VCPU must always be run on the current thread and the
        // given CPU, unless an explicit migration is performed.
        //
        // The reason we do this is that:
        // 1. The state of the current thread is stored within the VMCS, to be
        //    restored upon a guest-to-host transition.
        // 2. The state of the VMCS associated with the VCPU is cached within the
        //    CPU. To move to a different CPU, we must perform an explicit migration
        //    which will cost us performance.
        let thread = cpu::hypervisor::pin_thread(vpid);

        let mut vcpu = Box::new(Self {
            guest,
            vpid,
            thread,
            running: AtomicBool::new(false),
            vmx_state: VmxState::default(),
            local_apic_state: LocalApicState::default(),
            pvclock_state: PvClockState::default(),
            vmcs_page: VmxPage::new(),
            host_msr_page: VmxPage::new(),
            guest_msr_page: VmxPage::new(),
        });

        timer_init(&mut vcpu.local_apic_state.timer);
        let status = vcpu.local_apic_state.interrupt_tracker.init();
        if status != rx_OK {
            return Err(status);
        }

        vcpu.pvclock_state.is_stable = 
            if pvclock_is_present() { pvclock_is_stable() } 
            else { feature::x86_feature_test(feature::X86_FEATURE_INVAR_TSC) };

        let vmx_info = VmxInfo::new();
        let status = vcpu.host_msr_page.alloc(&vmx_info, 0);
        if status != rx_OK {
            return Err(status);
        }

        let status = vcpu.guest_msr_page.alloc(&vmx_info, 0);
        if status != rx_OK {
            return Err(status);
        }

        let status = vcpu.vmcs_page.alloc(&vmx_info, 0);
        if status != rx_OK {
            return Err(status);
        }
        
        auto_call.cancel();

        let region = vcpu.vmcs_page.virtual_address::<VmxRegion>();
        unsafe { region.as_mut().unwrap().revision_id = vmx_info.revision_id };
        
        let table = gpas.arch_aspace().arch_table_phys();
        let status = vmcs_init(
            vcpu.vmcs_page.physical_address(),
            vpid,
            entry as uintptr_t,
            guest.msr_bitmaps_address(),
            table,
            &mut vcpu.vmx_state,
            &mut vcpu.host_msr_page,
            &mut vcpu.guest_msr_page
        );
        
        if status != rx_OK {
            return Err(status);
        }

        Ok(vcpu)
    }

    pub fn resume(&mut self, packet: &mut rx_port_packet_t) -> rx_status_t {
        if !cpu::hypervisor::check_pinned_cpu_invariant(self.vpid, self.thread) {
            return rx_ERR_BAD_STATE;
        }
        
        let mut status;
        loop {
            let vmcs = AutoVmcs::new(self.vmcs_page.physical_address());
            status = local_apic_maybe_interrupt(&vmcs, &mut self.local_apic_state);
            if status != rx_OK {
                return status;
            }
            
            if feature::x86_feature_test(feature::X86_FEATURE_XSAVE) {
                // Save the host XCR0, and load the guest XCR0.
                self.vmx_state.host_state.xcr0 = x86_xgetbv(0);
                x86_xsetbv(0, self.vmx_state.guest_state.xcr0);
            }

            // Updates guest system time if the guest subscribed to updates.
            pvclock_update_system_time(&mut self.pvclock_state, self.guest.address_space());

            ktrace(TAG_VCPU_ENTER, 0, 0, 0, 0);
            self.running.store(true, Ordering::SeqCst);
            status = vmx_enter(&mut self.vmx_state);
            self.running.store(false, Ordering::SeqCst);
            
            if feature::x86_feature_test(feature::X86_FEATURE_XSAVE) {
                // Save the guest XCR0, and load the host XCR0.
                self.vmx_state.guest_state.xcr0 = x86_xgetbv(0);
                x86_xsetbv(0, self.vmx_state.host_state.xcr0);
            }

            if status != rx_OK {
                ktrace_vcpu_exit(VCPU_FAILURE, vmcs.read(VmcsFieldXX::GUEST_RIP));
                let error = vmcs.read(VmcsField32::INSTRUCTION_ERROR);
                dprintf!(INFO, "VCPU resume failed: {:#x}\n", error);
            } else {
                self.vmx_state.resume = true;
                status = vmexit_handler(
                    &vmcs,
                    &mut self.vmx_state.guest_state,
                    &mut self.local_apic_state,
                    &mut self.pvclock_state,
                    self.guest.address_space(),
                    self.guest.traps(),
                    packet
                );
            }
            
            if status != rx_OK {
                break;
            }
        }
        
        if status == rx_ERR_NEXT { rx_OK } else { status }
    }

    pub fn interrupt(&mut self, vector: u32, type_: hypervisor::InterruptType) -> cpu_mask_t {
        let mut signaled = false;
        self.local_apic_state.interrupt_tracker.interrupt(vector, type_, &mut signaled);
        if signaled || !self.running.load(Ordering::SeqCst) {
            return 0;
        }
        cpu_num_to_mask(cpu::hypervisor::cpu_of(self.vpid))
    }

    pub fn virtual_interrupt(&mut self, vector: u32) {
        let mask = self.interrupt(vector, hypervisor::InterruptType::VIRTUAL);
        if mask != 0 {
            mp::interrupt(MP_IPI_TARGET_MASK, mask);
        }
    }

    pub fn read_state(&self, kind: u32, buf: *mut libc::c_void, len: usize) -> rx_status_t {
        if !cpu::hypervisor::check_pinned_cpu_invariant(self.vpid, self.thread) {
            return rx_ERR_BAD_STATE;
        }
        
        match kind {
            rx_VCPU_STATE => {
                if len != core::mem::size_of::<rx_vcpu_state_t>() {
                    return rx_ERR_INVALID_ARGS;
                }
                
                let state = unsafe { &mut *(buf as *mut rx_vcpu_state_t) };
                register_copy(state, &self.vmx_state.guest_state);
                
                let vmcs = AutoVmcs::new(self.vmcs_page.physical_address());
                state.rsp = vmcs.read(VmcsFieldXX::GUEST_RSP);
                state.rflags = vmcs.read(VmcsFieldXX::GUEST_RFLAGS) & X86_FLAGS_USER;
                
                rx_OK
            }
            _ => rx_ERR_INVALID_ARGS,
        }
    }

    pub fn write_state(&mut self, kind: u32, buf: *const libc::c_void, len: usize) -> rx_status_t {
        if !cpu::hypervisor::check_pinned_cpu_invariant(self.vpid, self.thread) {
            return rx_ERR_BAD_STATE;
        }
        
        match kind {
            rx_VCPU_STATE => {
                if len != core::mem::size_of::<rx_vcpu_state_t>() {
                    return rx_ERR_INVALID_ARGS;
                }
                
                let state = unsafe { &*(buf as *const rx_vcpu_state_t) };
                register_copy(&mut self.vmx_state.guest_state, state);
                
                let vmcs = AutoVmcs::new(self.vmcs_page.physical_address());
                vmcs.write(VmcsFieldXX::GUEST_RSP, state.rsp);
                
                if state.rflags & X86_FLAGS_RESERVED_ONES != 0 {
                    let rflags = vmcs.read(VmcsFieldXX::GUEST_RFLAGS);
                    let user_flags = (rflags & !X86_FLAGS_USER) | (state.rflags & X86_FLAGS_USER);
                    vmcs.write(VmcsFieldXX::GUEST_RFLAGS, user_flags);
                }
                
                rx_OK
            }
            rx_VCPU_IO => {
                if len != core::mem::size_of::<rx_vcpu_io_t>() {
                    return rx_ERR_INVALID_ARGS;
                }
                
                let io = unsafe { &*(buf as *const rx_vcpu_io_t) };
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        io.data.as_ptr(),
                        &mut self.vmx_state.guest_state.rax as *mut u64 as *mut u8,
                        io.access_size as usize
                    );
                }
                
                rx_OK
            }
            _ => rx_ERR_INVALID_ARGS,
        }
    }
}

impl Drop for Vcpu {
    fn drop(&mut self) {
        if !self.vmcs_page.is_allocated() {
            return;
        }
        
        timer_cancel(&mut self.local_apic_state.timer);
        // The destructor may be called from a different thread, therefore we must
        // pin the current thread to the same CPU as the VCPU.
        let _pin = AutoPin::new(self.vpid);
        vmclear(self.vmcs_page.physical_address());
        
        let status = unsafe { (*self.guest).free_vpid(self.vpid) };
        debug_assert!(status == rx_OK);
    }
}

pub fn vmx_exit(vmx_state: &mut VmxState) {
    debug_assert!(arch_ints_disabled());

    // Reload the task segment in order to restore its limit. VMX always
    // restores it with a limit of 0x67, which excludes the IO bitmap.
    let selector = TSS_SELECTOR(arch_curr_cpu_num());
    x86_clear_tss_busy(selector);
    x86_ltr(selector);
}

pub fn cr0_is_invalid(vmcs: &AutoVmcs, cr0_value: u64) -> bool {
    let mut check_value = cr0_value;
    // From Volume 3, Section 26.3.1.1: PE and PG bits of CR0 are not checked when unrestricted
    // guest is enabled. Set both here to avoid clashing with X86_MSR_IA32_VMX_CR0_FIXED1.
    if vmcs.read(VmcsField32::PROCBASED_CTLS2) & PROCBASED_CTLS2_UNRESTRICTED_GUEST != 0 {
        check_value |= X86_CR0_PE | X86_CR0_PG;
    }
    cr_is_invalid(check_value, X86_MSR_IA32_VMX_CR0_FIXED0, X86_MSR_IA32_VMX_CR0_FIXED1)
}

fn register_copy<Out, In>(out: &mut Out, input: &In) 
where
    Out: RegisterAccess,
    In: RegisterAccess,
{
    out.set_rax(input.rax());
    out.set_rcx(input.rcx());
    out.set_rdx(input.rdx());
    out.set_rbx(input.rbx());
    out.set_rbp(input.rbp());
    out.set_rsi(input.rsi());
    out.set_rdi(input.rdi());
    out.set_r8(input.r8());
    out.set_r9(input.r9());
    out.set_r10(input.r10());
    out.set_r11(input.r11());
    out.set_r12(input.r12());
    out.set_r13(input.r13());
    out.set_r14(input.r14());
    out.set_r15(input.r15());
}