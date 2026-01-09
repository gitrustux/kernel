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

/// UEFI entry point for the kernel
#[entry]
fn main() -> Status {
    // Initialize UEFI services
    uefi::helpers::init().unwrap();

    uefi::system::with_stdout(|stdout| {
        let _ = stdout.clear();
        let _ = stdout.enable_cursor(true);

        // Display kernel banner
        let _ = stdout.output_string(cstr16!(
"==================================================================\r\n\
||                                                               ||\r\n\
||                    ### ### ####                           ||\r\n\
||                    #   # # # #                           ||\r\n\
||                                                               ||\r\n\
||                     Rustux OS Kernel v0.1.0                   ||\r\n\
||                  UEFI Application Mode                       ||\r\n\
||                                                               ||\r\n\
||==================================================================\r\n\
\r\n\
Architecture: AMD64 (x86_64)\r\n\
Boot Mode: UEFI\r\n\
Kernel Type: EFI Executable\r\n\
\r\n\
=== Kernel Status ===\r\n\
UEFI Environment: Initialized\r\n\
Memory Allocator: Active\r\n\
Console Output: Active\r\n\
\r\n\
=== Phase 1: Basic UEFI Setup ===\r\n\
[OK] UEFI system table accessed\r\n\
[OK] Console protocols initialized\r\n\
[OK] Memory allocator configured\r\n\
\r\n\
=== Phase 2: Kernel Initialization ===\r\n\
[TODO] Initialize memory management\r\n\
[TODO] Set up interrupt handlers\r\n\
[TODO] Initialize scheduler\r\n\
[TODO] Start kernel services\r\n\
\r\n\
=== Information ===\r\n\
This is a minimal UEFI kernel stub for Rustux OS.\r\n\
The kernel is running as a native UEFI application.\r\n\
\r\n\
Next steps:\r\n\
1. Port memory management from bare-metal kernel\r\n\
2. Implement UEFI-specific drivers\r\n\
3. Set up virtual memory with UEFI memory map\r\n\
4. Initialize hardware using UEFI protocols\r\n\
\r\n\
For development, see: /var/www/rustux.com/prod/kernel/\r\n\
\r\n\
Press any key to halt...\r\n\
"));

        // Wait for key press
        let _ = wait_for_key();

        // Halt
        let _ = stdout.output_string(cstr16!("\r\n=== Kernel Halted ===\r\n\
System stopped. Press reset to restart.\r\n\
"));
    });

    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
    }
}

/// Wait for a key press
fn wait_for_key() -> uefi::Result {
    uefi::system::with_stdin(|stdin| {
        let _ = stdin.reset(false);

        let mut attempts = 0;
        loop {
            match stdin.read_key() {
                Ok(Some(_key)) => break Ok(()),
                Ok(None) => {
                    uefi::boot::stall(Duration::from_millis(100));
                    attempts += 1;
                    if attempts > 100 {
                        break Ok(());
                    }
                }
                Err(_) => {
                    uefi::boot::stall(Duration::from_millis(100));
                    attempts += 1;
                    if attempts > 100 {
                        break Ok(());
                    }
                }
            }
        }
    })
}
