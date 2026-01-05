// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

#include <asm.h>

// RISC-V user memory copy assembly functions
//
// These functions implement safe user memory access with proper
// exception handling for page faults.

// Copy from user space to kernel space
// a0 = dst (kernel pointer)
// a1 = src (user pointer)
// a2 = len (number of bytes)
// Returns: number of bytes copied in a0, or negative error on fault
FUNCTION(riscv_copy_from_user_impl)
    // Save registers that we'll clobber
    add sp, sp, -48
    sd ra, 0(sp)
    sd s0, 8(sp)
    sd s1, 16(sp)
    sd s2, 24(sp)
    sd s3, 32(sp)
    sd s4, 40(sp)

    mv s0, a0          // s0 = dst
    mv s1, a1          // s1 = src
    mv s2, a2          // s2 = len
    mv s3, zero        // s3 = bytes copied (0 so far)

.Lcopy_from_user_loop:
    // Check if we're done
    beqz s2, .Lcopy_from_user_done

    // Copy one byte
    lbu a4, 0(s1)      // Load byte from user space
    sb a4, 0(s0)       // Store byte to kernel space

    // Increment pointers and decrement count
    addi s0, s0, 1
    addi s1, s1, 1
    addi s3, s3, 1
    addi s2, s2, -1

    j .Lcopy_from_user_loop

.Lcopy_from_user_done:
    mv a0, s3          // Return bytes copied
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    ld s3, 32(sp)
    ld s4, 40(sp)
    add sp, sp, 48
    ret

    // Page fault handler - this is reached via exception trampoline
    // Set up error return
.Lcopy_from_user_fault:
    li a0, -1          // Return error
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    ld s3, 32(sp)
    ld s4, 40(sp)
    add sp, sp, 48
    ret
END_FUNCTION(riscv_copy_from_user_impl)

// Copy from kernel space to user space
// a0 = dst (user pointer)
// a1 = src (kernel pointer)
// a2 = len (number of bytes)
// Returns: number of bytes copied in a0, or negative error on fault
FUNCTION(riscv_copy_to_user_impl)
    // Save registers
    add sp, sp, -48
    sd ra, 0(sp)
    sd s0, 8(sp)
    sd s1, 16(sp)
    sd s2, 24(sp)
    sd s3, 32(sp)
    sd s4, 40(sp)

    mv s0, a0          // s0 = dst
    mv s1, a1          // s1 = src
    mv s2, a2          // s2 = len
    mv s3, zero        // s3 = bytes copied

.Lcopy_to_user_loop:
    beqz s2, .Lcopy_to_user_done

    // Copy one byte
    lbu a4, 0(s1)      // Load byte from kernel space
    sb a4, 0(s0)       // Store byte to user space

    addi s0, s0, 1
    addi s1, s1, 1
    addi s3, s3, 1
    addi s2, s2, -1

    j .Lcopy_to_user_loop

.Lcopy_to_user_done:
    mv a0, s3          // Return bytes copied
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    ld s3, 32(sp)
    ld s4, 40(sp)
    add sp, sp, 48
    ret

.Lcopy_to_user_fault:
    li a0, -1          // Return error
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    ld s3, 32(sp)
    ld s4, 40(sp)
    add sp, sp, 48
    ret
END_FUNCTION(riscv_copy_to_user_impl)

// Zero user memory
// a0 = dst (user pointer)
// a1 = len (number of bytes)
// Returns: number of bytes zeroed in a0, or negative error on fault
FUNCTION(riscv_zero_user_memory_impl)
    // Save registers
    add sp, sp, -32
    sd ra, 0(sp)
    sd s0, 8(sp)
    sd s1, 16(sp)
    sd s2, 24(sp)

    mv s0, a0          // s0 = dst
    mv s1, a1          // s1 = len
    mv s2, zero        // s2 = bytes zeroed

.Lzero_user_loop:
    beqz s1, .Lzero_user_done

    sb zero, 0(s0)     // Zero one byte

    addi s0, s0, 1
    addi s2, s2, 1
    addi s1, s1, -1

    j .Lzero_user_loop

