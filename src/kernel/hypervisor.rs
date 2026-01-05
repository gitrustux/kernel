// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Hypervisor Support
//!
//! This module provides hypervisor/virtualization support for the Rustux kernel.

#![no_std]

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::convert::TryFrom;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::kernel::sync::spin::SpinMutex;
use crate::rustux::types::*;
use crate::rustux::types::err::*;

/// Maximum number of guests
pub const MAX_GUESTS: usize = 64;

/// Maximum number of VCPUs per guest
pub const MAX_VCPUS: usize = 8;

/// Guest Physical Address Space
#[repr(C)]
pub struct GuestPhysicalAddressSpace {
    base: PAddr,
    size: usize,
}

impl GuestPhysicalAddressSpace {
    pub fn new(_base: PAddr, _size: usize) -> core::result::Result<Self, RxError> {
        Ok(Self {
            base: 0,
            size: 0,
        })
    }
}

/// Trap entry for handling guest exits
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Trap {
    pub kind: u32,
    pub addr: VAddr,
    pub len: usize,
    pub port: u16,
    pub key: u64,
}

/// Trap map for handling guest exits
#[repr(C)]
pub struct TrapMap {
    traps: SpinMutex<BTreeMap<u64, Trap>>,
}

impl TrapMap {
    pub fn new() -> Self {
        Self {
            traps: SpinMutex::new(BTreeMap::new()),
        }
    }

    pub fn insert(&self, key: u64, trap: Trap) -> Result<()> {
        let mut traps = self.traps.lock();
        traps.insert(key, trap);
        Ok(())
    }

    pub fn remove(&self, key: u64) -> core::result::Result<Option<Trap>, RxError> {
        let mut traps = self.traps.lock();
        Ok(traps.remove(&key))
    }
}

/// ID Allocator for managing IDs
#[repr(C)]
pub struct IdAllocator<T, const N: usize>
where
    T: Copy + Clone + PartialEq,
{
    next_id: SpinMutex<u64>,
    max_id: u64,
    bitmap: SpinMutex<[u64; N]>,
    _phantom: core::marker::PhantomData<T>,
}

impl<T, const N: usize> IdAllocator<T, N>
where
    T: Copy + Clone + PartialEq + TryFrom<u64>,
{
    pub fn new() -> Self {
        Self {
            next_id: SpinMutex::new(1),
            max_id: (N * 64) as u64,
            bitmap: SpinMutex::new([0u64; N]),
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn alloc(&self) -> Result<T> {
        let mut next_id = self.next_id.lock();
        let id = *next_id;
        *next_id = id + 1;
        T::try_from(id).map_err(|_| err(ERR_OUT_OF_RANGE, "ID overflow"))
    }

    pub fn free(&self, _id: T) -> Result<()> {
        Ok(())
    }
}

impl<T, const N: usize> Default for IdAllocator<T, N>
where
    T: Copy + Clone + PartialEq + TryFrom<u64>,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Port dispatcher for handling port I/O
#[repr(C)]
pub struct PortDispatcher {
    port: u16,
}

impl PortDispatcher {
    pub fn new(_port: u16) -> Self {
        Self { port: 0 }
    }
}

/// Interrupt tracker for managing virtual interrupts
#[repr(C)]
pub struct InterruptTracker<const N: usize> {
    bitmap: SpinMutex<[u64; N]>,
}

impl<const N: usize> InterruptTracker<N> {
    pub fn new() -> Self {
        Self {
            bitmap: SpinMutex::new([0u64; N]),
        }
    }

    pub fn track(&self, vector: u32) -> Result<()> {
        let index = (vector / 64) as usize;
        let bit = vector % 64;
        if index < N {
            let mut bitmap = self.bitmap.lock();
            bitmap[index] |= 1 << bit;
            Ok(())
        } else {
            Err(RX_ERR_INVALID_ARGS)
        }
    }
}

impl<const N: usize> Default for InterruptTracker<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Interrupt type enumeration
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptType {
    External = 0,
    Virtual = 1,
}

/// Guest pointer type for safe guest memory access
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GuestPtr {
    addr: usize,
}

impl GuestPtr {
    pub fn new(addr: usize) -> Self {
        Self { addr }
    }

    pub fn addr(&self) -> usize {
        self.addr
    }
}

impl Default for GuestPtr {
    fn default() -> Self {
        Self { addr: 0 }
    }
}
