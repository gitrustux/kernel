#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::proto::loaded_image::LoadedImage;
use uefi::boot::{AllocateType, MemoryType};
use core::time::Duration;

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

/// UEFI entry point
#[entry]
fn main() -> Status {
    // Initialize the system
    uefi::helpers::init().unwrap();

    uefi::system::with_stdout(|stdout| {
        stdout.clear().unwrap();
        stdout.enable_cursor(true).unwrap();

        // Simple ASCII banner - no Unicode box drawing characters
        stdout.output_string(cstr16!(
"==================================================================\r\n\
||                                                               ||\r\n\
||                    ### ### ####                           ||\r\n\
||                    #   # # # #                           ||\r\n\
||                                                               ||\r\n\
||                     Rustica OS v0.1.0                        ||\r\n\
||                  Native UEFI Bootloader                      ||\r\n\
||                                                               ||\r\n\
||                    Phase 2: Kernel Loading                   ||\r\n\
||                                                               ||\r\n\
||==================================================================\r\n\
\r\n\
Architecture: AMD64 (x86_64)\r\n\
Boot Mode: UEFI\r\n\
Loader Version: 0.1.0\r\n\
\r\n\
=== Bootloader Status ===\r\n\
UEFI Bootloader: ACTIVE\r\n\
Searching for kernel.efi...\r\n\
")).unwrap();
    });

    // Try to load and start the kernel
    match load_and_start_kernel() {
        Ok(()) => {
            // Kernel started successfully (shouldn't return)
            unreachable!();
        }
        Err(e) => {
            // Failed to load kernel
            uefi::system::with_stdout(|stdout| {
                stdout.output_string(cstr16!("\r\n=== Kernel Load Failed ===\r\n\
")).unwrap();
                match e.status() {
                    uefi::Status::NOT_FOUND => {
                        stdout.output_string(cstr16!("Error: kernel.efi not found\r\n\
\r\n\
Expected location: /EFI/Rustux/kernel.efi\r\n\
\r\n\
The kernel must be compiled as an EFI executable\r\n\
and placed in the EFI System Partition.\r\n\
")).unwrap();
                    }
                    _ => {
                        stdout.output_string(cstr16!("Error: Failed to load kernel\r\n\
\r\n\
Status code: ")).unwrap();
                        // Cannot easily convert status to string in no_std
                        // Just show generic error message
                        stdout.output_string(cstr16!("UEFI Error\r\n\
")).unwrap();
                    }
                }
            });
        }
    }

    // Wait for key press then halt
    uefi::system::with_stdout(|stdout| {
        stdout.output_string(cstr16!("\r\n\
Press any key to halt system...\r\n\
")).unwrap();
    });

    wait_for_key();

    uefi::system::with_stdout(|stdout| {
        stdout.output_string(cstr16!("\r\n=== System Halted ===\r\n\
Press reset button to restart\r\n\
")).unwrap();
    });

    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
}

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
        let _ = stdout.output_string(cstr16!("Opened EFI volume\r\n"));
    });

    // Try to open kernel.efi
    let kernel_path = cstr16!("\\EFI\\Rustux\\kernel.efi");
    let kernel_file = root.open(kernel_path, FileMode::Read, FileAttribute::empty());

    match kernel_file {
        Ok(handle) => {
            uefi::system::with_stdout(|stdout| {
                let _ = stdout.output_string(cstr16!("Found kernel.efi, loading...\r\n"));
            });

            // Get the file as a regular file
            match handle.into_type().map_err(|e| e.status())? {
                uefi::proto::media::file::FileType::Regular(mut file) => {
                    // Read the entire file into memory
                    let mut info_buf = [0u8; 256];
                    let info = file.get_info::<uefi::proto::media::file::FileInfo>(&mut info_buf)
                        .map_err(|e| uefi::Error::from(e.status()))?;

                    let file_size = info.file_size() as usize;
                    uefi::system::with_stdout(|stdout| {
                        let _ = stdout.output_string(cstr16!("Kernel file size: "));
                        // Simple size display without format! macro
                        // (We can't easily format dynamic integers to CStr16)
                        let _ = stdout.output_string(cstr16!("OK\r\n"));
                    });

                    // Allocate memory for the kernel
                    let num_pages = (file_size + 0xFFF) / 0x1000; // Round up to page size
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
                        let _ = stdout.output_string(cstr16!("Kernel loaded into memory\r\n\
Loading as EFI image...\r\n\
"));
                    });

                    // Load and start the kernel as an EFI image using raw boot services
                    let result = unsafe {
                        // Get the system table and boot services
                        let st = uefi::table::system_table_raw().expect("System table not initialized");
                        let system_table = st.as_ref();
                        let boot_services = system_table.boot_services;

                        // Call LoadImage
                        // EFI_STATUS LoadImage(
                        //   IN BOOLEAN  BootPolicy,
                        //   IN EFI_HANDLE  ParentImageHandle,
                        //   IN EFI_DEVICE_PATH_PROTOCOL  *DevicePath,
                        //   IN VOID  *SourceBuffer,
                        //   IN UINTN  SourceSize,
                        //   OUT EFI_HANDLE  *ImageHandle
                        // );
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
                                let _ = stdout.output_string(cstr16!("EFI image loaded\r\n\
Starting kernel...\r\n\
"));
                            });

                            // Call StartImage
                            // EFI_STATUS StartImage(
                            //   IN EFI_HANDLE  ImageHandle,
                            //   OUT UINTN  *ExitDataSize OPTIONAL,
                            //   OUT CHAR16  **ExitData OPTIONAL
                            // );
                            let start_image = (*boot_services).start_image;
                            let status = start_image(
                                kernel_handle,
                                core::ptr::null_mut(),
                                core::ptr::null_mut(),
                            );

                            // If we get here, the kernel returned
                            if status.is_success() {
                                uefi::system::with_stdout(|stdout| {
                                    let _ = stdout.output_string(cstr16!("Kernel returned unexpectedly\r\n"));
                                });
                                Err(uefi::Status::ABORTED)
                            } else {
                                uefi::system::with_stdout(|stdout| {
                                    let _ = stdout.output_string(cstr16!("Kernel exited with error\r\n"));
                                });
                                Err(status)
                            }
                        } else {
                            uefi::system::with_stdout(|stdout| {
                                let _ = stdout.output_string(cstr16!("LoadImage failed\r\n"));
                            });
                            Err(status)
                        }
                    };

                    result.map_err(|e| uefi::Error::from(e))
                }
                _ => {
                    uefi::system::with_stdout(|stdout| {
                        let _ = stdout.output_string(cstr16!("Error: Not a regular file\r\n"));
                    });
                    Err(uefi::Status::NOT_FOUND.into())
                }
            }
        }
        Err(e) => {
            uefi::system::with_stdout(|stdout| {
                let _ = stdout.output_string(cstr16!("kernel.efi not found\r\n"));
            });
            Err(e)
        }
    }
}

/// Wait for key press with timeout
fn wait_for_key() {
    uefi::system::with_stdin(|stdin| {
        let mut attempts = 0;
        loop {
            match stdin.read_key() {
                Ok(Some(_key)) => {
                    break;
                }
                Ok(None) => {
                    uefi::boot::stall(Duration::from_millis(100));
                    attempts += 1;
                    if attempts > 100 {
                        break;
                    }
                }
                Err(_) => {
                    uefi::boot::stall(Duration::from_millis(100));
                    attempts += 1;
                    if attempts > 100 {
                        break;
                    }
                }
            }
        }
    });
}
