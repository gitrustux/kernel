// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

use crate::arch::arm64::mmu;
use core::mem;
use core::ptr::{self, addr_of_mut};
use crate::sys::types::*;
use crate::vm::bootalloc;
use crate::vm::physmap;
use crate::rustux::errors::*;

// Early boot time page table creation code, called from start.S while running in physical address space
// with the mmu disabled. This code should be position independent as long as it sticks to basic code.

// this code only works on a 4K page granule, 48 bits of kernel address space
const _: () = assert!(mmu::MMU_KERNEL_PAGE_SIZE_SHIFT == 12, "");
const _: () = assert!(mmu::MMU_KERNEL_SIZE_SHIFT == 48, "");

// 1GB pages
const L1_LARGE_PAGE_SIZE: usize = 1 << mmu::MMU_LX_X(mmu::MMU_KERNEL_PAGE_SIZE_SHIFT, 1);
const L1_LARGE_PAGE_SIZE_MASK: usize = L1_LARGE_PAGE_SIZE - 1;

// 2MB pages
const L2_LARGE_PAGE_SIZE: usize = 1 << mmu::MMU_LX_X(mmu::MMU_KERNEL_PAGE_SIZE_SHIFT, 2);
const L2_LARGE_PAGE_SIZE_MASK: usize = L2_LARGE_PAGE_SIZE - 2;

fn vaddr_to_l0_index(addr: usize) -> usize {
    (addr >> mmu::MMU_KERNEL_TOP_SHIFT) & (mmu::MMU_KERNEL_PAGE_TABLE_ENTRIES_TOP - 1)
}

fn vaddr_to_l1_index(addr: usize) -> usize {
    (addr >> mmu::MMU_LX_X(mmu::MMU_KERNEL_PAGE_SIZE_SHIFT, 1)) & (mmu::MMU_KERNEL_PAGE_TABLE_ENTRIES - 1)
}

fn vaddr_to_l2_index(addr: usize) -> usize {
    (addr >> mmu::MMU_LX_X(mmu::MMU_KERNEL_PAGE_SIZE_SHIFT, 2)) & (mmu::MMU_KERNEL_PAGE_TABLE_ENTRIES - 1)
}

fn vaddr_to_l3_index(addr: usize) -> usize {
    (addr >> mmu::MMU_LX_X(mmu::MMU_KERNEL_PAGE_SIZE_SHIFT, 3)) & (mmu::MMU_KERNEL_PAGE_TABLE_ENTRIES - 1)
}

// called from start.S to grab another page to back a page table from the boot allocator
#[no_mangle]
#[no_sanitize(address, memory, thread)]
pub extern "C" fn boot_alloc_ptable() -> *mut mmu::pte_t {
    // allocate a page out of the boot allocator, asking for a physical address
    let ptr = bootalloc::boot_alloc_page_phys() as *mut mmu::pte_t;

    // avoid using memset, since this relies on dc zva instruction, which isn't set up at
    // this point in the boot process
    // use a volatile pointer to make sure writes aren't optimized away
    let vptr = ptr as *mut core::cell::UnsafeCell<mmu::pte_t>;
    for i in 0..mmu::MMU_KERNEL_PAGE_TABLE_ENTRIES {
        unsafe {
            core::ptr::write_volatile((*vptr.add(i)).get(), 0);
        }
    }

    ptr
}

// inner mapping routine passed two helper routines
#[no_sanitize(address, memory, thread)]
unsafe fn _arm64_boot_map<F, G>(kernel_table0: *mut mmu::pte_t,
                             vaddr: vaddr_t,
                             paddr: paddr_t,
                             len: usize,
                             flags: mmu::pte_t,
                             mut alloc_func: F,
                             mut phys_to_virt: G) -> rx_status_t 
