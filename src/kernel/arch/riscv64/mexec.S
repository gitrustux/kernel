// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

#include <asm.h>

// RISC-V Multi-Exec (mexec) header
//
// Provides support for kexec-style multi-boot functionality,
// allowing the kernel to boot another kernel without going through firmware.

// The mexec header is placed at a known location and contains
// information needed to jump to a new kernel image.

// mexec header structure
// This must match the C struct definition
.mexec_header:
    .quad 0x52495356  // Magic: "RISCV" in little endian
    .quad 0           // Reserved
    .quad 0           // Entry point (physical address)
    .quad 0           // Device tree blob (physical address)
    .quad 0           // Kernel image size
    .quad 0           // Flags
    .quad 0           // Reserved
    .quad 0           // Reserved
GLOBAL_FUNCTION(mexec_header)

// Prepare for multi-exec jump
// a0 = entry_point (physical address of new kernel)
// a1 = dtb (physical address of device tree blob)
// a2 = flags (reserved, must be 0)
FUNCTION(riscv_mexec_prepare)
    // Disable interrupts
    csrw sie, zero
    csrw sip, zero

    // Disable timer interrupts
    csrw stimecmp, -1

    // Flush all caches
    // RISC-V doesn't have a direct cache flush instruction
    // but we can use fence.i to flush the instruction cache
    fence.i

    // Ensure all memory operations are complete
    fence

    // Save parameters in the mexec header
    la t0, mexec_header
    sd a0, 16(t0)   // Save entry point
    sd a1, 24(t0)   // Save dtb

    // Return success
    mv a0, zero
    ret
END_FUNCTION(riscv_mexec_prepare)

// Execute the multi-exec jump
// This function does not return
// a0 = entry_point (physical address)
// a1 = hart_id (current hart ID)
FUNCTION(riscv_mexec_jump)
    // a0 = entry point
    // a1 = hart_id

    // Disable MMU by setting SATP to 0
    csrw satp, zero

    // Flush TLB
    sfence.vma

    // Disable FPU
    csrrci t0, sstatus, (3 << 13)  // Clear FS field

    // Disable all interrupts
    csrw sie, zero
    csrw sip, zero

    // Reset stack pointer to a safe location
    // Use a temporary stack in low memory
    li sp, 0x8000
    addi sp, sp, -16

    // Clear all general-purpose registers
    mv t0, zero
    mv t1, zero
    mv t2, zero
    mv t3, zero
    mv t4, zero
    mv t5, zero
    mv t6, zero
    // Don't clear a0 (entry point) or a1 (hart_id)

    // Jump to the new kernel
    // The new kernel expects:
    // a0 = hart_id
    // a1 = device tree blob (if available)

    mv t0, a0    // t0 = entry point
    mv a0, a1    // a0 = hart_id

    // Load entry point into ra and jump
    jr t0

    // Should never return
1:
    wfi
    j 1b
END_FUNCTION(riscv_mexec_jump)

// Halt all harts and prepare for reboot
// This is called when initiating a system restart
FUNCTION(riscv_system_halt)
    // Disable all interrupts
    csrw sie, zero

    // Disable timer
    li t0, -1
    csrw stimecmp, t0

    // Infinite halt loop
1:
    wfi
    j 1b
END_FUNCTION(riscv_system_halt)

// System reset (if supported)
// Attempts to trigger a system reset through the watchdog
// or other reset mechanism
FUNCTION(riscv_system_reset)
    // TODO: Implement platform-specific reset
    // For now, just halt
    tail riscv_system_halt
END_FUNCTION(riscv_system_reset)

// Get mexec header address
// Returns: a0 = pointer to mexec header
FUNCTION(riscv_get_mexec_header)
    la a0, mexec_header
    ret
END_FUNCTION(riscv_get_mexec_header)

// Validate mexec header
// a0 = pointer to candidate mexec header
// Returns: a0 = 0 if valid, negative if invalid
FUNCTION(riscv_validate_mexec_header)
    // Check magic number
    lw t0, 0(a0)
    li t1, 0x56534952  // "RISCV" in little endian (stored as word)
    bne t0, t1, .Linvalid

    // Check reserved fields are zero
    ld t0, 8(a0)
    bnez t0, .Linvalid

    // Valid
    mv a0, zero
    ret

.Linvalid:
    li a0, -1
    ret
END_FUNCTION(riscv_validate_mexec_header)

// Crash kernel jump
// Jumps to a crash kernel with saved crash information
// a0 = crash kernel entry point
// a1 = crash info pointer
// a2 = crash info size
FUNCTION(riscv_crash_jump)
    // Similar to mexec_jump but preserves crash info

    // Disable MMU
    csrw satp, zero
    sfence.vma

    // Disable interrupts
    csrw sie, zero
    csrw sip, zero

    // Set up registers for crash kernel
    // a0 = crash info pointer
    // a1 = crash info size
    mv a0, a1
    mv a1, a2

    // Jump to crash kernel
    jr t0

    // Should never return
1:
    wfi
    j 1b
END_FUNCTION(riscv_crash_jump)

// Align the mexec header to a 256-byte boundary
.align 8

// mexec header is defined above
// Ensure it's at the end of this file

// Size of mexec header (for validation)
.set MEXEC_HEADER_SIZE, . - mexec_header
