#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
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
||                    Phase 1: EFI Application                  ||\r\n\
||                                                               ||\r\n\
||==================================================================\r\n\
\r\n\
Architecture: AMD64 (x86_64)\r\n\
Boot Mode: UEFI\r\n\
Loader Version: 0.1.0\r\n\
\r\n\
=== Bootloader Status ===\r\n\
UEFI Bootloader: ACTIVE\r\n\
Kernel: Not found (expected - Phase 2)\r\n\
\r\n\
Next Steps:\r\n\
  1. Implement kernel loading (Phase 2)\r\n\
  2. Create kernel EFI format (Phase 3)\r\n\
  3. Add kernel execution jump\r\n\
\r\n\
See: /var/www/rustux.com/prod/TODO.md\r\n\
\r\n\
Press any key to continue...\r\n\
")).unwrap();
    });

    // Wait for key press using boot services
    uefi::system::with_stdin(|stdin| {
        let mut attempts = 0;
        loop {
            match stdin.read_key() {
                Ok(Some(_key)) => {
                    break;
                }
                Ok(None) => {
                    // No key yet, wait a bit
                    uefi::boot::stall(Duration::from_millis(100));
                    attempts += 1;
                    if attempts > 100 {
                        // Timeout after 10 seconds, continue anyway
                        break;
                    }
                }
                Err(_) => {
                    // Error reading, wait a bit and retry
                    uefi::boot::stall(Duration::from_millis(100));
                    attempts += 1;
                    if attempts > 100 {
                        break;
                    }
                }
            }
        }
    });

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
