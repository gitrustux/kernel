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
    }
}

fn build_x86_64_c() {
    // Get the output directory
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // C source files for x86_64
    let c_sources = vec![
        "src/kernel/arch/amd64/sys_x86.c",
    ];

    // Compile C code
    let mut cc_build = cc::Build::new();

    for src in &c_sources {
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

    // Rebuild if C files change
    for src in &c_sources {
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