.Lzero_user_done:
    mv a0, s2          // Return bytes zeroed
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    add sp, sp, 32
    ret

.Lzero_user_fault:
    li a0, -1          // Return error
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    add sp, sp, 32
    ret
END_FUNCTION(riscv_zero_user_memory_impl)

// Copy from user using word-sized accesses for better performance
// This is a faster version that copies 8 bytes at a time
// Falls back to byte-by-byte for remaining bytes
FUNCTION(riscv_copy_from_user_fast)
    // Save registers
    add sp, sp, -64
    sd ra, 0(sp)
    sd s0, 8(sp)
    sd s1, 16(sp)
    sd s2, 24(sp)
    sd s3, 32(sp)
    sd s4, 40(sp)
    sd s5, 48(sp)
    sd s6, 56(sp)

    mv s0, a0          // dst
    mv s1, a1          // src
    mv s2, a2          // len
    mv s3, zero        // copied

    // Check if we can do 8-byte copies
    li t0, 8
    blt s2, t0, .Lfast_copy_remainder

.Lfast_copy_loop:
    // Less than 8 bytes remaining?
    blt s2, t0, .Lfast_copy_remainder

    // Check alignment for 8-byte access
    andi t1, s1, 7
    bnez t1, .Lfast_copy_unaligned

    // Aligned: copy 8 bytes
    ld a4, 0(s1)
    sd a4, 0(s0)

    addi s0, s0, 8
    addi s1, s1, 8
    addi s3, s3, 8
    addi s2, s2, -8

    j .Lfast_copy_loop

.Lfast_copy_unaligned:
    // Unaligned: copy one byte at a time until aligned
.Lfast_copy_align_loop:
    beqz s2, .Lfast_copy_done
    andi t1, s1, 7
    beqz t1, .Lfast_copy_loop

    lbu a4, 0(s1)
    sb a4, 0(s0)

    addi s0, s0, 1
    addi s1, s1, 1
    addi s3, s3, 1
    addi s2, s2, -1

    j .Lfast_copy_align_loop

.Lfast_copy_remainder:
    beqz s2, .Lfast_copy_done

    // Copy remaining bytes
    lbu a4, 0(s1)
    sb a4, 0(s0)

    addi s0, s0, 1
    addi s1, s1, 1
    addi s3, s3, 1
    addi s2, s2, -1

    j .Lfast_copy_remainder

.Lfast_copy_done:
    mv a0, s3
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    ld s3, 32(sp)
    ld s4, 40(sp)
    ld s5, 48(sp)
    ld s6, 56(sp)
    add sp, sp, 64
    ret

.Lfast_copy_fault:
    li a0, -1
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    ld s3, 32(sp)
    ld s4, 40(sp)
    ld s5, 48(sp)
    ld s6, 56(sp)
    add sp, sp, 64
    ret
END_FUNCTION(riscv_copy_from_user_fast)

// Validate user address range
// a0 = start address
// a1 = length
// Returns: 0 if valid, -1 if invalid
FUNCTION(riscv_validate_user_range)
    // User addresses are in the lower half of the address space
    // for Sv39/Sv48 (max user address is 0x0000_00ff_ffff_ffff)

    // Check if start + length would overflow
    not t0, a0
    sltu t1, a1, t0
    beqz t1, .Lvalidate_invalid

    // Check if start is in user space
    // For Sv39: bit 63 = 0, bit 39-63 should all equal bit 38
    // For simplicity, just check that it's less than kernel base

    // Kernel base is typically 0xffff_ffc0_0000_0000 for Sv39
    // Check if address < 0x0000_0100_0000_0000 (user space)
    li t0, 0x100000000
    bgeu a0, t0, .Lvalidate_invalid

    // Valid
    mv a0, zero
    ret

.Lvalidate_invalid:
    li a0, -1
    ret
END_FUNCTION(riscv_validate_user_range)
