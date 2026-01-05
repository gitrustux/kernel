/* Copyright 2025 The Rustux Authors
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */

/* RISC-V 64-bit exception vector and trampoline
 *
 * This file contains the exception entry points for RISC-V.
 * It follows the standard RISC-V exception handling convention.
 */

.section .text.exceptions

/* Exception vector - must be 256-byte aligned */
.align 9
.globl riscv_exception_vector_table
riscv_exception_vector_table:

    /* 1. Instruction address misaligned */
    .align 2
    j handle_exception

    /* 2. Instruction access fault */
    .align 2
    j handle_exception

    /* 3. Illegal instruction */
    .align 2
    j handle_exception

    /* 4. Breakpoint */
    .align 2
    j handle_exception

    /* 5. Load address misaligned */
    .align 2
    j handle_exception

    /* 6. Load access fault */
    .align 2
    j handle_exception

    /* 7. Store/AMO address misaligned */
    .align 2
    j handle_exception

    /* 8. Store/AMO access fault */
    .align 2
    j handle_exception

    /* 9. Environment call from U-mode */
    .align 2
    j handle_syscall

    /* 10. Environment call from S-mode */
    .align 2
    j handle_exception

    /* 11. Reserved */
    .align 2
    j handle_exception

    /* 12. Instruction page fault */
    .align 2
    j handle_exception

    /* 13. Load page fault */
    .align 2
    j handle_exception

    /* 14. Reserved */
    .align 2
    j handle_exception

    /* 15. Store/AMO page fault */
    .align 2
    j handle_exception

    /* 16-31: Reserved */
    .rept 16
    .align 2
    j handle_exception
    .endr

    /* Exception entry point - saves registers and calls Rust handler */
handle_exception:
    /* TODO: Implement full exception save/restore
     * 1. Save all registers to stack
     * 2. Call Rust exception handler
     * 3. Restore registers and return
     */
    j .

handle_syscall:
    /* RISC-V System Call Entry Point
     *
     * Calling convention:
     * - Syscall number: a7
     * - Arguments: a0-a5
     * - Return value: a0
     *
     * This function:
     * 1. Saves all user registers to stack
     * 2. Validates syscall number
     * 3. Calls Rust syscall dispatcher
     * 4. Restores registers and returns to user mode
     */

    /* Allocate space on stack for saving registers
     * We need to save: ra, sp, gp, tp, t0-t6, s0-s11, a0-a7
     * That's 1+1+1+1+7+16+8 = 35 registers = 280 bytes
     * Plus 8 bytes for syscall number and alignment
     */
    addi sp, sp, -304

    /* Save all callee-saved and temporary registers FIRST */
    sd ra, 24(sp)
    sd gp, 32(sp)
    sd tp, 40(sp)

    sd t0, 48(sp)
    sd t1, 56(sp)
    sd t2, 64(sp)
    sd t3, 72(sp)
    sd t4, 80(sp)
    sd t5, 88(sp)
    sd t6, 96(sp)

    /* Save user SP (use t0 as scratch since we already saved it) */
    mv t0, sp
    addi t0, t0, 304
    sd t0, 16(sp)

    sd s0, 104(sp)
    sd s1, 112(sp)
    sd s2, 120(sp)
    sd s3, 128(sp)
    sd s4, 136(sp)
    sd s5, 144(sp)
    sd s6, 152(sp)
    sd s7, 160(sp)
    sd s8, 168(sp)
    sd s9, 176(sp)
    sd s10, 184(sp)
    sd s11, 192(sp)

    /* Save syscall number from a7 */
    sd a7, 200(sp)

    /* Prepare syscall arguments for syscall_dispatch
     * SyscallArgs struct layout (offset from sp):
     * +208: number (u32) - syscall number
     * +212: args[0] (usize)
     * +220: args[1] (usize)
     * +228: args[2] (usize)
     * +236: args[3] (usize)
     * +244: args[4] (usize)
     * +252: args[5] (usize)
     */

    /* Store syscall number */
    sw a7, 208(sp)

    /* Store argument registers a0-a5 */
    sd a0, 212(sp)
    sd a1, 220(sp)
    sd a2, 228(sp)
    sd a3, 236(sp)
    sd a4, 244(sp)
    sd a5, 252(sp)

    /* Call syscall_dispatch(&SyscallArgs)
     * Pass pointer to SyscallArgs in a0
     */
    addi a0, sp, 208
    call syscall_dispatch

    /* syscall_dispatch returns isize in a0
     * This becomes our return value
     */

    /* Restore all registers */
    ld ra, 24(sp)
    ld gp, 32(sp)
    ld tp, 40(sp)

    ld t0, 48(sp)
    ld t1, 56(sp)
    ld t2, 64(sp)
    ld t3, 72(sp)
    ld t4, 80(sp)
    ld t5, 88(sp)
    ld t6, 96(sp)

    ld s0, 104(sp)
    ld s1, 112(sp)
    ld s2, 120(sp)
    ld s3, 128(sp)
    ld s4, 136(sp)
    ld s5, 144(sp)
    ld s6, 152(sp)
    ld s7, 160(sp)
    ld s8, 168(sp)
    ld s9, 176(sp)
    ld s10, 184(sp)
    ld s11, 192(sp)

    /* Restore user SP */
    ld sp, 16(sp)

    /* Return to user mode via sret */
    sret

.size riscv_exception_vector_table, . - riscv_exception_vector_table
