// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Build script for Rustux kernel
//!
//! This build script compiles architecture-specific C code
//! that provides low-level operations as a bridge to Rust.

use std::env;
use std::path::PathBuf;

fn main() {
    // Get the target architecture
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Only build C code for x86_64
    if target_arch == "x86_64" {
        build_x86_64_c();
        configure_linker_x86_64();
    }
}

fn configure_linker_x86_64() {
    // Use custom linker script for bare-metal kernel
    println!("cargo:rustc-link-arg=-Tsrc/kernel/kernel_minimal.ld");

    // Set entry point
    println!("cargo:rustc-link-arg=-entry=kmain");

    // Disable standard library startup files
    println!("cargo:rustc-link-arg=-nostartfiles");

    // Disable PIE
    println!("cargo:rustc-link-arg=-no-pie");

    // Use rust-lld
    println!("cargo:rustc-link-arg=-fuse-ld=lld");

    // Include all objects from sys_x86
    println!("cargo:rustc-link-arg=-Wl,--whole-archive");
    println!("cargo:rustc-link-arg=-Wl,-Bstatic");
}

fn build_x86_64_c() {
    // Get the output directory
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // C source files for x86_64
    let c_sources = vec![
        "src/kernel/arch/amd64/sys_x86.c",
    ];

    // Assembly source files for x86_64
    let asm_sources = vec![
        "src/kernel/arch/amd64/multiboot_header.S",
    ];

    // Compile C code
    let mut cc_build = cc::Build::new();

    for src in &c_sources {
        let full_path = manifest_dir.join(src);
        if full_path.exists() {
            cc_build.file(&full_path);
        }
    }

    // Add assembly sources
    for src in &asm_sources {
        let full_path = manifest_dir.join(src);
        if full_path.exists() {
            cc_build.file(&full_path);
        }
    }

    // Set compiler flags for bare-metal x86_64
    cc_build
        .include(manifest_dir.join("src/kernel/arch/amd64"))
        .warnings(true)
        .flag("-ffreestanding")
        .flag("-fno-builtin")
        .flag("-fno-stack-protector")
        .flag("-m64")
        .flag("-march=x86-64")
        .flag("-mno-red-zone")     // Disable red zone for kernel
        .flag("-fno-PIE")          // No position independent code
        .flag("-fno-pie")          // No position independent executable
        .define("__x86_64__", None)
        .define("__RUSTUX_KERNEL__", None);

    // Compile
    cc_build.compile("sys_x86");

    // Link search directory
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sys_x86");

    // Find and directly link the multiboot header object file
    // This ensures it's included even without references
    let multiboot_obj = out_dir.join("multiboot_header.o");
    if multiboot_obj.exists() {
        println!("cargo:rustc-link-arg={}", multiboot_obj.display());
    } else {
        // Try to find it with a hash prefix
        if let Ok(entries) = std::fs::read_dir(&out_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.to_string_lossy().contains("multiboot_header") {
                    println!("cargo:rustc-link-arg={}", path.display());
                    break;
                }
            }
        }
    }

    // Rebuild if C files change
    for src in &c_sources {
        let full_path = manifest_dir.join(src);
        if full_path.exists() {
            println!("cargo:rerun-if-changed={}", full_path.display());
        }
    }

    // Rebuild if assembly files change
    for src in &asm_sources {
        let full_path = manifest_dir.join(src);
        if full_path.exists() {
            println!("cargo:rerun-if-changed={}", full_path.display());
        }
    }

    // Rebuild if header changes
    let header_path = manifest_dir.join("src/kernel/arch/amd64/sys_x86.h");
    if header_path.exists() {
        println!("cargo:rerun-if-changed={}", header_path.display());
    }
}
