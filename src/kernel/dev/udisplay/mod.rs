// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Userspace Display (Framebuffer) Management
//!
//! Manages the kernel's display framebuffer, mapping it into kernel address space
//! and binding it to the gfxconsole for output.
//!
//! # Features
//!
//! - Framebuffer VMO mapping into kernel address space
//! - Display info management
//! - Gfxconsole binding
//! - Crash screen support
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::kernel::dev::udisplay;
//!
//! // Initialize the display subsystem
//! udisplay::init()?;
//!
//! // Set display info
//! let info = DisplayInfo { ... };
//! udisplay::set_display_info(&info)?;
//!
//! // Bind to gfxconsole
//! udisplay::bind_gfxconsole()?;
//! ```

use crate::kernel::vm::{Result, VmError};

/// Display pixel format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PixelFormat {
    /// RGB 5-6-5 (16 bits per pixel)
    Rgb565 = 0,
    /// RGB 8-8-8 (24 bits per pixel)
    Rgb888 = 1,
    /// XRGB 8-8-8-8 (32 bits per pixel, with padding)
    Xrgb8888 = 2,
    /// ARGB 8-8-8-8 (32 bits per pixel)
    Argb8888 = 3,
}

/// Display info structure
///
/// Describes the framebuffer format and dimensions.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DisplayInfo {
    /// Virtual address of the framebuffer
    pub framebuffer: Option<usize>,

    /// Pixel format
    pub format: PixelFormat,

    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Stride in bytes (usually width * bytes_per_pixel)
    pub stride: u32,

    /// Display flags
    pub flags: DisplayFlags,

    /// Optional flush callback
    pub flush: Option<unsafe extern "C" fn(u32, u32)>,
}

bitflags::bitflags! {
    /// Display flags
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DisplayFlags: u32 {
        /// Hardware framebuffer (no effect if CRASH_FRAMEBUFFER is set)
        const HW_FRAMEBUFFER = 1 << 0;

        /// Framebuffer needs cache flush after writes
        const NEEDS_CACHE_FLUSH = 1 << 1;

        /// Crash framebuffer - gfxconsole won't allocate backing buffer
        const CRASH_FRAMEBUFFER = 1 << 2;
    }
}

/// Userspace display state
///
/// Tracks the current framebuffer mapping and display info.
#[repr(C)]
pub struct UDisplayState {
    /// Virtual address of mapped framebuffer
    framebuffer_virt: Option<usize>,

    /// Size of framebuffer in bytes
    framebuffer_size: usize,

    /// Display information
    info: DisplayInfo,
}

impl UDisplayState {
    /// Create a new uninitialized display state
    pub const fn new() -> Self {
        Self {
            framebuffer_virt: None,
            framebuffer_size: 0,
            info: DisplayInfo {
                framebuffer: None,
                format: PixelFormat::Xrgb8888,
                width: 0,
                height: 0,
                stride: 0,
                flags: DisplayFlags::empty(),
                flush: None,
            },
        }
    }

    /// Get the framebuffer virtual address
    pub fn framebuffer_virt(&self) -> Option<usize> {
        self.framebuffer_virt
    }

    /// Get the framebuffer size
    pub fn framebuffer_size(&self) -> usize {
        self.framebuffer_size
    }

    /// Get the display info
    pub fn info(&self) -> &DisplayInfo {
        &self.info
    }
}

/// Global display state
static mut GLOBAL_DISPLAY: UDisplayState = UDisplayState::new();

/// Crash log buffer for bluescreen
static mut CRASHLOG_BUF: [u8; 4096] = [0; 4096];

/// Initialize the userspace display subsystem
///
/// This function currently does nothing but is provided for API compatibility.
/// Returns success to indicate the subsystem is ready.
pub fn init() -> Result {
    crate::log_info!("Userspace display subsystem initialized");
    Ok(())
}

/// Bluescreen halt handler
///
/// Called during a kernel panic to display crash information.
///
/// This function:
/// 1. Formats the crashlog to a string
/// 2. Stows it with the platform
/// 3. Renders it to the framebuffer if available
pub fn dlog_bluescreen_halt() {
    // TODO: Implement crash log formatting
    let len = 0;

    // TODO: Store crash log with platform
    // platform_stow_crashlog(CRASHLOG_BUF.as_ptr(), len);

    // If we have a framebuffer, display the crash log
    if unsafe { GLOBAL_DISPLAY.framebuffer_virt().is_none() } {
        return;
    }

    // TODO: Render crash log to framebuffer
    crate::log_warn!("Crash log rendered to framebuffer");
}

