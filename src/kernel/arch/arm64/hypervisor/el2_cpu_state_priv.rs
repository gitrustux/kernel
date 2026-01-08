// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::kernel::arch::hypervisor::{IdAllocator, Page};
use crate::kernel::syscalls::SyscallResult;
use crate::kernel::vm::PhysAddr;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU8, Ordering};

/// Represents an EL2 translation table.
pub struct El2TranslationTable {
    l0_page: Box<Page>,
    l1_page: Box<Page>,
}

impl El2TranslationTable {
    /// Initialize the EL2 translation table.
    pub fn init(&mut self) -> SyscallResult {
        // Allocate L0 and L1 pages.
        self.l0_page = Box::new(Page::alloc()?);
        self.l1_page = Box::new(Page::alloc()?);

        // L0: Point to a single L1 translation table.
        let l0_pte = self.l0_page.virtual_address::<u64>();
        unsafe { *l0_pte = self.l1_page.physical_address() | MMU_PTE_L012_DESCRIPTOR_TABLE };

        // L1: Identity map the first 512GB of physical memory.
        let l1_pte = self.l1_page.virtual_address::<u64>();
        for i in 0..Page::SIZE / core::mem::size_of::<u64>() {
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
        self.l0_page.physical_address()
    }
}

/// Represents an EL2 stack.
pub struct El2Stack {
    page: Box<Page>,
}

impl El2Stack {
    /// Allocate a new EL2 stack.
    pub fn alloc(&mut self) -> SyscallResult {
        self.page = Box::new(Page::alloc()?);
        SyscallResult::Ok(0)
    }

    /// Get the top of the stack.
    pub fn top(&self) -> PhysAddr {
        self.page.physical_address() + Page::SIZE
    }
}

/// Maintains the EL2 state for each CPU.
pub struct El2CpuState {
    cpu_mask: u64,
    table: El2TranslationTable,
    stacks: Vec<El2Stack>,
    id_allocator: IdAllocator<u8, 64>,
}

impl El2CpuState {
    /// Create a new EL2 CPU state.
    pub fn create() -> SyscallResult<Box<Self>> {
        let mut cpu_state = Box::new(Self {
            cpu_mask: 0,
            table: El2TranslationTable {
                l0_page: Box::new(Page::alloc()?),
                l1_page: Box::new(Page::alloc()?),
            },
            stacks: Vec::new(),
            id_allocator: IdAllocator::new(),
        });

        // Initialize the EL2 translation table.
        cpu_state.table.init()?;

        // Allocate EL2 stacks for each CPU.
        let num_cpus = crate::kernel::arch::max_num_cpus();
        cpu_state.stacks = Vec::with_capacity(num_cpus);
        for _ in 0..num_cpus {
            let mut stack = El2Stack { page: Box::new(Page::alloc()?) };
            stack.alloc()?;
            cpu_state.stacks.push(stack);
        }

        // Enable EL2 for all online CPUs.
        cpu_state.cpu_mask = crate::kernel::arch::mp_online_mask();
        Ok(cpu_state)
    }

    /// Task to enable EL2 for a specific CPU.
    pub fn on_task(&self, cpu_num: u32) -> SyscallResult {
        let table_base = self.table.base();
        let stack_top = self.stacks[cpu_num as usize].top();
        unsafe { rx_el2_on(table_base, stack_top) }
    }

    /// Allocate a VMID.
    pub fn alloc_vmid(&mut self) -> SyscallResult<u8> {
        self.id_allocator.alloc()
    }

    /// Free a VMID.
    pub fn free_vmid(&mut self, vmid: u8) -> SyscallResult {
        self.id_allocator.free(vmid)
    }
}

impl Drop for El2CpuState {
    fn drop(&mut self) {
        // Clean up EL2 state for all CPUs.
        crate::kernel::arch::mp_sync_exec(self.cpu_mask, |_| {
            unsafe { rx_el2_off() };
        });
    }
}

/// Allocate a VMID.
pub fn alloc_vmid() -> SyscallResult<u8> {
    static EL2_CPU_STATE: Mutex<Option<Box<El2CpuState>>> = Mutex::new(None);
    static NUM_GUESTS: AtomicUsize = AtomicUsize::new(0);

    let mut lock = EL2_CPU_STATE.lock();
    if NUM_GUESTS.load(Ordering::SeqCst) == 0 {
        *lock = Some(El2CpuState::create()?);
    }
    NUM_GUESTS.fetch_add(1, Ordering::SeqCst);
    lock.as_mut().unwrap().alloc_vmid()
}

/// Free a VMID.
pub fn free_vmid(vmid: u8) -> SyscallResult {
    static EL2_CPU_STATE: Mutex<Option<Box<El2CpuState>>> = Mutex::new(None);
    static NUM_GUESTS: AtomicUsize = AtomicUsize::new(0);

    let mut lock = EL2_CPU_STATE.lock();
    let cpu_state = lock.as_mut().unwrap();
    cpu_state.free_vmid(vmid)?;
    NUM_GUESTS.fetch_sub(1, Ordering::SeqCst);
    if NUM_GUESTS.load(Ordering::SeqCst) == 0 {
        *lock = None;
    }
    SyscallResult::Ok(0)
}

/// Allocate a VPID.
pub fn alloc_vpid() -> SyscallResult<u8> {
    static VPID_ALLOCATOR: IdAllocator<u8, 64> = IdAllocator::new();
    VPID_ALLOCATOR.alloc()
}

/// Free a VPID.
pub fn free_vpid(vpid: u8) -> SyscallResult {
    static VPID_ALLOCATOR: IdAllocator<u8, 64> = IdAllocator::new();
    VPID_ALLOCATOR.free(vpid)
}