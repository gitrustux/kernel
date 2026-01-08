// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::kernel::arch::arm64::el2_state::{El2TranslationTable, El2Stack};
use crate::kernel::arch::arm64::mmu::{MMU_PTE_L012_DESCRIPTOR_TABLE, MMU_PTE_ATTR_AF, MMU_PTE_ATTR_SH_INNER_SHAREABLE, MMU_PTE_ATTR_AP_P_RW_U_RW, MMU_PTE_ATTR_NORMAL_MEMORY, MMU_PTE_L012_DESCRIPTOR_BLOCK};
use crate::kernel::arch::hypervisor::{rx_el2_on, rx_el2_off};
use crate::kernel::dev::interrupt::{mask_interrupt, unmask_interrupt, kMaintenanceVector, kTimerVector};
use crate::kernel::sync::Mutex;
use crate::kernel::task::percpu_exec;
use crate::kernel::vm::{PhysAddr, PAGE_SIZE};
use crate::kernel::syscalls::SyscallResult;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Global state for managing EL2 CPU state.
static GUEST_MUTEX: Mutex = Mutex::new();
static NUM_GUESTS: AtomicUsize = AtomicUsize::new(0);
static EL2_CPU_STATE: Mutex<Option<Arc<El2CpuState>>> = Mutex::new(None);

/// Represents an EL2 translation table.
pub struct El2TranslationTable {
    l0_page: Box<[u8; PAGE_SIZE]>,
    l1_page: Box<[u8; PAGE_SIZE]>,
}

impl El2TranslationTable {
    /// Initialize the EL2 translation table.
    pub fn init(&mut self) -> SyscallResult {
        // Allocate L0 and L1 pages.
        self.l0_page = Box::new([0; PAGE_SIZE]);
        self.l1_page = Box::new([0; PAGE_SIZE]);

        // L0: Point to a single L1 translation table.
        let l0_pte = &mut self.l0_page[0] as *mut _ as *mut u64;
        unsafe {
            *l0_pte = self.l1_page.as_ptr() as u64 | MMU_PTE_L012_DESCRIPTOR_TABLE;
        }

        // L1: Identity map the first 512GB of physical memory.
        let l1_pte = &mut self.l1_page[0] as *mut _ as *mut u64;
        for i in 0..PAGE_SIZE / core::mem::size_of::<u64>() {
            unsafe {
                *l1_pte.add(i) = (i as u64) * (1u64 << 30)
                    | MMU_PTE_ATTR_AF
                    | MMU_PTE_ATTR_SH_INNER_SHAREABLE
                    | MMU_PTE_ATTR_AP_P_RW_U_RW
                    | MMU_PTE_ATTR_NORMAL_MEMORY
                    | MMU_PTE_L012_DESCRIPTOR_BLOCK;
            }
        }

        // Ensure memory barriers.
        core::sync::atomic::fence(Ordering::SeqCst);
        SyscallResult::Ok(0)
    }

    /// Get the base address of the translation table.
    pub fn base(&self) -> PhysAddr {
        self.l0_page.as_ptr() as PhysAddr
    }
}

/// Represents an EL2 stack.
pub struct El2Stack {
    page: Box<[u8; PAGE_SIZE]>,
}

impl El2Stack {
    /// Allocate a new EL2 stack.
    pub fn alloc() -> SyscallResult<Self> {
        let page = Box::new([0; PAGE_SIZE]);
        Ok(Self { page })
    }

    /// Get the top of the stack.
    pub fn top(&self) -> PhysAddr {
        self.page.as_ptr() as PhysAddr + PAGE_SIZE
    }
}

/// Represents the EL2 CPU state.
pub struct El2CpuState {
    table: El2TranslationTable,
    stacks: Vec<El2Stack>,
    cpu_mask: usize,
}

impl El2CpuState {
    /// Initialize the EL2 CPU state.
    pub fn init(&mut self) -> SyscallResult {
        self.table.init()
    }

    /// Task to enable EL2 for a specific CPU.
    pub fn on_task(context: *mut Self, cpu_num: usize) -> SyscallResult {
        let cpu_state = unsafe { &mut *context };
        let table_base = cpu_state.table.base();
        let stack_top = cpu_state.stacks[cpu_num].top();
        let status = unsafe { rx_el2_on(table_base, stack_top) };
        if status.is_err() {
            println!("Failed to turn EL2 on for CPU {}", cpu_num);
            return status;
        }
        unmask_interrupt(kMaintenanceVector);
        unmask_interrupt(kTimerVector);
        SyscallResult::Ok(0)
    }

    /// Task to disable EL2 for a specific CPU.
    pub fn off_task() {
        mask_interrupt(kTimerVector);
        mask_interrupt(kMaintenanceVector);
        let status = unsafe { rx_el2_off() };
        if status.is_err() {
            println!("Failed to turn EL2 off for CPU {}", crate::kernel::arch::current_cpu_num());
        }
    }

    /// Create a new EL2 CPU state.
    pub fn create() -> SyscallResult<Arc<Self>> {
        let mut cpu_state = Box::new(Self {
            table: El2TranslationTable { l0_page: Box::new([0; PAGE_SIZE]), l1_page: Box::new([0; PAGE_SIZE]) },
            stacks: Vec::new(),
            cpu_mask: 0,
        });

        // Initialize the EL2 translation table.
        let status = cpu_state.table.init();
        if status.is_err() {
            return status;
        }

        // Allocate EL2 stacks for each CPU.
        let num_cpus = crate::kernel::arch::max_num_cpus();
        cpu_state.stacks = Vec::with_capacity(num_cpus);
        for _ in 0..num_cpus {
            let stack = El2Stack::alloc()?;
            cpu_state.stacks.push(stack);
        }

        // Enable EL2 for all online CPUs.
        cpu_state.cpu_mask = percpu_exec(Self::on_task, cpu_state.as_mut() as *mut _);
        if cpu_state.cpu_mask != crate::kernel::arch::mp_get_online_mask() {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported);
        }

        Ok(Arc::new(*cpu_state))
    }

    /// Allocate a VMID.
    pub fn alloc_id(&self) -> SyscallResult<u8> {
        // Placeholder for VMID allocation logic.
        SyscallResult::Ok(0)
    }

    /// Free a VMID.
    pub fn free_id(&self, _vmid: u8) -> SyscallResult {
        // Placeholder for VMID deallocation logic.
        SyscallResult::Ok(0)
    }
}

/// Allocate a VMID.
pub fn alloc_vmid() -> SyscallResult<u8> {
    let mut lock = GUEST_MUTEX.lock();
    if NUM_GUESTS.load(Ordering::SeqCst) == 0 {
        let cpu_state = El2CpuState::create()?;
        *EL2_CPU_STATE.lock() = Some(cpu_state);
    }
    NUM_GUESTS.fetch_add(1, Ordering::SeqCst);
    EL2_CPU_STATE.lock().as_ref().unwrap().alloc_id()
}

/// Free a VMID.
pub fn free_vmid(vmid: u8) -> SyscallResult {
    let mut lock = GUEST_MUTEX.lock();
    let cpu_state = EL2_CPU_STATE.lock().as_ref().unwrap();
    cpu_state.free_id(vmid)?;
    NUM_GUESTS.fetch_sub(1, Ordering::SeqCst);
    if NUM_GUESTS.load(Ordering::SeqCst) == 0 {
        *EL2_CPU_STATE.lock() = None;
    }
    SyscallResult::Ok(0)
}