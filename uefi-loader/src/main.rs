#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
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

/// UEFI entry point
#[entry]
fn main() -> Status {
    // Initialize the system
    uefi::helpers::init().unwrap();

    uefi::system::with_stdout(|stdout| {
        stdout.clear().unwrap();

        stdout.output_string(cstr16!("
╔═══════════════════════════════════════════════════════════════╗
║                                                               ║
║                    █ █ █▀▄ ▀▄▀ █▀▀                          ║
║                    █▄▀█ █▄▀ █▄█ ██▄                          ║
║                                                               ║
║                     Rustica OS v0.1.0                        ║
║                  Native UEFI Bootloader                      ║
║                                                               ║
║                    Phase 1: EFI Application                  ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝
")).unwrap();

        stdout.output_string(cstr16!("\r\n")).unwrap();
        stdout.output_string(cstr16!("Architecture: AMD64 (x86_64)\r\n")).unwrap();
        stdout.output_string(cstr16!("Boot Mode: UEFI\r\n")).unwrap();
        stdout.output_string(cstr16!("Loader Version: 0.1.0\r\n")).unwrap();
        stdout.output_string(cstr16!("\r\n")).unwrap();

        stdout.output_string(cstr16!("=== Bootloader Status ===\r\n")).unwrap();
        stdout.output_string(cstr16!("UEFI Bootloader: ACTIVE\r\n")).unwrap();
        stdout.output_string(cstr16!("Kernel: Not found (expected - Phase 2)\r\n")).unwrap();
        stdout.output_string(cstr16!("\r\nNext Steps:\r\n")).unwrap();
        stdout.output_string(cstr16!("  1. Implement kernel loading (Phase 2)\r\n")).unwrap();
        stdout.output_string(cstr16!("  2. Create kernel EFI format (Phase 3)\r\n")).unwrap();
        stdout.output_string(cstr16!("  3. Add kernel execution jump\r\n")).unwrap();
        stdout.output_string(cstr16!("\r\nSee: /var/www/rustux.com/prod/TODO.md\r\n")).unwrap();
        stdout.output_string(cstr16!("\r\n\r\nPress any key to halt...\r\n")).unwrap();
    });

    uefi::system::with_stdin(|stdin| {
        loop {
            if let Some(_) = stdin.read_key().unwrap() {
                break;
            }
        }
    });

    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }

    Status::SUCCESS
}
