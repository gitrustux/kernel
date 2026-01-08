// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Graphics Library
//!
//! This module provides 2D graphics drawing functions for the kernel.
//! It supports multiple pixel formats and drawing primitives.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use crate::rustux::types::*;

/// Graphics format types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GfxFormat {
    /// RGB 565 (16 bits per pixel)
    RGB565 = 0,
    /// ARGB 8888 (32 bits per pixel)
    ARGB8888 = 1,
    /// RGB x888 (32 bits per pixel, no alpha)
    RGBx888 = 2,
    /// Monochrome 8-bit
    Mono8 = 3,
    /// RGB 332 (8 bits per pixel)
    RGB332 = 4,
    /// RGB 2220 (8 bits per pixel)
    RGB2220 = 5,
}

/// Graphics surface flags
pub const GFX_FLAG_FLUSH_CPU_CACHE: u32 = 0x01;
pub const GFX_FLAG_FREE_ON_DESTROY: u32 = 0x02;

/// Maximum alpha value
pub const MAX_ALPHA: u8 = 255;

/// Graphics font
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GfxFont {
    /// Font data (array of 16-bit values per character row)
    pub data: *const u16,
    /// Character width in pixels
    pub width: u32,
    /// Character height in pixels
    pub height: u32,
}

/// Graphics surface
pub struct GfxSurface {
    /// Surface flags
    pub flags: u32,
    /// Pixel format
    pub format: GfxFormat,
    /// Surface width in pixels
    pub width: u32,
    /// Surface height in pixels
    pub height: u32,
    /// Stride in pixels
    pub stride: u32,
    /// Alpha value
    pub alpha: u8,
    /// Pixel size in bytes
    pub pixelsize: u32,
    /// Total buffer size in bytes
    pub len: usize,
    /// Pointer to pixel data
    pub ptr: *mut u8,
    /// Flush function (optional)
    pub flush: Option<extern "C" fn(u32, u32)>,
    /// Color translation function (optional)
    pub translate_color: Option<extern "C" fn(u32) -> u32>,
}

unsafe impl Send for GfxSurface {}
unsafe impl Sync for GfxSurface {}

impl GfxSurface {
    /// Create a new graphics surface
    ///
    /// # Arguments
    ///
    /// * `ptr` - Pointer to pixel data (null to allocate)
    /// * `width` - Width in pixels
    /// * `height` - Height in pixels
    /// * `stride` - Stride in pixels
    /// * `format` - Pixel format
    /// * `flags` - Surface flags
    pub fn create(
        ptr: *mut u8,
        width: u32,
        height: u32,
        stride: u32,
        format: GfxFormat,
        flags: u32,
    ) -> Result<Self, i32> {
        if width == 0 || height == 0 || stride < width {
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }

        let pixelsize = format.pixel_size();
        let len = (height * stride * pixelsize) as usize;

        let ptr = if ptr.is_null() {
            // Allocate buffer
            let mut vec = Vec::with_capacity(len);
            vec.resize(len, 0);
            let ptr = vec.as_mut_ptr();
            core::mem::forget(vec);
            ptr
        } else {
            ptr
        };

        let mut surface = Self {
            flags,
            format,
            width,
            height,
            stride,
            alpha: MAX_ALPHA,
            pixelsize,
            len,
            ptr,
            flush: None,
            translate_color: None,
        };

        surface.setup_format();
        Ok(surface)
    }

    /// Setup format-specific function pointers
    fn setup_format(&mut self) {
        match self.format {
            GfxFormat::RGB565 => {
                self.pixelsize = 2;
                self.translate_color = Some(argb8888_to_rgb565);
            }
            GfxFormat::ARGB8888 | GfxFormat::RGBx888 => {
                self.pixelsize = 4;
                self.translate_color = None;
            }
            GfxFormat::Mono8 => {
                self.pixelsize = 1;
                self.translate_color = Some(argb8888_to_luma);
            }
            GfxFormat::RGB332 => {
                self.pixelsize = 1;
                self.translate_color = Some(argb8888_to_rgb332);
            }
            GfxFormat::RGB2220 => {
                self.pixelsize = 1;
                self.translate_color = Some(argb8888_to_rgb2220);
            }
        }

        self.len = (self.height * self.stride * self.pixelsize) as usize;
    }

    /// Copy a rectangle of pixels
    pub fn copyrect(&mut self, x: u32, y: u32, width: u32, height: u32, x2: u32, y2: u32) {
        // Trim and clip
        if x >= self.width || x2 >= self.width || y >= self.height || y2 >= self.height {
            return;
        }
        if width == 0 || height == 0 {
            return;
        }

        let mut width = width;
        let mut height = height;

        // Clip width
        if x + width > self.width {
            width = self.width - x;
        }
        if x2 + width > self.width {
            width = self.width - x2;
        }

        // Clip height
        if y + height > self.height {
            height = self.height - y;
        }
        if y2 + height > self.height {
            height = self.height - y2;
        }

        unsafe {
            match self.format {
                GfxFormat::RGB565 => self.copyrect_impl::<u16>(x, y, width, height, x2, y2),
                GfxFormat::ARGB8888 | GfxFormat::RGBx888 => {
                    self.copyrect_impl::<u32>(x, y, width, height, x2, y2)
                }
                _ => self.copyrect_impl::<u8>(x, y, width, height, x2, y2),
            }
        }
    }

