#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::proto::loaded_image::LoadedImage;
use uefi::table::cfg;
use uefi::table::system_table_raw;
use uefi::boot::{AllocateType, MemoryType};
use uefi::Status;
use alloc::vec::Vec;

// Global allocator for UEFI
#[global_allocator]
static ALLOCATOR: uefi::allocator::Allocator = uefi::allocator::Allocator;

// Required for UEFI no_std
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
    }
}

// ============================================================================
// Memory Types - Adapted from Zircon's efi_memory_type conversion
// ============================================================================

/// Rustux memory type for kernel handoff
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustuxMemoryType {
    Available = 1,
    Reserved = 2,
    Reclaimable = 3,
    Peripheral = 4,
}

impl From<MemoryType> for RustuxMemoryType {
    fn from(efi_type: MemoryType) -> Self {
        match efi_type {
            MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::CONVENTIONAL => RustuxMemoryType::Available,

            MemoryType::MMIO
            | MemoryType::MMIO_PORT_SPACE => RustuxMemoryType::Peripheral,

            MemoryType::ACPI_RECLAIM
            | MemoryType::ACPI_NON_VOLATILE => RustuxMemoryType::Reclaimable,

            _ => RustuxMemoryType::Reserved,
        }
    }
}

/// Memory range descriptor for kernel handoff
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryRange {
    pub base: u64,
    pub length: u64,
    pub mem_type: RustuxMemoryType,
}

/// Kernel handoff structure - passed to kernel on boot
#[repr(C)]
#[derive(Debug)]
pub struct KernelHandoff {
    pub memory_map: Vec<MemoryRange>,
    pub acpi_rsdp: Option<u64>,
    pub smbios_entry: Option<u64>,
    pub system_table: u64,
    pub framebuffer: Option<FramebufferInfo>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub base: u64,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: FramebufferFormat,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum FramebufferFormat {
    RGB,
    BGR,
}

// ============================================================================
// UEFI Bootloader - Incorporating Zircon patterns
// ============================================================================

/// UEFI entry point
#[entry]
fn main() -> Status {
    // Initialize the system
    uefi::helpers::init().unwrap();

    uefi::system::with_stdout(|stdout| {
        stdout.clear().unwrap();
        stdout.enable_cursor(true).unwrap();

        // Simple ASCII banner
        stdout.output_string(cstr16!(
"==================================================================\r\n\
||                     Rustux OS Bootloader                      ||\r\n\
||                   v0.3.0 - Zircon Patterns                   ||\r\n\
||==================================================================\r\n\
\r\n\
[Phase 1] UEFI Environment Initialization\r\n\
  - System table acquired\r\n\
  - Memory allocator initialized\r\n\
  - Console protocols ready\r\n\
\r\n\
[Phase 2] Platform Discovery (Zircon-inspired)\r\n\
"));

        // Zircon pattern: Discover ACPI tables
        match find_acpi_rsdp() {
            Some(_rsdp) => {
                let _ = stdout.output_string(cstr16!("  - ACPI RSDP: Found\r\n"));
            }
            None => {
                let _ = stdout.output_string(cstr16!("  - ACPI: Not present (warning)\r\n"));
            }
        }

        let _ = stdout.output_string(cstr16!("\r\n\
[Phase 3] Memory Map Acquisition\r\n\
"));

        // Get memory map using Zircon-inspired pattern
        match get_efi_memory_map(stdout) {
            Ok(memory_ranges) => {
                let _ = stdout.output_string(cstr16!("  - Memory map acquired\r\n"));
                let _ = stdout.output_string(cstr16!("    - Total ranges: "));
                // Simple count display
                let count = memory_ranges.len();
                if count >= 10 {
                    let _tens = count / 10;
                    let _ = stdout.output_string(cstr16!(">10\r\n"));
                } else {
                    let digits = [cstr16!("0\r\n"), cstr16!("1\r\n"), cstr16!("2\r\n"),
                                  cstr16!("3\r\n"), cstr16!("4\r\n"), cstr16!("5\r\n"),
                                  cstr16!("6\r\n"), cstr16!("7\r\n"), cstr16!("8\r\n"),
                                  cstr16!("9\r\n")];
                    if count > 0 && count <= 9 {
                        let _ = stdout.output_string(digits[count]);
                    }
                }
            }
            Err(_) => {
                let _ = stdout.output_string(cstr16!("  - Warning: Memory map acquisition failed\r\n"));
            }
        }

        let _ = stdout.output_string(cstr16!("\r\n\
[Phase 4] Kernel Loading\r\n\
  - Searching for /EFI/Rustux/kernel.efi\r\n\
"));

        match load_and_start_kernel() {
            Ok(_) => {
                // Should not reach here
                let _ = stdout.output_string(cstr16!("\r\n\
[ERROR] Kernel returned unexpectedly\r\n\
"));
                Status::ABORTED
            }
            Err(_) => {
                let _ = stdout.output_string(cstr16!("\r\n\
[ERROR] Kernel load failed\r\n\
"));
                Status::ABORTED
            }
        }
    });

    // Halt
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
    }
}

