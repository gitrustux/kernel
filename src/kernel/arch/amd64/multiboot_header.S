// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Multiboot2 Header and PVH ELF Note for QEMU Boot
//!
//! This allows QEMU to boot the x86_64 kernel directly using the -kernel option.
//! - Multiboot2 header for legacy bootloaders
//! - PVH ELF note for modern QEMU/Linux boot protocols

// PVH (Protected Mode Virtualization) ELF Note
// This tells QEMU that this kernel supports the PVH entry point
.section .note.pv, "a"
.align 4

pvh_note:
    .long 2f - 1f           // namesz
    .long 4f - 3f           // descsz
    .long 0x49534d56        // type: "VMS" + 1 (Linux PVH note type)
1:  .asciz "Linux"         // name (must be "Linux" for PVH)
2:  .align 4
3:  .quad 0                // entry point (0 means use ELF e_entry)
4:  .align 4

// Multiboot2 header for legacy bootloaders
.set ALIGN, 1<<0           // Align loaded modules on page boundaries
.set MEMINFO, 1<<1         // Provide memory map
.set FLAGS, ALIGN | MEMINFO
.set HEADER_SIZE, (end_multiboot_header - multiboot_header)

.section .multiboot, "a"
.align 8

multiboot_header:
    .long 0xe85250d6       // Magic number
    .long 0                // Architecture (i386)
    .long HEADER_SIZE      // Header length
    .long -(0xe85250d6 + 0 + HEADER_SIZE)  // Checksum

    // End tag
    .short 0               // type
    .short 0               // flags
    .long 8                // size

end_multiboot_header:
