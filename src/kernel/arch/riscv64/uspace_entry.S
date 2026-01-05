// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

#include <asm.h>

// RISC-V user space entry
//
// Transitions from supervisor mode (S-mode) to user mode (U-mode)
// by setting up the trap frame and executing an sret instruction.

// Entry point to user space
// a0 = arg1 (user function argument)
// a1 = arg2 (user function argument)
// a2 = sp (user stack pointer)
// a3 = pc (user program counter / entry point)
// a4 = a0_user (value to load into user's a0 register)
FUNCTION(riscv_uspace_entry)
    // Save our sp (kernel sp) in tp (thread pointer register)
    // This allows us to restore it when coming back from user space
    mv tp, sp

    // Set up the trap frame that sret will use
    // sret loads pc from sepc and status from sstatus

    // Set sstatus.SIE to enable interrupts in user mode
    // Clear sstatus.SPP to indicate we're returning to user mode
    csrrsi t0, sstatus, 1  // Set SIE (interrupt enable)
    csrrc t0, sstatus, (1 << 8)  // Clear SPP (previous privilege = user)

    // Set the user program counter
    csrw sepc, a3

    // Set user stack pointer
    mv sp, a2

    // Set up user registers a0 and a1
    mv a0, a4          // a0 = a0_user (may differ from arg1)
    mv a1, a1          // a1 = arg2

    // Clear all other registers to avoid leaking kernel data
    // (except for tp which holds kernel sp)
    mv t0, zero
    mv t1, zero
    mv t2, zero
    mv t3, zero
    mv t4, zero
    mv t5, zero
    mv t6, zero
    // s0-s2 are callee-saved, should already be saved
    // a2-a7 are scratch, clear them
    mv a2, zero
    mv a3, zero
    mv a4, zero
    mv a5, zero
    mv a6, zero
    mv a7, zero
    // s0-s11 should be in thread context
    // ft0-ft11, fa0-fa7, fs0-fs11 are FPU, handled separately

    // Execute sret to enter user space
    // sret:
    // 1. Sets PC to sepc
    // 2. Sets privilege mode based on SPP bit (0 = user, 1 = supervisor)
    // 3. Enables interrupts if SIE is set
    sret
END_FUNCTION(riscv_uspace_entry)

// Alternative entry point with simpler calling convention
// a0 = sp (user stack pointer)
// a1 = pc (user program counter)
// a2 = arg (user function argument, goes into a0)
FUNCTION(riscv_uspace_entry_simple)
    // Save kernel sp in tp
    mv tp, sp

    // Set sstatus for user return
    csrrsi t0, sstatus, 1  // Set SIE
    csrrc t0, sstatus, (1 << 8)  // Clear SPP

    // Set user PC
    csrw sepc, a1

    // Set user stack
    mv sp, a0

    // Set user argument
    mv a0, a2

    // Clear other registers
    mv t0, zero
    mv t1, zero
    mv t2, zero
    mv t3, zero
    mv t4, zero
    mv t5, zero
    mv t6, zero
    mv a1, zero
    mv a3, zero
    mv a4, zero
    mv a5, zero
    mv a6, zero
    mv a7, zero

    // Enter user space
    sret
END_FUNCTION(riscv_uspace_entry_simple)

// Fork entry point
// Used when creating a new thread/process that starts in user space
// a0 = sp (user stack pointer)
// a1 = pc (user program counter)
// a2 = arg1 (user function argument)
// a3 = arg2 (user function argument)
FUNCTION(riscv_uspace_fork_entry)
    // Save kernel sp
    mv tp, sp

    // Set sstatus for user return
    csrrsi t0, sstatus, 1  // Set SIE
    csrrc t0, sstatus, (1 << 8)  // Clear SPP

    // Set user PC
    csrw sepc, a1

    // Set user stack
    mv sp, a0

    // Set user arguments (a0 and a1)
    mv a0, a2
    mv a1, a3

    // Clear other registers
    mv t0, zero
    mv t1, zero
    mv t2, zero
    mv t3, zero
    mv t4, zero
    mv t5, zero
    mv t6, zero
    mv a2, zero
    mv a3, zero
    mv a4, zero
    mv a5, zero
    mv a6, zero
    mv a7, zero

    // Enter user space
    sret
END_FUNCTION(riscv_uspace_fork_entry)

// Exit to user space from exception return
// This is used when returning from a system call or exception
// a0 = iframe pointer (pointer to RiscvIframe structure)
//
// The iframe structure contains saved register state
// This function restores the user state and executes sret
FUNCTION(riscv_uspace_exception_return)
    // a0 points to iframe
    // iframe layout:
    // 0-247: registers ra, sp, gp, tp, t0-t6, s0-s11, a0-a7
    // 248: pc
    // 256: status
    // 264: cause
    // 272: tval

    // Restore saved registers from iframe
    // First restore callee-saved registers
    ld s0, 80(a0)
    ld s1, 88(a0)
    ld s2, 96(a0)
    ld s3, 104(a0)
    ld s4, 112(a0)
    ld s5, 120(a0)
    ld s6, 128(a0)
    ld s7, 136(a0)
    ld s8, 144(a0)
    ld s9, 152(a0)
    ld s10, 160(a0)
    ld s11, 168(a0)

    // Restore tp (thread pointer, may have kernel sp)
    ld tp, 28(a0)

    // Restore user stack pointer
    ld sp, 8(a0)

    // Restore user GP
    ld gp, 16(a0)

    // Restore user ra
    ld ra, 0(a0)

    // Restore temporary registers
    ld t0, 32(a0)
    ld t1, 40(a0)
    ld t2, 48(a0)
    ld t3, 56(a0)
    ld t4, 64(a0)
    ld t5, 72(a0)
    ld t6, 76(a0)

    // Restore argument registers
    ld a0, 112(a0)  // Original user a0 (return value)
    ld a1, 120(a0)
    ld a2, 128(a0)
    ld a3, 136(a0)
    ld a4, 144(a0)
    ld a5, 152(a0)
    ld a6, 160(a0)
    ld a7, 168(a0)

    // Set sstatus from saved status
    ld t0, 256(a0)
    csrw sstatus, t0

    // Set PC from saved pc
    ld t0, 248(a0)
    csrw sepc, t0

    // Execute sret to return to user space
    sret
END_FUNCTION(riscv_uspace_exception_return)

// Resume execution after signal
// Similar to exception return but potentially modifies PC
// to jump to signal handler
// a0 = iframe pointer
// a1 = new pc (signal handler address)
// a2 = new sp (signal stack)
// a3 = signal number
FUNCTION(riscv_uspace_resume_to_signal)
    // Load iframe pointer
    mv t0, a0

    // Save current PC and SP for signal handler info
    // (normally pushed to user stack)

    // Modify iframe with new PC and SP
    sd a1, 248(t0)  // Set new PC
    sd a2, 8(t0)    // Set new SP

    // Set up signal number in a0
    // (will be restored as user's a0)
    mv a1, a3

    // Use standard exception return path
    tail riscv_uspace_exception_return
END_FUNCTION(riscv_uspace_resume_to_signal)