// ============================================================================
// Memory Map Handling - Zircon-inspired implementation
// ============================================================================

/// Zircon-style GetMemoryMap implementation
/// Returns the memory map buffer, entry size, and map key for ExitBootServices
struct EfiMemoryMapInfo {
    buffer: *mut u8,
    size: usize,
    entry_size: usize,
    key: usize,
}

/// Get EFI memory map - Zircon pattern
/// First call gets size, second call fills the buffer
fn get_efi_memory_map_raw() -> Result<EfiMemoryMapInfo, uefi::Error> {
    // Zircon pattern: First call to get required buffer size
    let mut map_size = 0usize;
    let mut map_key = 0usize;
    let mut entry_size = 0usize;
    let mut entry_version = 0u32;

    let status = unsafe {
        let bt = uefi::table::system_table_raw()
            .ok_or(uefi::Status::NOT_FOUND)?;
        let st = bt.as_ref();
        let boot_services = st.boot_services;

        // GetMemoryMap first call - get buffer size
        // EFI_STATUS GetMemoryMap(
        //   IN OUT UINTN  *MemoryMapSize,
        //   OUT VOID *MemoryMap,
        //   OUT UINTN *MapKey,
        //   OUT UINTN *DescriptorSize,
        //   OUT UINT32 *DescriptorVersion
        // );
        let get_memory_map = (*boot_services).get_memory_map;
        get_memory_map(
            &mut map_size,
            core::ptr::null_mut(),
            &mut map_key,
            &mut entry_size,
            &mut entry_version,
        )
    };

    // EFI_BUFFER_TOO_SMALL (5) is expected on first call
    if !status.is_success() && status != Status::BUFFER_TOO_SMALL {
        return Err(uefi::Error::from(status));
    }

    // Zircon pattern: Add extra space for dynamic allocations
    // during ExitBootServices
    map_size += entry_size * 8;

    // Allocate buffer for memory map
    let buffer_pages = (map_size + 0xFFF) / 0x1000;
    let buffer = uefi::boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        buffer_pages,
    )?;

    let buffer_ptr = buffer.as_ptr() as *mut u8;

    // Get actual memory map
    let status = unsafe {
        let bt = uefi::table::system_table_raw()
            .ok_or(uefi::Status::NOT_FOUND)?;
        let st = bt.as_ref();
        let boot_services = st.boot_services;

        let get_memory_map = (*boot_services).get_memory_map;
        get_memory_map(
            &mut map_size,
            buffer_ptr as *mut uefi_raw::table::boot::MemoryDescriptor,
            &mut map_key,
            &mut entry_size,
            &mut entry_version,
        )
    };

    if !status.is_success() {
        return Err(uefi::Error::from(status));
    }

    Ok(EfiMemoryMapInfo {
        buffer: buffer_ptr,
        size: map_size,
        entry_size,
        key: map_key,
    })
}

