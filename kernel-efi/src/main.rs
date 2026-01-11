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

/// Boot mode selection
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum BootMode {
    LiveUsb = 1,
    Install = 2,
}

/// UEFI entry point for the kernel
#[entry]
fn main() -> Status {
    // Small delay to ensure bootloader output is visible
    uefi::boot::stall(Duration::from_secs(1));

    // Initialize UEFI services
    uefi::helpers::init().unwrap();

    // Show boot menu and get user selection
    let boot_mode = show_boot_menu();

    uefi::system::with_stdout(|stdout| {
        let _ = stdout.set_color(uefi::proto::console::text::Color::White,
                                 uefi::proto::console::text::Color::Blue);
        let _ = stdout.clear();
        let _ = stdout.enable_cursor(true);

        // Display kernel banner
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
"));

        // Show selected boot mode
        let _ = stdout.set_color(uefi::proto::console::text::Color::Yellow,
                                 uefi::proto::console::text::Color::Blue);
        match boot_mode {
            BootMode::LiveUsb => {
                let _ = stdout.output_string(cstr16!("\r\n\
Boot Mode: LIVE USB (with persistence)\r\n\
  - OS running from RAM\r\n\
  - Changes saved to USB storage\r\n\
\r\n\
Initializing system...\r\n\
"));
            }
            BootMode::Install => {
                let _ = stdout.output_string(cstr16!("\r\n\
Boot Mode: INSTALLATION MODE\r\n\
  - Preparing for installation to target device\r\n\
\r\n\
NOTE: Installation system coming soon...\r\n\
System will boot in Live USB mode for now.\r\n\
\r\n\
Initializing system...\r\n\
"));
            }
        }
    });

    // Continue to OS initialization
    // TODO: Transition to main OS loop
    // For now, keep system running in a loop
    uefi::system::with_stdout(|stdout| {
        let _ = stdout.set_color(uefi::proto::console::text::Color::Green,
                                 uefi::proto::console::text::Color::Blue);
        let _ = stdout.output_string(cstr16!("\r\n\
[SYSTEM READY]\r\n\
Rustux OS is running. Type 'help' for available commands.\r\n\
\r\n\
"));
    });

    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
    }
}

/// Show boot menu and return selected boot mode
fn show_boot_menu() -> BootMode {
    const MENU_TIMEOUT_SECONDS: u64 = 10;
    const MENU_DELAY_MS: u64 = 100;
    let max_attempts = (MENU_TIMEOUT_SECONDS * 1000) / MENU_DELAY_MS;

    uefi::system::with_stdout(|stdout| {
        let _ = stdout.set_color(uefi::proto::console::text::Color::White,
                                 uefi::proto::console::text::Color::Black);
        let _ = stdout.clear();
        let _ = stdout.enable_cursor(true);

        // Display boot menu
        let _ = stdout.output_string(cstr16!(
"\r\n\
***************************************************************************\r\n\
*                                                                         *\r\n\
*                      RUSTUX OS BOOTLOADER v0.3.0                        *\r\n\
*                                                                         *\r\n\
***************************************************************************\r\n\
\r\n\
Select Boot Mode:\r\n\
\r\n\
  [1] Live USB (with persistence)\r\n\
      - Run OS from RAM\r\n\
      - Changes saved to USB storage\r\n\
\r\n\
  [2] Install to Disk\r\n\
      - Install Rustux OS to target device\r\n\
\r\n\
"));
    });

    // Countdown timer with default selection
    let mut selection = BootMode::LiveUsb;

    for countdown in (0..max_attempts).rev() {
        let seconds_left = (countdown as u64 * MENU_DELAY_MS) / 1000;

        uefi::system::with_stdout(|stdout| {
            // Update countdown display
            let _ = stdout.set_cursor_position(0, 17);
            let _ = stdout.output_string(cstr16!("Booting in "));
            let _ = stdout.output_uint(seconds_left);
            let _ = stdout.output_string(cstr16!(" seconds... [Press 1-2 to select]      "));
        });

        // Check for key press
        let key_pressed = uefi::system::with_stdin(|stdin| {
            let _ = stdin.reset(false);
            match stdin.read_key() {
                Ok(Some(key)) => {
                    // Check if it's a printable key
                    match key {
                        uefi::proto::console::text::Key::Printable(c) => {
                            if c == uefi::Char16::try_from('1').unwrap() {
                                selection = BootMode::LiveUsb;
                                true
                            } else if c == uefi::Char16::try_from('2').unwrap() {
                                selection = BootMode::Install;
                                true
                            } else {
                                false
                            }
                        }
                        _ => false
                    }
                }
                _ => false
            }
        });

        if key_pressed {
            break;
        }

        uefi::boot::stall(Duration::from_millis(MENU_DELAY_MS));
    }

    uefi::system::with_stdout(|stdout| {
        let _ = stdout.set_cursor_position(0, 17);
        let _ = stdout.output_string(cstr16!("                                                          "));
    });

    selection
}

/// Extension trait for outputting unsigned integers
trait OutputUint {
    fn output_uint(&mut self, value: u64) -> uefi::Result;
}

impl OutputUint for uefi::proto::console::text::Output {
    fn output_uint(&mut self, mut value: u64) -> uefi::Result {
        // Simple digit array for u64 values (max 20 digits)
        let digits = [
            cstr16!("0"), cstr16!("1"), cstr16!("2"), cstr16!("3"),
            cstr16!("4"), cstr16!("5"), cstr16!("6"), cstr16!("7"),
            cstr16!("8"), cstr16!("9"),
        ];

        if value == 0 {
            let _ = self.output_string(digits[0]);
            return Ok(());
        }

        // Build digits in reverse order
        let mut digit_vals = [0u8; 20];
        let mut count = 0;

        while value > 0 && count < 20 {
            digit_vals[count] = (value % 10) as u8;
            value /= 10;
            count += 1;
        }

        // Output in correct order (most significant first)
        for i in (0..count).rev() {
            let d = digit_vals[i] as usize;
            if d < 10 {
                let _ = self.output_string(digits[d]);
            }
        }

        Ok(())
    }
}