    unsafe fn copyrect_impl<T>(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        x2: u32,
        y2: u32,
    ) {
        let src = self.ptr as *const T;
        let dest = self.ptr as *mut T;
        let stride_diff = self.stride - width;

        let src = src.add((x + y * self.stride) as usize);
        let mut dest = dest.add((x2 + y2 * self.stride) as usize);

        if (dest as usize) < (src as usize) {
            // Copy forward
            for _ in 0..height {
                for _ in 0..width {
                    *dest = *src;
                    dest = dest.add(1);
                    src = src.add(1);
                }
                dest = dest.add(stride_diff as usize);
                // src already advanced
            }
        } else {
            // Copy backward
            let mut src = src.add((height * self.stride + width) as usize);
            let mut dest = dest.add((height * self.stride + width) as usize);

            for _ in 0..height {
                for _ in 0..width {
                    dest = dest.sub(1);
                    src = src.sub(1);
                    *dest = *src;
                }
                dest = dest.sub(stride_diff as usize);
                src = src.sub(stride_diff as usize);
            }
        }
    }

    /// Fill a rectangle with a solid color
    pub fn fillrect(&mut self, x: u32, y: u32, width: u32, height: u32, mut color: u32) {
        // Trim
        if x >= self.width || y >= self.height {
            return;
        }
        if width == 0 || height == 0 {
            return;
        }

        let mut width = width;
        let mut height = height;

        // Clip
        if x + width > self.width {
            width = self.width - x;
        }
        if y + height > self.height {
            height = self.height - y;
        }

        // Translate color if needed
        if let Some(func) = self.translate_color {
            color = func(color);
        }

        unsafe {
            match self.format {
                GfxFormat::RGB565 => self.fillrect_impl::<u16>(x, y, width, height, color),
                GfxFormat::ARGB8888 | GfxFormat::RGBx888 => {
                    self.fillrect_impl::<u32>(x, y, width, height, color)
                }
                _ => self.fillrect_impl::<u8>(x, y, width, height, color),
            }
        }
    }

    unsafe fn fillrect_impl<T>(&mut self, x: u32, y: u32, width: u32, height: u32, color: u32) {
        let mut dest = self.ptr as *mut T;
        dest = dest.add((x + y * self.stride) as usize);
        let stride_diff = self.stride - width;

        let color = color as T;

        for _ in 0..height {
            for _ in 0..width {
                *dest = color;
                dest = dest.add(1);
            }
            dest = dest.add(stride_diff as usize);
        }
    }

    /// Put a single pixel
    pub fn putpixel(&mut self, x: u32, y: u32, mut color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        // Translate color if needed
        if let Some(func) = self.translate_color {
            color = func(color);
        }

        unsafe {
            let offset = (x + y * self.stride) as usize;
            match self.format {
                GfxFormat::RGB565 => {
                    let p = self.ptr.add(offset * 2) as *mut u16;
                    *p = color as u16;
                }
                GfxFormat::ARGB8888 | GfxFormat::RGBx888 => {
                    let p = self.ptr.add(offset * 4) as *mut u32;
                    *p = color as u32;
                }
                _ => {
                    let p = self.ptr.add(offset);
                    *p = color as u8;
                }
            }
        }
    }

    /// Flush the surface to display
    pub fn flush(&self) {
        if (self.flags & GFX_FLAG_FLUSH_CPU_CACHE) != 0 {
            // TODO: Implement cache flush
        }

        if let Some(flush_fn) = self.flush {
            flush_fn(0, self.height - 1);
        }
    }

    /// Flush specific rows
    pub fn flush_rows(&self, mut start: u32, mut end: u32) {
        if start > end {
            core::mem::swap(&mut start, &mut end);
        }

        if start >= self.height {
            return;
        }
        if end >= self.height {
            end = self.height - 1;
        }

        if (self.flags & GFX_FLAG_FLUSH_CPU_CACHE) != 0 {
            // TODO: Implement cache flush for specific rows
        }

        if let Some(flush_fn) = self.flush {
            flush_fn(start, end);
        }
    }
}

impl Drop for GfxSurface {
    fn drop(&mut self) {
        if (self.flags & GFX_FLAG_FREE_ON_DESTROY) != 0 && !self.ptr.is_null() {
            // TODO: Implement proper deallocation
            // For now, we leak the memory since we can't safely deallocate in no_std
        }
    }
}

impl GfxFormat {
    /// Get pixel size in bytes for this format
    fn pixel_size(self) -> u32 {
        match self {
            GfxFormat::RGB565 => 2,
            GfxFormat::ARGB8888 | GfxFormat::RGBx888 => 4,
            GfxFormat::Mono8 | GfxFormat::RGB332 | GfxFormat::RGB2220 => 1,
        }
    }
}