/// Zircon-style memory range coalescing
/// Combine contiguous ranges of the same type
fn coalesce_ranges(ranges: &mut Vec<MemoryRange>) {
    if ranges.len() <= 1 {
        return;
    }

    // Zircon pattern: sort by physical address first
    ranges.sort_by_key(|r| r.base);

    let mut write_idx = 1;
    for read_idx in 1..ranges.len() {
        let prev = &ranges[write_idx - 1];
        let curr = &ranges[read_idx];

        // Check if ranges are contiguous and have same type
        if prev.mem_type == curr.mem_type && prev.base + prev.length == curr.base {
            // Merge into previous range
            ranges[write_idx - 1].length += curr.length;
        } else {
            // Keep this range separate
            if read_idx != write_idx {
                ranges[write_idx] = *curr;
            }
            write_idx += 1;
        }
    }

    ranges.truncate(write_idx);
}

/// Convert EFI memory map to Rustux format - Zircon pattern
fn get_efi_memory_map(_stdout: &mut uefi::proto::console::text::Output) -> Result<Vec<MemoryRange>, uefi::Error> {
    let map_info = get_efi_memory_map_raw()?;

    // Convert EFI memory descriptors to Rustux format
    let num_entries = map_info.size / map_info.entry_size;
    let mut ranges = Vec::new();

    for i in 0..num_entries {
        let desc_ptr = unsafe {
            (map_info.buffer as *const u8).add(i * map_info.entry_size)
                as *const uefi_raw::table::boot::MemoryDescriptor
        };
        let desc = unsafe { &*desc_ptr };

        // Zircon pattern: Ignore zero-length entries
        if desc.page_count > 0 {
            // Convert uefi_raw::MemoryType to uefi::MemoryType for our From impl
            let efi_memory_type: MemoryType = unsafe { core::mem::transmute(desc.ty) };
            let range = MemoryRange {
                base: desc.phys_start,
                length: desc.page_count * 4096, // UEFI page size
                mem_type: RustuxMemoryType::from(efi_memory_type),
            };
            ranges.push(range);
        }
    }

    // Zircon pattern: Coalesce contiguous ranges of same type
    coalesce_ranges(&mut ranges);

    Ok(ranges)
}

// ============================================================================
// ACPI Discovery - Zircon-inspired
// ============================================================================

/// ACPI RSDP (Root System Description Pointer) signature
const ACPI_RSDP_SIGNATURE: u64 = 0x2052545020445352; // "RSD PTR "

/// Find ACPI RSDP from UEFI configuration tables - Zircon pattern
fn find_acpi_rsdp() -> Option<u64> {
    // Zircon pattern: Search configuration tables for ACPI GUIDs
    // Try ACPI 2.0 GUID first, then ACPI 1.0 GUID
    let acpi2_guid = cfg::ConfigTableEntry::ACPI2_GUID;

    if let Some(st) = system_table_raw() {
        let system_table: &uefi_raw::table::system::SystemTable = unsafe { st.as_ref() };

        for i in 0..system_table.number_of_configuration_table_entries {
            let entry_ptr = unsafe {
                system_table.configuration_table.add(i)
            };
            let entry = unsafe { &*entry_ptr };

            if entry.vendor_guid == acpi2_guid && !entry.vendor_table.is_null() {
                // Found ACPI table
                let rsdp_ptr = entry.vendor_table as u64;
                return Some(rsdp_ptr);
            }
        }
    }

    None
}

// ============================================================================
// Kernel Loading
// ============================================================================

