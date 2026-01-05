// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::kernel::arch::arm64::hypervisor::{Guest, El2State, GichState, VcpuExit};
use crate::kernel::arch::hypervisor::{GuestPhysicalAddressSpace, InterruptType, VcpuState};
use crate::kernel::dev::interrupt::arm_gic_hw_interface::{gic_write_gich_hcr, gic_write_gich_vmcr, gic_write_gich_apr, gic_write_gich_lr, gic_read_gich_misr, gic_read_gich_vmcr, gic_read_gich_elrsr, gic_read_gich_apr, gic_read_gich_lr, gic_default_gich_vmcr, gic_get_num_pres, gic_get_num_lrs};
use crate::kernel::sync::Mutex;
use crate::kernel::task::{Thread, pin_thread};
use crate::kernel::syscalls::SyscallResult;
use crate::kernel::vm::{PhysAddr, VirtAddr};
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

/// Constants for GICH (Generic Interrupt Controller Hypervisor) registers.
const GICH_HCR_EN: u32 = 1 << 0;
const GICH_HCR_UIE: u32 = 1 << 1;
const GICH_MISR_U: u32 = 1 << 1;
const SPSR_DAIF: u32 = 0b1111 << 6;
const SPSR_EL1H: u32 = 0b0101;
const SPSR_NZCV: u32 = 0b1111 << 28;

/// Represents a virtual CPU (VCPU).
pub struct Vcpu {
    guest: Arc<Guest>,
    vpid: u8,
    thread: Arc<Thread>,
    running: AtomicBool,
    el2_state: El2State,
    gich_state: GichState,
    hcr: u64,
}

impl Vcpu {
    /// Create a new VCPU.
    pub fn create(guest: Arc<Guest>, entry: VirtAddr) -> SyscallResult<Arc<Self>> {
        let gpas = guest.address_space();
        if entry >= gpas.size() {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::InvalidArgs);
        }

        // Allocate a VPID for the VCPU.
        let vpid = guest.alloc_vpid()?;
        let auto_cleanup = scopeguard::guard((guest.clone(), vpid), |(guest, vpid)| {
            guest.free_vpid(vpid).unwrap();
        });

        // Pin the thread to the CPU.
        let thread = pin_thread(vpid);

        // Create the VCPU instance.
        let vcpu = Arc::new(Self {
            guest: guest.clone(),
            vpid,
            thread: thread.clone(),
            running: AtomicBool::new(false),
            el2_state: El2State::new(),
            gich_state: GichState::new(),
            hcr: 0,
        });

        // Initialize the GICH state.
        vcpu.gich_state.interrupt_tracker.init()?;
        vcpu.el2_state.alloc()?;

        // Configure the GICH state.
        vcpu.gich_state.active_interrupts.reset(kNumInterrupts);
        vcpu.gich_state.num_aprs = num_aprs(gic_get_num_pres());
        vcpu.gich_state.num_lrs = gic_get_num_lrs();
        vcpu.gich_state.vmcr = gic_default_gich_vmcr();
        vcpu.gich_state.elrsr = (1 << gic_get_num_lrs()) - 1;

        // Set up the guest state.
        vcpu.el2_state.guest_state.system_state.elr_el2 = entry;
        vcpu.el2_state.guest_state.system_state.spsr_el2 = SPSR_DAIF | SPSR_EL1H;
        let mpidr = unsafe { core::arch::asm!("mrs {}, mpidr_el1", out(reg) _) };
        vcpu.el2_state.guest_state.system_state.vmpidr_el2 = vmpidr_of(vpid, mpidr);
        vcpu.el2_state.host_state.system_state.vmpidr_el2 = mpidr;
        vcpu.hcr = HCR_EL2_VM | HCR_EL2_PTW | HCR_EL2_FMO | HCR_EL2_IMO | HCR_EL2_DC | HCR_EL2_TWI
            | HCR_EL2_TWE | HCR_EL2_TSC | HCR_EL2_TVM | HCR_EL2_RW;

        // Cancel the auto-cleanup guard.
        scopeguard::ScopeGuard::into_inner(auto_cleanup);

