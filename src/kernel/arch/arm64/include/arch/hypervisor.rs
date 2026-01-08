// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64::el2_state::{El2State};
use crate::fbl::{RefPtr, Mutex};
// Import from kernel::hypervisor module
use crate::kernel::hypervisor::{
    GuestPhysicalAddressSpace,
    InterruptTracker,
    TrapMap,
    IdAllocator,
};
use crate::kernel::{event, spinlock::*};
// TODO: ktl and bitmap modules don't exist yet - comment out for now
// use crate::ktl::unique_ptr::UniquePtr;
// use crate::bitmap::{RawBitmapGeneric, FixedStorage};
use crate::rustux::types::*;
use crate::rustux::types::err::*;
use crate::rustux::types::VAddr as rx_vaddr_t;
use crate::rustux::types::PAddr as paddr_t;
use crate::kernel::thread::Thread;

// Stub UniquePtr for ktl compatibility
pub struct UniquePtr<T> {
    _phantom: core::marker::PhantomData<T>,
}

impl<T> UniquePtr<T> {
    pub fn null() -> Self {
        Self { _phantom: core::marker::PhantomData }
    }
}

// Stub RawBitmapGeneric for bitmap compatibility
pub struct RawBitmapGeneric<S> {
    _phantom: core::marker::PhantomData<S>,
}

// Stub FixedStorage for bitmap compatibility
pub struct FixedStorage<const N: usize>;

// Stub Page type for VM compatibility
#[derive(Default)]
pub struct Page;

// See CoreLink GIC-400, Section 2.3.2 PPIs.
pub const MAINTENANCE_VECTOR: u32 = 25;
pub const TIMER_VECTOR: u32 = 27;
pub const NUM_INTERRUPTS: u16 = 256;

// Static assertions to ensure interrupt vectors are in range
const _: () = assert!(MAINTENANCE_VECTOR < NUM_INTERRUPTS as u32, "Maintenance vector is out of range");
const _: () = assert!(TIMER_VECTOR < NUM_INTERRUPTS as u32, "Timer vector is out of range");

pub type rx_port_packet_t = crate::rustux::types::rx_port_packet;
pub struct PortDispatcher;

pub struct Guest {
    gpas: UniquePtr<GuestPhysicalAddressSpace>,
    traps: TrapMap,
    vmid: u8,
    
    vcpu_mutex: Mutex<()>,
    // TODO(alexlegg): Find a good place for this constant to live (max vcpus).
    vpid_allocator: IdAllocator<u8, 8>,
}

impl Guest {
    pub fn create() -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    pub fn set_trap(&mut self, kind: u32, addr: rx_vaddr_t, len: usize,
                  port: RefPtr<PortDispatcher>, key: u64) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    pub fn address_space(&self) -> &GuestPhysicalAddressSpace {
        &self.gpas
    }
    
    pub fn traps(&mut self) -> &mut TrapMap {
        &mut self.traps
    }
    
    pub fn vmid(&self) -> u8 {
        self.vmid
    }
    
    pub fn alloc_vpid(&mut self) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    pub fn free_vpid(&mut self, vpid: u8) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    // Private constructor
    fn new(vmid: u8) -> Self {
        Self {
            gpas: UniquePtr::default(),
            traps: TrapMap::default(),
            vmid,
            vcpu_mutex: Mutex::new(()),
            vpid_allocator: IdAllocator::new(),
        }
    }
}

// Stores the state of the GICH across VM exits.
pub struct GichState {
    // Tracks pending interrupts.
    pub interrupt_tracker: InterruptTracker<{ NUM_INTERRUPTS as usize }>,
    // Tracks active interrupts.
    pub active_interrupts: RawBitmapGeneric<FixedStorage<{ NUM_INTERRUPTS as usize }>>,

    // GICH state to be restored between VM exits.
    pub num_aprs: u32,
    pub num_lrs: u32,
    pub vmcr: u32,
    pub elrsr: u64,
    pub apr: [u32; 4],
    pub lr: [u64; 64],
}

impl Default for GichState {
    fn default() -> Self {
        Self {
            interrupt_tracker: InterruptTracker::new(),
            active_interrupts: RawBitmapGeneric::new(),
            num_aprs: 0,
            num_lrs: 0,
            vmcr: 0,
            elrsr: 0,
            apr: [0; 4],
            lr: [0; 64],
        }
    }
}

// Loads a GICH within a given scope.
pub struct AutoGich<'a> {
    gich_state: &'a mut GichState,
}

impl<'a> AutoGich<'a> {
    pub fn new(gich_state: &'a mut GichState) -> Self {
        // Implementation would go here - loading GICH state
        Self { gich_state }
    }
}

impl<'a> Drop for AutoGich<'a> {
    fn drop(&mut self) {
        // Implementation would go here - saving GICH state
    }
}

// Provides a smart pointer to an El2State allocated in its own page.
//
// We allocate an El2State into its own page as the structure is passed between
// EL1 and EL2, which have different address spaces mappings. This ensures that
// El2State will not cross a page boundary and be incorrectly accessed in EL2.
pub struct El2StatePtr {
    page: Page,
    state: *mut El2State,
}

impl El2StatePtr {
    pub fn new() -> Self {
        Self {
            page: Page::default(),
            state: core::ptr::null_mut(),
        }
    }
    
    pub fn alloc(&mut self) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    pub fn physical_address(&self) -> paddr_t {
        self.page.physical_address()
    }
}

impl core::ops::Deref for El2StatePtr {
    type Target = El2State;
    
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.state }
    }
}

impl core::ops::DerefMut for El2StatePtr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.state }
    }
}

pub struct Vcpu {
    guest: *mut Guest,
    vpid: u8,
    thread: *const crate::kernel::thread::Thread, // Assuming thread_t maps to Thread in Rust
    running: core::sync::atomic::AtomicBool,
    gich_state: GichState,
    el2_state: El2StatePtr,
    hcr: u64,
}

// Hypervisor interrupt types
pub enum InterruptType {
    Virtual,
    Physical,
}

impl Vcpu {
    pub fn create(guest: &mut Guest, entry: rx_vaddr_t) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    pub fn resume(&mut self, packet: &mut rx_port_packet_t) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    pub fn interrupt(&mut self, vector: u32, interrupt_type: InterruptType) -> cpu_mask_t {
        // Implementation would go here
        0
    }
    
    pub fn virtual_interrupt(&mut self, vector: u32) {
        // Implementation would go here
    }
    
    pub fn read_state(&self, kind: u32, buf: *mut core::ffi::c_void, len: usize) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    pub fn write_state(&mut self, kind: u32, buf: *const core::ffi::c_void, len: usize) -> rx_status_t {
        // Implementation would go here
        RX_OK
    }
    
    // Private constructor
    fn new(guest: *mut Guest, vpid: u8, thread: *const crate::kernel::thread::Thread) -> Self {
        Self {
            guest,
            vpid,
            thread,
            running: core::sync::atomic::AtomicBool::new(false),
            gich_state: GichState::default(),
            el2_state: El2StatePtr::new(),
            hcr: 0,
        }
    }
}

// Ensure VCPU is Send to allow it to be sent between threads
unsafe impl Send for Vcpu {}