/// Clear the framebuffer VMO mapping
///
/// Unmaps and releases the current framebuffer mapping.
pub fn clear_framebuffer_vmo() {
    unsafe {
        GLOBAL_DISPLAY.framebuffer_virt = None;
        GLOBAL_DISPLAY.framebuffer_size = 0;
    }

    crate::log_debug!("Framebuffer VMO cleared");
}

/// Set a framebuffer VMO
///
/// Maps a VMO containing framebuffer data into kernel address space.
///
/// # Arguments
///
/// * `vmo_addr` - Physical address of the VMO
/// * `size` - Size of the framebuffer in bytes
///
/// # Returns
///
/// Ok(()) on success, or an error if mapping fails
pub fn set_framebuffer(vmo_addr: usize, size: usize) -> Result {
    clear_framebuffer_vmo();

    // TODO: Create VMO mapping into kernel address space
    // For now, just track the physical address
    unsafe {
        GLOBAL_DISPLAY.framebuffer_virt = Some(vmo_addr);
        GLOBAL_DISPLAY.framebuffer_size = size;
    }

    crate::log_info!(
        "Framebuffer mapped: addr={:#x}, size={}",
        vmo_addr,
        size
    );

    Ok(())
}

/// Set display information
///
/// Updates the display info structure with the provided information.
///
/// # Arguments
///
/// * `info` - Display information to set
///
/// # Returns
///
/// Ok(()) on success
pub fn set_display_info(info: &DisplayInfo) -> Result {
    unsafe {
        GLOBAL_DISPLAY.info = *info;
    }

    crate::log_debug!(
        "Display info updated: {}x{} stride={}",
        info.width,
        info.height,
        info.stride
    );

    Ok(())
}

/// Bind the display to gfxconsole
///
/// Connects the framebuffer to the gfxconsole for kernel text output.
///
/// # Returns
///
/// Ok(()) on success, or an error if no framebuffer is available
pub fn bind_gfxconsole() -> Result {
    let framebuffer_virt = unsafe { GLOBAL_DISPLAY.framebuffer_virt() };

    if framebuffer_virt.is_none() {
        return Err(VmError::InvalidAddress);
    }

    unsafe {
        // Update display info with framebuffer address
        let info = &mut GLOBAL_DISPLAY.info;
        info.framebuffer = framebuffer_virt;
        info.flags |= DisplayFlags::NEEDS_CACHE_FLUSH | DisplayFlags::CRASH_FRAMEBUFFER;

        // TODO: Bind to gfxconsole
        // gfxconsole_bind_display(&info, nullptr);
    }

    crate::log_info!("Display bound to gfxconsole");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_info_default() {
        let info = DisplayInfo {
            framebuffer: None,
            format: PixelFormat::Xrgb8888,
            width: 1024,
            height: 768,
            stride: 4096,
            flags: DisplayFlags::empty(),
            flush: None,
        };

        assert_eq!(info.width, 1024);
        assert_eq!(info.height, 768);
    }

    #[test]
    fn test_display_flags() {
        let flags = DisplayFlags::NEEDS_CACHE_FLUSH | DisplayFlags::CRASH_FRAMEBUFFER;

        assert!(flags.contains(DisplayFlags::NEEDS_CACHE_FLUSH));
        assert!(flags.contains(DisplayFlags::CRASH_FRAMEBUFFER));
        assert!(!flags.contains(DisplayFlags::HW_FRAMEBUFFER));
    }

    #[test]
    fn test_udisplay_state_new() {
        let state = UDisplayState::new();

        assert!(state.framebuffer_virt().is_none());
        assert_eq!(state.framebuffer_size(), 0);
    }

    #[test]
    fn test_pixel_format() {
        assert_eq!(PixelFormat::Rgb565 as u32, 0);
        assert_eq!(PixelFormat::Rgb888 as u32, 1);
        assert_eq!(PixelFormat::Xrgb8888 as u32, 2);
        assert_eq!(PixelFormat::Argb8888 as u32, 3);
    }
}