where
    F: FnMut() -> paddr_t,
    G: FnMut(paddr_t) -> *mut mmu::pte_t
{
    // loop through the virtual range and map each physical page, using the largest
    // page size supported. Allocates necessary page tables along the way.
    let mut off = 0;
    while off < len {
        // make sure the level 1 pointer is valid
        let index0 = vaddr_to_l0_index(vaddr + off);
        let mut kernel_table1: *mut mmu::pte_t = ptr::null_mut();
        
        match *kernel_table0.add(index0) & mmu::MMU_PTE_DESCRIPTOR_MASK {
            // invalid/unused entry
            _ if (*kernel_table0.add(index0) & mmu::MMU_PTE_DESCRIPTOR_MASK) != mmu::MMU_PTE_L012_DESCRIPTOR_TABLE &&
                 (*kernel_table0.add(index0) & mmu::MMU_PTE_DESCRIPTOR_MASK) != mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK => {
                
                let pa = alloc_func();
                *kernel_table0.add(index0) = (pa & mmu::MMU_PTE_OUTPUT_ADDR_MASK) |
                                           mmu::MMU_PTE_L012_DESCRIPTOR_TABLE;
                
                kernel_table1 = phys_to_virt(pa);
            }
            
            mmu::MMU_PTE_L012_DESCRIPTOR_TABLE => {
                kernel_table1 = phys_to_virt(*kernel_table0.add(index0) & mmu::MMU_PTE_OUTPUT_ADDR_MASK);
            }
            
            mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK => {
                // not legal to have a block pointer at this level
                return RX_ERR_BAD_STATE;
            }
            
            _ => return RX_ERR_BAD_STATE,
        }

        // make sure the level 2 pointer is valid
        let index1 = vaddr_to_l1_index(vaddr + off);
        let mut kernel_table2: *mut mmu::pte_t = ptr::null_mut();
        
        match *kernel_table1.add(index1) & mmu::MMU_PTE_DESCRIPTOR_MASK {
            // invalid/unused entry
            _ if (*kernel_table1.add(index1) & mmu::MMU_PTE_DESCRIPTOR_MASK) != mmu::MMU_PTE_L012_DESCRIPTOR_TABLE &&
                 (*kernel_table1.add(index1) & mmu::MMU_PTE_DESCRIPTOR_MASK) != mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK => {
                
                // a large page at this level is 1GB long, see if we can make one here
                if (((vaddr + off) & L1_LARGE_PAGE_SIZE_MASK) == 0) &&
                   (((paddr + off) & L1_LARGE_PAGE_SIZE_MASK) == 0) &&
                   (len - off) >= L1_LARGE_PAGE_SIZE {
                    
                    // set up a 1GB page here
                    *kernel_table1.add(index1) = ((paddr + off) & !L1_LARGE_PAGE_SIZE_MASK) |
                                               flags | mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK;
                    
                    off += L1_LARGE_PAGE_SIZE;
                    continue;
                }
                
                let pa = alloc_func();
                *kernel_table1.add(index1) = (pa & mmu::MMU_PTE_OUTPUT_ADDR_MASK) |
                                           mmu::MMU_PTE_L012_DESCRIPTOR_TABLE;
                
                kernel_table2 = phys_to_virt(pa);
            }
            
            mmu::MMU_PTE_L012_DESCRIPTOR_TABLE => {
                kernel_table2 = phys_to_virt(*kernel_table1.add(index1) & mmu::MMU_PTE_OUTPUT_ADDR_MASK);
            }
            
            mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK => {
                // block pointer at level 1 is a 1GB mapping, which is valid
                off += L1_LARGE_PAGE_SIZE;
                continue;
            }
            
            _ => return RX_ERR_BAD_STATE,
        }

        // make sure the level 3 pointer is valid
        let index2 = vaddr_to_l2_index(vaddr + off);
        let mut kernel_table3: *mut mmu::pte_t = ptr::null_mut();
        
        match *kernel_table2.add(index2) & mmu::MMU_PTE_DESCRIPTOR_MASK {
            // invalid/unused entry
            _ if (*kernel_table2.add(index2) & mmu::MMU_PTE_DESCRIPTOR_MASK) != mmu::MMU_PTE_L012_DESCRIPTOR_TABLE &&
                 (*kernel_table2.add(index2) & mmu::MMU_PTE_DESCRIPTOR_MASK) != mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK => {
                
                // a large page at this level is 2MB long, see if we can make one here
                if (((vaddr + off) & L2_LARGE_PAGE_SIZE_MASK) == 0) &&
                   (((paddr + off) & L2_LARGE_PAGE_SIZE_MASK) == 0) &&
                   (len - off) >= L2_LARGE_PAGE_SIZE {
                    
                    // set up a 2MB page here
                    *kernel_table2.add(index2) = ((paddr + off) & !L2_LARGE_PAGE_SIZE_MASK) |
                                               flags | mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK;
                    
                    off += L2_LARGE_PAGE_SIZE;
                    continue;
                }
                
                let pa = alloc_func();
                *kernel_table2.add(index2) = (pa & mmu::MMU_PTE_OUTPUT_ADDR_MASK) |
                                           mmu::MMU_PTE_L012_DESCRIPTOR_TABLE;
                
                kernel_table3 = phys_to_virt(pa);
            }
            
            mmu::MMU_PTE_L012_DESCRIPTOR_TABLE => {
                kernel_table3 = phys_to_virt(*kernel_table2.add(index2) & mmu::MMU_PTE_OUTPUT_ADDR_MASK);
            }
            
            mmu::MMU_PTE_L012_DESCRIPTOR_BLOCK => {
                // block pointer at level 2 is a 2MB mapping, which is valid
                off += L2_LARGE_PAGE_SIZE;
                continue;
            }
            
            _ => return RX_ERR_BAD_STATE,
        }

        // generate a standard page mapping
        let index3 = vaddr_to_l3_index(vaddr + off);
        *kernel_table3.add(index3) = ((paddr + off) & mmu::MMU_PTE_OUTPUT_ADDR_MASK) | 
                                   flags | mmu::MMU_PTE_L3_DESCRIPTOR_PAGE;
        
        off += PAGE_SIZE;
    }

    RX_OK
}

