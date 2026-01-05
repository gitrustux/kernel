// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Hello World Test
//!
//! Simple test program that demonstrates the libsys and libipc APIs.

#![no_std]
#![no_main]

use core::fmt::Write;

extern crate libsys;
extern crate libipc;

use libsys::*;
use libipc::*;

/// Simple stdout writer
struct StdoutWriter;

impl core::fmt::Write for StdoutWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            unsafe {
                syscall::syscall1(syscall::SyscallNumber::WriteStdio as u64, b as u64);
            }
        }
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut writer = StdoutWriter;

    // Print greeting
    let _ = writeln!(writer, "Rustux Userspace Test");

    // Test channel creation
    let _ = writeln!(writer, "Creating channel...");
    match Channel::create() {
        Ok((tx, rx)) => {
            let _ = writeln!(writer, "Channel created successfully");

            // Send a message
            let msg = b"Hello, Rustux!";
            let _ = writeln!(writer, "Sending message: {}", core::str::from_utf8(msg).unwrap());
            if tx.write(msg, &[]).is_err() {
                let _ = writeln!(writer, "Failed to send message");
                return -1;
            }

            // Receive the message
            let mut buf = [0u8; 64];
            let mut handles = vec![];
            match rx.read(&mut buf, &mut handles) {
                Ok(n) => {
                    let _ = writeln!(writer, "Received: {}", core::str::from_utf8(&buf[..n]).unwrap());
                }
                Err(e) => {
                    let _ = writeln!(writer, "Failed to receive: {:?}", e);
                    return -1;
                }
            }
        }
        Err(e) => {
            let _ = writeln!(writer, "Failed to create channel: {:?}", e);
            return -1;
        }
    }

    // Test VMO creation
    let _ = writeln!(writer, "Creating VMO...");
    match Vmo::create(4096, Some("test_vmo")) {
        Ok(vmo) => {
            let _ = writeln!(writer, "VMO created successfully");

            // Get VMO size
            match vmo.get_size() {
                Ok(size) => {
                    let _ = writeln!(writer, "VMO size: {} bytes", size);
                }
                Err(e) => {
                    let _ = writeln!(writer, "Failed to get VMO size: {:?}", e);
                }
            }

            // Write to VMO
            let data = b"Test data";
            match vmo.write(0, data) {
                Ok(n) => {
                    let _ = writeln!(writer, "Wrote {} bytes to VMO", n);
                }
                Err(e) => {
                    let _ = writeln!(writer, "Failed to write to VMO: {:?}", e);
                }
            }

            // Read from VMO
            let mut buf = [0u8; 64];
            match vmo.read(0, &mut buf) {
                Ok(n) => {
                    let _ = writeln!(writer, "Read from VMO: {}", core::str::from_utf8(&buf[..n]).unwrap());
                }
                Err(e) => {
                    let _ = writeln!(writer, "Failed to read from VMO: {:?}", e);
                }
            }
        }
        Err(e) => {
            let _ = writeln!(writer, "Failed to create VMO: {:?}", e);
            return -1;
        }
    }

    // Test event
    let _ = writeln!(writer, "Creating event...");
    match Event::create(true) {
        Ok(event) => {
            let _ = writeln!(writer, "Event created successfully");

            // Wait on the event (should return immediately since it's signaled)
            match event.wait(0) {
                Ok(()) => {
                    let _ = writeln!(writer, "Event was signaled");
                }
                Err(e) => {
                    let _ = writeln!(writer, "Failed to wait on event: {:?}", e);
                }
            }
        }
        Err(e) => {
            let _ = writeln!(writer, "Failed to create event: {:?}", e);
        }
    }

    let _ = writeln!(writer, "All tests completed successfully");

    0
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut writer = StdoutWriter;
    let _ = writeln!(writer, "PANIC: {:?}", info);
    libsys::Process::exit(1)
}