/// Load and start the kernel.efi from disk
fn load_and_start_kernel() -> uefi::Result {
    // Get the loaded image protocol to find our device
    let image_handle = uefi::boot::image_handle();

    // Get the device handle
    let loaded_image = uefi::boot::open_protocol_exclusive::<LoadedImage>(image_handle)?;
    let device = loaded_image.device().ok_or(uefi::Status::DEVICE_ERROR)?;

    // Get the SimpleFileSystem protocol
    let mut fs = uefi::boot::open_protocol_exclusive::<SimpleFileSystem>(device)?;

    // Open the volume
    let mut root = fs.open_volume()?;

    uefi::system::with_stdout(|stdout| {
        let _ = stdout.output_string(cstr16!("  - Opened EFI volume\r\n"));
    });

    // Try to open kernel.efi
    let kernel_path = cstr16!("\\EFI\\Rustux\\kernel.efi");
    let kernel_file = root.open(kernel_path, FileMode::Read, FileAttribute::empty());

    match kernel_file {
        Ok(handle) => {
            uefi::system::with_stdout(|stdout| {
                let _ = stdout.output_string(cstr16!("  - Found kernel.efi, loading...\r\n"));
            });

            match handle.into_type().map_err(|e| e.status())? {
                uefi::proto::media::file::FileType::Regular(mut file) => {
                    // Read file info
                    let mut info_buf = [0u8; 256];
                    let info = file.get_info::<uefi::proto::media::file::FileInfo>(&mut info_buf)
                        .map_err(|e| uefi::Error::from(e.status()))?;

                    let file_size = info.file_size() as usize;

                    uefi::system::with_stdout(|stdout| {
                        let _ = stdout.output_string(cstr16!("  - Kernel file size: OK\r\n"));
                    });

                    // Allocate memory for the kernel
                    let num_pages = (file_size + 0xFFF) / 0x1000;
                    let kernel_data = uefi::boot::allocate_pages(
                        AllocateType::AnyPages,
                        MemoryType::LOADER_DATA,
                        num_pages,
                    )?;

                    // Read the file
                    let kernel_slice = unsafe {
                        core::slice::from_raw_parts_mut(kernel_data.as_ptr(), file_size)
                    };
                    file.read(kernel_slice).map_err(|e| uefi::Error::from(e.status()))?;

                    uefi::system::with_stdout(|stdout| {
                        let _ = stdout.output_string(cstr16!("  - Kernel loaded into memory\r\n"));
                    });

                    // Load and start the kernel as an EFI image using raw boot services
                    let result = unsafe {
                        let bt = uefi::table::system_table_raw().unwrap();
                        let system_table = bt.as_ref();
                        let boot_services = system_table.boot_services;

                        let mut kernel_handle: *mut core::ffi::c_void = core::ptr::null_mut();
                        let load_image = (*boot_services).load_image;

                        let status = load_image(
                            false.into(),
                            uefi::boot::image_handle().as_ptr(),
                            core::ptr::null(),
                            kernel_data.as_ptr(),
                            file_size,
                            &mut kernel_handle,
                        );

                        if status.is_success() {
                            uefi::system::with_stdout(|stdout| {
                                let _ = stdout.output_string(cstr16!("  - EFI image loaded\r\n"));
                            });

                            let start_image = (*boot_services).start_image;
                            let status = start_image(
                                kernel_handle,
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                            );

                            if status.is_success() {
                                uefi::system::with_stdout(|stdout| {
                                    let _ = stdout.output_string(cstr16!("  - Kernel started\r\n"));
                                });
                                Err(uefi::Status::ABORTED)
                            } else {
                                uefi::system::with_stdout(|stdout| {
                                    let _ = stdout.output_string(cstr16!("  - StartImage failed\r\n"));
                                });
                                Err(status)
                            }
                        } else {
                            uefi::system::with_stdout(|stdout| {
                                let _ = stdout.output_string(cstr16!("  - LoadImage failed\r\n"));
                            });
                            Err(status)
                        }
                    };

                    result.map_err(|e| uefi::Error::from(e))
                }
                _ => {
                    uefi::system::with_stdout(|stdout| {
                        let _ = stdout.output_string(cstr16!("  - Error: Not a regular file\r\n"));
                    });
                    Err(uefi::Status::NOT_FOUND.into())
                }
            }
        }
        Err(e) => {
            uefi::system::with_stdout(|stdout| {
                let _ = stdout.output_string(cstr16!("  - kernel.efi not found\r\n"));
            });
            Err(e)
        }
    }
}