// called from start.S to configure level 1-3 page tables to map the kernel wherever it is located physically
// to KERNEL_BASE
#[no_mangle]
#[no_sanitize(address, memory, thread)]
pub extern "C" fn arm64_boot_map(kernel_table0: *mut mmu::pte_t,
                              vaddr: vaddr_t,
                              paddr: paddr_t,
                              len: usize,
                              flags: mmu::pte_t) -> rx_status_t {
    
    // the following helper routines assume that code is running in physical addressing mode (mmu off).
    // any physical addresses calculated are assumed to be the same as virtual
    unsafe {
        let alloc = || -> paddr_t {
            // allocate a page out of the boot allocator, asking for a physical address
            let pa = bootalloc::boot_alloc_page_phys();
            
            // avoid using memset, since this relies on dc zva instruction, which isn't set up at
            // this point in the boot process
            // use a volatile pointer to make sure the compiler doesn't emit a memset call
            let vptr = pa as *mut core::cell::UnsafeCell<mmu::pte_t>;
            for i in 0..mmu::MMU_KERNEL_PAGE_TABLE_ENTRIES {
                core::ptr::write_volatile((*vptr.add(i)).get(), 0);
            }
            
            pa
        };
        
        let phys_to_virt = |pa: paddr_t| -> *mut mmu::pte_t {
            pa as *mut mmu::pte_t
        };
        
        _arm64_boot_map(kernel_table0, vaddr, paddr, len, flags, alloc, phys_to_virt)
    }
}

// called a bit later in the boot process once the kernel is in virtual memory to map early kernel data
#[no_mangle]
pub extern "C" fn arm64_boot_map_v(vaddr: vaddr_t,
                                paddr: paddr_t,
                                len: usize,
                                flags: mmu::pte_t) -> rx_status_t {
    
    // assumed to be running with virtual memory enabled, so use a slightly different set of routines
    // to allocate and find the virtual mapping of memory
    unsafe {
        let alloc = || -> paddr_t {
            // allocate a page out of the boot allocator, asking for a physical address
            let pa = bootalloc::boot_alloc_page_phys();
            
            // zero the memory using the physmap
            let ptr = physmap::paddr_to_physmap(pa) as *mut mmu::pte_t;
            ptr::write_bytes(ptr, 0, mmu::MMU_KERNEL_PAGE_TABLE_ENTRIES);
            
            pa
        };
        
        let phys_to_virt = |pa: paddr_t| -> *mut mmu::pte_t {
            physmap::paddr_to_physmap(pa) as *mut mmu::pte_t
        };
        
        _arm64_boot_map(mmu::arm64_get_kernel_ptable(), vaddr, paddr, len, flags, alloc, phys_to_virt)
    }
}

extern "C" {
    // External C functions
    fn vm_allocate_kstack(stack: *mut thread::kstack_t) -> rx_status_t;
    fn vm_free_kstack(stack: *mut thread::kstack_t) -> rx_status_t;
}