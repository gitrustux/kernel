// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! I/O Port bitmap management
//!
//! This module provides functionality for managing I/O port access permissions
//! through a bitmap that can be loaded into the TSS.

use crate::kernel::spinlock::SpinLock;
use crate::bitmap::rle_bitmap::RleBitmap;
use core::ptr::NonNull;
use alloc::sync::Arc;

/// Structure to manage x86 I/O port access permissions
pub struct IoBitmap {
    /// The bitmap storing I/O port permissions
    bitmap: Option<RleBitmap>,
    /// Lock to protect concurrent access to the bitmap
    lock: SpinLock,
}

impl IoBitmap {
    /// Create a new I/O bitmap with default permissions
    pub fn new() -> Self {
        Self {
            bitmap: Some(RleBitmap::new()),
            lock: SpinLock::new(),
        }
    }

    /// Get the IoBitmap associated with the current thread
    ///
    /// # Returns
    ///
    /// Reference to the current thread's I/O bitmap
    pub fn get_current() -> &'static Self {
        // This would be implemented elsewhere in the system
        // and properly return a reference to the current thread's IoBitmap
        unsafe { sys_get_current_io_bitmap() }
    }

    /// Set I/O bitmap permissions for a range of ports
    ///
    /// # Arguments
    ///
    /// * `port` - Starting port number
    /// * `len` - Number of consecutive ports to modify
    /// * `enable` - Whether to enable (true) or disable (false) access
    ///
    /// # Returns
    ///
    /// 0 on success, or an error code on failure
    pub fn set_io_bitmap(&mut self, port: u32, len: u32, enable: bool) -> i32 {
        let guard = self.lock.lock();
        
        if let Some(bitmap) = &mut self.bitmap {
            let result = if enable {
                bitmap.clear_range(port as usize, (port + len) as usize)
            } else {
                bitmap.set_range(port as usize, (port + len) as usize)
            };
            
            // Schedule an update on all CPUs
            if result.is_ok() {
                unsafe { sys_schedule_update_task(self) };
            }
            
            result.map(|_| 0).unwrap_or(-1)
        } else {
            -1
        }
    }
}

impl Drop for IoBitmap {
    fn drop(&mut self) {
        // Ensure any resources are properly cleaned up
        // This would typically involve removing this bitmap from the TSS
        unsafe {
            if self.bitmap.is_some() {
                sys_cleanup_io_bitmap(self);
            }
        }
    }
}

// External C functions that this module interfaces with
extern "C" {
    /// Get the current thread's I/O bitmap
    fn sys_get_current_io_bitmap() -> &'static IoBitmap;
    
    /// Schedule the bitmap update task on all CPUs
    fn sys_schedule_update_task(bitmap: *const IoBitmap);
    
    /// Clean up resources when an I/O bitmap is dropped
    fn sys_cleanup_io_bitmap(bitmap: *mut IoBitmap);
    
    /// Set the TSS I/O bitmap
    pub fn sys_x86_set_tss_io_bitmap(bitmap: *mut IoBitmap);
    
    /// Clear the TSS I/O bitmap
    pub fn sys_x86_clear_tss_io_bitmap(bitmap: *mut IoBitmap);
}

/// Set the TSS I/O bitmap
///
/// # Safety
///
/// This function modifies the CPU state and should only be called
/// from privileged code.
///
/// # Arguments
///
/// * `bitmap` - The bitmap to set in the TSS
pub unsafe fn x86_set_tss_io_bitmap(bitmap: &mut IoBitmap) {
    sys_x86_set_tss_io_bitmap(bitmap);
}

/// Clear the TSS I/O bitmap
///
/// # Safety
///
/// This function modifies the CPU state and should only be called
/// from privileged code.
///
/// # Arguments
///
/// * `bitmap` - The bitmap to clear from the TSS
pub unsafe fn x86_clear_tss_io_bitmap(bitmap: &mut IoBitmap) {
    sys_x86_clear_tss_io_bitmap(bitmap);
}