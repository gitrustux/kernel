// Copyright 2023 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::kernel::arch::arm64::hypervisor::{alloc_vmid, free_vmid};
use crate::kernel::arch::hypervisor::{GuestPhysicalAddressSpace, TrapKind};
use crate::kernel::dev::interrupt::arm_gic_hw_interface::gic_get_gicv;
use crate::kernel::sync::Mutex;
use crate::kernel::syscalls::SyscallResult;
use crate::kernel::vm::{PhysAddr, VirtAddr, PAGE_SIZE, IS_PAGE_ALIGNED};
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU8, Ordering};

/// Base address for the GIC virtual interface.
const GICV_ADDRESS: VirtAddr = 0x800001000;
/// Size of the GIC virtual interface region.
const GICV_SIZE: usize = 0x2000;

/// Represents a guest virtual machine.
pub struct Guest {
    vmid: u8,
    gpas: Arc<GuestPhysicalAddressSpace>,
    traps: Mutex<TrapManager>,
    vpid_allocator: Mutex<IdAllocator>,
    vcpu_mutex: Mutex<()>,
}

impl Guest {
    /// Create a new guest virtual machine.
    pub fn create() -> SyscallResult<Arc<Self>> {
        // Check if the current EL is less than 2 (EL2 is required for virtualization).
        if crate::kernel::arch::arm64::get_boot_el() < 2 {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported);
        }

        // Allocate a VMID for the guest.
        let vmid = alloc_vmid()?;

        // Create the guest instance.
        let guest = Arc::new(Self {
            vmid,
            gpas: GuestPhysicalAddressSpace::create(vmid)?,
            traps: Mutex::new(TrapManager::new()),
            vpid_allocator: Mutex::new(IdAllocator::new()),
            vcpu_mutex: Mutex::new(()),
        });

        // Map the GIC virtual interface if running on GICv2.
        let gicv_paddr = gic_get_gicv()?;
        if gicv_paddr != 0 {
            guest.gpas.map_interrupt_controller(GICV_ADDRESS, gicv_paddr, GICV_SIZE)?;
        }

        Ok(guest)
    }

    /// Set a trap for the guest.
    pub fn set_trap(
        &self,
        kind: TrapKind,
        addr: VirtAddr,
        len: usize,
        port: Option<Arc<PortDispatcher>>,
        key: u64,
    ) -> SyscallResult {
        // Validate the trap kind and arguments.
        match kind {
            TrapKind::Memory => {
                if port.is_some() {
                    return SyscallResult::Err(crate::kernel::syscalls::SyscallError::InvalidArgs);
                }
            }
            TrapKind::Bell => {
                if port.is_none() {
                    return SyscallResult::Err(crate::kernel::syscalls::SyscallError::InvalidArgs);
                }
            }
            TrapKind::Io => {
                return SyscallResult::Err(crate::kernel::syscalls::SyscallError::NotSupported);
            }
        }

        // Validate the address and length.
        if addr.checked_add(len).is_none() {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::OutOfRange);
        }
        if !IS_PAGE_ALIGNED(addr) || !IS_PAGE_ALIGNED(len) || len == 0 {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::InvalidArgs);
        }

        // Unmap the range in the guest physical address space.
        self.gpas.unmap_range(addr, len)?;

        // Insert the trap.
        let mut traps = self.traps.lock();
        traps.insert_trap(kind, addr, len, port, key)
    }

    /// Allocate a VPID (Virtual Processor ID) for a VCPU.
    pub fn alloc_vpid(&self) -> SyscallResult<u8> {
        let mut allocator = self.vpid_allocator.lock();
        allocator.alloc_id()
    }

    /// Free a VPID (Virtual Processor ID) for a VCPU.
    pub fn free_vpid(&self, vpid: u8) -> SyscallResult {
        let mut allocator = self.vpid_allocator.lock();
        allocator.free_id(vpid)
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        // Free the VMID when the guest is dropped.
        free_vmid(self.vmid).unwrap();
    }
}

/// Manages traps for a guest.
struct TrapManager {
    // Placeholder for trap management logic.
}

impl TrapManager {
    fn new() -> Self {
        Self {}
    }

    fn insert_trap(
        &mut self,
        kind: TrapKind,
        addr: VirtAddr,
        len: usize,
        port: Option<Arc<PortDispatcher>>,
        key: u64,
    ) -> SyscallResult {
        // Placeholder for trap insertion logic.
        SyscallResult::Ok(0)
    }
}

/// Allocates unique IDs (e.g., VPIDs).
struct IdAllocator {
    next_id: AtomicU8,
}

impl IdAllocator {
    fn new() -> Self {
        Self {
            next_id: AtomicU8::new(0),
        }
    }

    fn alloc_id(&self) -> SyscallResult<u8> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        if id == u8::MAX {
            return SyscallResult::Err(crate::kernel::syscalls::SyscallError::NoResources);
        }
        SyscallResult::Ok(id)
    }

    fn free_id(&self, _id: u8) -> SyscallResult {
        // Placeholder for ID deallocation logic.
        SyscallResult::Ok(0)
    }
}