        Ok(vcpu)
    }

    /// Resume execution of the VCPU.
    pub fn resume(&self, packet: &mut VcpuPacket) -> SyscallResult {
        if !self.guest.check_pinned_cpu_invariant(self.vpid, &self.thread) {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::BadState);
        }

        let aspace = self.guest.address_space().arch_aspace();
        let vttbr = arm64_vttbr(aspace.arch_asid(), aspace.arch_table_phys());
        let guest_state = &mut self.el2_state.guest_state;

        loop {
            self.timer_maybe_interrupt(guest_state, &self.gich_state);
            self.gich_maybe_interrupt(&self.gich_state);

            {
                let _auto_gich = AutoGich::new(&self.gich_state);

                self.running.store(true, Ordering::SeqCst);
                let status = unsafe { rx_el2_resume(vttbr, self.el2_state.physical_address(), self.hcr) };
                self.running.store(false, Ordering::SeqCst);

                if status == SyscallResult::Err(crate::kernel::syscalls::SyscallError::Next) {
                    // Handle physical interrupt.
                    if self.thread.signals() & THREAD_SIGNAL_KILL != 0 {
                        return SyscallResult::Err(crate::kernel::syscalls::SyscallError::Canceled);
                    }
                    continue;
                } else if status.is_ok() {
                    return self.vmexit_handler(&self.hcr, guest_state, &self.gich_state, packet);
                } else {
                    return status;
                }
            }
        }
    }

    /// Handle an interrupt for the VCPU.
    pub fn interrupt(&self, vector: u32, kind: InterruptType) -> cpu_mask_t {
        let mut signaled = false;
        self.gich_state.interrupt_tracker.interrupt(vector, kind, &mut signaled);
        if signaled || !self.running.load(Ordering::SeqCst) {
            return 0;
        }
        cpu_num_to_mask(hypervisor::cpu_of(self.vpid))
    }

    /// Handle a virtual interrupt for the VCPU.
    pub fn virtual_interrupt(&self, vector: u32) {
        let mask = self.interrupt(vector, InterruptType::Virtual);
        if mask != 0 {
            mp_interrupt(MP_IPI_TARGET_MASK, mask);
        }
    }

    /// Read the VCPU state.
    pub fn read_state(&self, kind: u32, buf: &mut [u8]) -> SyscallResult {
        if !self.guest.check_pinned_cpu_invariant(self.vpid, &self.thread) {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::BadState);
        } else if kind != RX_VCPU_STATE || buf.len() != core::mem::size_of::<rx_vcpu_state_t>() {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::InvalidArgs);
        }

        let state = unsafe { &mut *(buf.as_mut_ptr() as *mut rx_vcpu_state_t) };
        state.x.copy_from_slice(&self.el2_state.guest_state.x);
        state.sp = self.el2_state.guest_state.system_state.sp_el1;
        state.cpsr = self.el2_state.guest_state.system_state.spsr_el2 & SPSR_NZCV;
        SyscallResult::Ok(0)
    }

    /// Write the VCPU state.
    pub fn write_state(&self, kind: u32, buf: &[u8]) -> SyscallResult {
        if !self.guest.check_pinned_cpu_invariant(self.vpid, &self.thread) {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::BadState);
        } else if kind != RX_VCPU_STATE || buf.len() != core::mem::size_of::<rx_vcpu_state_t>() {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::InvalidArgs);
        }

        let state = unsafe { &*(buf.as_ptr() as *const rx_vcpu_state_t) };
        self.el2_state.guest_state.x.copy_from_slice(&state.x);
        self.el2_state.guest_state.system_state.sp_el1 = state.sp;
        self.el2_state.guest_state.system_state.spsr_el2 |= state.cpsr & SPSR_NZCV;
        SyscallResult::Ok(0)
    }
}

impl Drop for Vcpu {
    fn drop(&mut self) {
        let _ = self.guest.free_vpid(self.vpid);
    }
}

/// Automatically manage GICH state.
struct AutoGich<'a> {
    gich_state: &'a mut GichState,
}

impl<'a> AutoGich<'a> {
    fn new(gich_state: &'a mut GichState) -> Self {
        unsafe { core::arch::asm!("msr daifset, #2") }; // Disable interrupts
        gic_write_gich_vmcr(gich_state.vmcr);
        for i in 0..gich_state.num_aprs {
            gic_write_gich_apr(i, gich_state.apr[i]);
        }
        for i in 0..gich_state.num_lrs {
            gic_write_gich_lr(i, gich_state.lr[i]);
        }

        let gich_hcr = if gich_state.interrupt_tracker.pending() && gich_state.num_lrs > 1 {
            GICH_HCR_EN | GICH_HCR_UIE
        } else {
            GICH_HCR_EN
        };
        gic_write_gich_hcr(gich_hcr);

        Self { gich_state }
    }
}

impl<'a> Drop for AutoGich<'a> {
    fn drop(&mut self) {
        self.gich_state.vmcr = gic_read_gich_vmcr();
        self.gich_state.elrsr = gic_read_gich_elrsr();
        for i in 0..self.gich_state.num_aprs {
            self.gich_state.apr[i] = gic_read_gich_apr(i);
        }
        for i in 0..self.gich_state.num_lrs {
            self.gich_state.lr[i] = if (self.gich_state.elrsr & (1 << i)) == 0 {
                gic_read_gich_lr(i)
            } else {
                0
            };
        }
        unsafe { core::arch::asm!("msr daifclr, #2") }; // Enable interrupts
    }
}