/// Convert ARGB 8888 to grayscale
extern "C" fn argb8888_to_luma(argb: u32) -> u32 {
    let blue = (argb & 0xFF) * 74;
    let green = ((argb >> 8) & 0xFF) * 732;
    let red = ((argb >> 16) & 0xFF) * 218;

    let intensity = red + blue + green;
    ((intensity >> 10) & 0xFF) as u32
}

/// Convert ARGB 8888 to RGB 565
extern "C" fn argb8888_to_rgb565(argb: u32) -> u32 {
    let mut out = 0;

    out |= (argb >> 3) & 0x1f; // Blue
    out |= ((argb >> 10) & 0x3f) << 5; // Green
    out |= ((argb >> 19) & 0x1f) << 11; // Red

    out
}

/// Convert ARGB 8888 to RGB 332
extern "C" fn argb8888_to_rgb332(argb: u32) -> u32 {
    let mut out = 0;

    out |= (argb >> 6) & 0x3; // Blue
    out |= ((argb >> 13) & 0x7) << 2; // Green
    out |= ((argb >> 21) & 0x7) << 5; // Red

    out
}

/// Convert ARGB 8888 to RGB 2220
extern "C" fn argb8888_to_rgb2220(argb: u32) -> u32 {
    let mut out = 0;

    out |= ((argb >> 6) & 0x3) << 2; // Blue
    out |= ((argb >> 14) & 0x3) << 4; // Green
    out |= ((argb >> 22) & 0x3) << 6; // Red

    out
}

/// Draw a line
pub fn gfx_line(surface: &mut GfxSurface, x1: u32, y1: u32, x2: u32, y2: u32, color: u32) {
    if x1 >= surface.width || x2 >= surface.width {
        return;
    }
    if y1 >= surface.height || y2 >= surface.height {
        return;
    }

    let dx = x2 as i32 - x1 as i32;
    let dy = y2 as i32 - y1 as i32;

    let sdx = if dx > 0 { 1 } else { -1 };
    let sdy = if dy > 0 { 1 } else { -1 };

    let dxabs = dx.abs() as u32;
    let dyabs = dy.abs() as u32;

    let mut x = dyabs / 2;
    let mut y = dxabs / 2;

    let mut px = x1;
    let mut py = y1;

    if dxabs >= dyabs {
        // Mostly horizontal
        for _ in 0..dxabs {
            x += dyabs;
            if x >= dxabs {
                x -= dxabs;
                py = (py as i32 + sdy) as u32;
            }
            px = (px as i32 + sdx) as u32;
            surface.putpixel(px, py, color);
        }
    } else {
        // Mostly vertical
        for _ in 0..dyabs {
            y += dxabs;
            if y >= dyabs {
                y -= dxabs;
                px = (px as i32 + sdx) as u32;
            }
            py = (py as i32 + sdy) as u32;
            surface.putpixel(px, py, color);
        }
    }
}

/// Put a character on the surface
pub fn gfx_putchar(
    surface: &mut GfxSurface,
    font: &GfxFont,
    ch: u8,
    x: u32,
    y: u32,
    mut fg: u32,
    mut bg: u32,
) {
    if ch > 127 {
        return;
    }
    if x > surface.width - font.width {
        return;
    }
    if y > surface.height - font.height {
        return;
    }

    // Translate colors if needed
    if let Some(func) = surface.translate_color {
        fg = func(fg);
        bg = func(bg);
    }

    unsafe {
        let mut dest = surface.ptr;
        let cdata = font.data.add(ch as usize * font.height as usize);

        for _ in 0..font.height {
            let mut xdata = *cdata;
            for _j in 0..font.width {
                let color = if xdata & 1 != 0 { fg } else { bg };

                match surface.format {
                    GfxFormat::RGB565 => {
                        let p = dest as *mut u16;
                        *p = color as u16;
                        dest = dest.add(2);
                    }
                    GfxFormat::ARGB8888 | GfxFormat::RGBx888 => {
                        let p = dest as *mut u32;
                        *p = color as u32;
                        dest = dest.add(4);
                    }
                    _ => {
                        *dest = color as u8;
                        dest = dest.add(1);
                    }
                }

                xdata >>= 1;
            }
            // Add stride padding
            let padding = (surface.stride - font.width) as usize * surface.pixelsize as usize;
            dest = dest.add(padding);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_conversions() {
        let argb = 0xFF808080; // Gray

        let luma = argb8888_to_luma(argb);
        assert!(luma >= 0 && luma <= 255);

        let rgb565 = argb8888_to_rgb565(argb);
        assert!(rgb565 <= 0xFFFF);

        let rgb332 = argb8888_to_rgb332(argb);
        assert!(rgb332 <= 0xFF);
    }

    #[test]
    fn test_surface_creation() {
        let surface = GfxSurface::create(
            core::ptr::null_mut(),
            640,
            480,
            640,
            GfxFormat::ARGB8888,
            GFX_FLAG_FREE_ON_DESTROY,
        );
        assert!(surface.is_ok());

        let surface = surface.unwrap();
        assert_eq!(surface.width, 640);
        assert_eq!(surface.height, 480);
        assert_eq!(surface.pixelsize, 4);
    }
}
