// Copyright Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::pvclock_priv::*;
use crate::arch::hypervisor;
use crate::arch::amd64::pvclock::{self, pvclock_boot_time, pvclock_system_time, PvClockOffset};
use crate::bits;
use crate::hypervisor::guest_physical_address_space::GuestPhysicalAddressSpace;
use crate::platform;
use crate::vm::physmap;
use crate::rustux::types::*;
use core::sync::atomic::{self, AtomicI64, AtomicU32, Ordering};

fn calculate_scale_factor(tsc_freq: u64, mul: &mut u32, shift: &mut i8) {
    // Guests converts TSC ticks to nanoseconds using this formula:
    //   ns = #TSCticks * mul * 2^(shift - 32).
    // mul * 2^(shift - 32) is a fractional number used as a scale factor in conversion.
    // It's very similar to how floating point numbers are usually represented in memory.
    static TARGET_FREQ: u64 = 1000000000;

    debug_assert!(tsc_freq != 0);

    // We maintain the following invariant: 2^(exponent - 32) * x/y ~ target_freq / tsc_freq,
    let mut exponent: i8 = 32;
    let mut x: u64 = TARGET_FREQ;
    let mut y: u64 = tsc_freq;

    // First make y small enough so that (y << 31) doesn't overflow in the next step. Adjust
    // exponent along the way to maintain invariant.
    while y >= (1u64 << 31) {
        y >>= 1;
        exponent -= 1;
    }

    // We scale x/y multiplying x by 2 until it gets big enough or we run out of bits.
    while x < (y << 31) && bits::BIT(x, 63) == 0 {
        x <<= 1;
        exponent -= 1;
    }

    // Though it's very unlikely lets also consider a situation when x/y is still too small.
    while x < y {
        y >>= 1;
        exponent += 1;
    }

    // Finally make sure that x/y fits within 32 bits.
    while x >= (y << 32) {
        x >>= 1;
        exponent += 1;
    }

    *shift = exponent;
    *mul = (x / y) as u32;
}

// External atomic UTC offset
extern "C" {
    static utc_offset: AtomicI64;
}

pub fn pvclock_update_boot_time(
    gpas: &mut GuestPhysicalAddressSpace,
    guest_paddr: rx_vaddr_t,
) -> rx_status_t {
    // KVM doesn't provide any protection against concurrent wall time requests from different
    // VCPUs, but documentation doesn't mention that it cannot happen and moreover it properly
    // protects per VCPU system time. Therefore to be on the safer side we use one global mutex
    // for protection.
    static MUTEX: Mutex<()> = Mutex::new(());
    static mut VERSION: u32 = 0;

    let mut guest_ptr = GuestPtr::new();
    let status = gpas.create_guest_ptr(
        guest_paddr,
        core::mem::size_of::<pvclock_boot_time>(),
        "pvclock-boot-time-guest-mapping",
        &mut guest_ptr,
    );
    if status != rx_OK {
        return status;
    }

    let boot_time = guest_ptr.as_mut::<pvclock_boot_time>().unwrap();
    unsafe {
        core::ptr::write_bytes(boot_time, 0, 1);
    }

    let _lock = MUTEX.lock();
    let time = unsafe { utc_offset.load(Ordering::Relaxed) };
    
    // See the comment for pvclock_boot_time structure in arch/amd64/pvclock.h
    let version = unsafe { VERSION };
    AtomicU32::from_ptr(&mut boot_time.version).store(version + 1, Ordering::Relaxed);
    atomic::fence(Ordering::SeqCst);
    
    boot_time.seconds = (time / rx_SEC(1)) as u32;
    boot_time.nseconds = (time % rx_SEC(1)) as u32;
    
    atomic::fence(Ordering::SeqCst);
    AtomicU32::from_ptr(&mut boot_time.version).store(version + 2, Ordering::Relaxed);
    
    unsafe {
        VERSION += 2;
    }
    
    rx_OK
}

pub fn pvclock_reset_clock(
    pvclock: &mut PvClockState,
    gpas: &mut GuestPhysicalAddressSpace,
    guest_paddr: rx_vaddr_t,
) -> rx_status_t {
    let status = gpas.create_guest_ptr(
        guest_paddr,
        core::mem::size_of::<pvclock_system_time>(),
        "pvclock-system-time-guest-mapping",
        &mut pvclock.guest_ptr,
    );
    
    if status != rx_OK {
        return status;
    }
    
    pvclock.system_time = pvclock.guest_ptr.as_mut::<pvclock_system_time>();
    if let Some(system_time) = pvclock.system_time.as_mut() {
        unsafe {
            core::ptr::write_bytes(system_time, 0, 1);
        }
    }
    
    rx_OK
}

pub fn pvclock_update_system_time(
    pvclock: &mut PvClockState,
    _gpas: &mut GuestPhysicalAddressSpace,
) {
    if pvclock.system_time.is_none() {
        return;
    }

    let mut tsc_mul: u32 = 0;
    let mut tsc_shift: i8 = 0;
    calculate_scale_factor(platform::ticks_per_second(), &mut tsc_mul, &mut tsc_shift);

    // See the comment for pvclock_boot_time structure in arch/amd64/pvclock.h
    let system_time = pvclock.system_time.as_mut().unwrap();
    
    AtomicU32::from_ptr(&mut system_time.version).store(pvclock.version + 1, Ordering::Relaxed);
    atomic::fence(Ordering::SeqCst);
    
    system_time.tsc_mul = tsc_mul;
    system_time.tsc_shift = tsc_shift;
    system_time.system_time = platform::current_time();
    system_time.tsc_timestamp = platform::rdtsc();
    system_time.flags = if pvclock.is_stable { pvclock::KVM_SYSTEM_TIME_STABLE } else { 0 };
    
    atomic::fence(Ordering::SeqCst);
    AtomicU32::from_ptr(&mut system_time.version).store(pvclock.version + 2, Ordering::Relaxed);
    
    pvclock.version += 2;
}

pub fn pvclock_stop_clock(pvclock: &mut PvClockState) {
    pvclock.system_time = None;
    pvclock.guest_ptr.reset();
}

pub fn pvclock_populate_offset(
    gpas: &mut GuestPhysicalAddressSpace,
    guest_paddr: rx_vaddr_t,
) -> rx_status_t {
    let mut guest_ptr = GuestPtr::new();
    let status = gpas.create_guest_ptr(
        guest_paddr,
        core::mem::size_of::<PvClockOffset>(),
        "pvclock-offset-guest-mapping",
        &mut guest_ptr,
    );
    
    if status != rx_OK {
        return status;
    }
    
    let offset = guest_ptr.as_mut::<PvClockOffset>().unwrap();
    unsafe {
        core::ptr::write_bytes(offset, 0, 1);
    }
    
    let time = unsafe { utc_offset.load(Ordering::Relaxed) } + platform::current_time();
    let tsc = platform::rdtsc();
    
    offset.sec = time / rx_SEC(1);
    offset.nsec = time % rx_SEC(1);
    offset.tsc = tsc;
    
    rx_OK
}