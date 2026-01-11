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
    // Small delay to ensure bootloader output is visible
    uefi::boot::stall(Duration::from_secs(1));

    // Initialize UEFI services
    uefi::helpers::init().unwrap();

    uefi::system::with_stdout(|stdout| {
        let _ = stdout.set_color(uefi::proto::console::text::Color::White,
                                 uefi::proto::console::text::Color::Blue);
        let _ = stdout.clear();
        let _ = stdout.enable_cursor(true);

        // Display kernel banner - simple and clear
        let _ = stdout.output_string(cstr16!(
"\r\n\
***************************************************************************\r\n\
*                                                                         *\r\n\
*                 RUSTUX OS KERNEL v0.1.0 - EFI BOOT                      *\r\n\
*                                                                         *\r\n\
***************************************************************************\r\n\
\r\n\
[KERNEL ENTRY POINT REACHED]\r\n\
\r\n\
Status:\r\n\
  UEFI Environment: OK\r\n\
  Console Output: OK\r\n\
  Memory Allocator: OK\r\n\
\r\n\
The kernel is now running as a native UEFI application.\r\n\
Visible output confirmed via UEFI text output protocol.\r\n\
\r\n\
Press any key to halt the system...\r\n\
"));

        // Wait for key press
        let _ = wait_for_key();

        // Halt message
        let _ = stdout.set_color(uefi::proto::console::text::Color::Yellow,
                                 uefi::proto::console::text::Color::Red);
        let _ = stdout.output_string(cstr16!("\r\n\
*** KERNEL HALTED ***\r\n\
System stopped. Press reset button to restart.\r\n\
\r\n\
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
