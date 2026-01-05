/* Copyright 2025 The Rustux Authors
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */

/* RISC-V 64-bit assembly utilities */

.section .text

/* Memory barriers */
.globl riscv_mb
riscv_mb:
    fence iorw, iorw
    ret

.globl riscv_smp_mb
riscv_smp_mb:
    fence iorw, iorw
    ret

/*
 * RISC-V context switch
 *
 * void riscv_context_switch(uint64_t *old_sp, uint64_t new_sp)
 *
 * This function saves the current context to the stack pointer
 * pointed to by old_sp, then switches to the new context.
 *
 * Calling convention:
 * a0 = pointer to location to save current stack pointer
 * a1 = new stack pointer (points to saved context on new stack)
 *
 * Saved context layout (on stack, growing downward):
 *   s0-s11 (callee-saved registers)
 *   ra (return address)
 *
 * The context is organized such that we can pop it all at once
 * when switching back to this thread.
 */
.globl riscv_context_switch
.type riscv_context_switch, @function
riscv_context_switch:
    /* Save callee-saved registers on current stack
     * We need to save: ra, s0-s11 (13 registers total)
     * Stack must remain 16-byte aligned per ABI
     */

    /* Add space for saved registers (13 * 8 = 104 bytes)
     * Round up to 112 bytes for 16-byte alignment
     */
    addi sp, sp, -112

    /* Save return address (ra = x1) */
    sd ra, 104(sp)

    /* Save callee-saved registers s0-s11
     * s0 = x8, s1 = x9, s2-s11 = x18-x27
     */
    sd s0, 96(sp)
    sd s1, 88(sp)
    sd s2, 80(sp)
    sd s3, 72(sp)
    sd s4, 64(sp)
    sd s5, 56(sp)
    sd s6, 48(sp)
    sd s7, 40(sp)
    sd s8, 32(sp)
    sd s9, 24(sp)
    sd s10, 16(sp)
    sd s11, 8(sp)

    /* Save current stack pointer to old_sp location
     * a0 contains the address to save sp to
     */
    sd sp, 0(a0)

    /* Load new stack pointer
     * a1 contains the new sp value
     * Note: new_sp should point to where the saved context starts
     * We need to adjust it because we saved above the current sp
     */
    mv sp, a1

    /* Restore callee-saved registers from new stack */
    ld s0, 96(sp)
    ld s1, 88(sp)
    ld s2, 80(sp)
    ld s3, 72(sp)
    ld s4, 64(sp)
    ld s5, 56(sp)
    ld s6, 48(sp)
    ld s7, 40(sp)
    ld s8, 32(sp)
    ld s9, 24(sp)
    ld s10, 16(sp)
    ld s11, 8(sp)

    /* Restore return address */
    ld ra, 104(sp)

    /* Clean up stack and return */
    addi sp, sp, 112
    ret

.size riscv_context_switch, . - riscv_context_switch

/* Flush TLB */
.globl riscv_tlb_flush
riscv_tlb_flush:
    /* sfence.vma - flushes all TLB entries */
    sfence.vma
    ret

.globl riscv_tlb_flush_asid
riscv_tlb_flush_asid:
    /* sfence.vma asid - flushes TLB entries for specific ASID */
    sfence.vma a0
    ret

.globl riscv_tlb_flush_page
riscv_tlb_flush_page:
    /* sfence.vma addr, asid - flushes specific TLB entry */
    /* a0 = virtual address, a1 = asid (0 for all ASIDs) */
    sfence.vma a0, a1
    ret

/* Halt instruction */
.globl riscv_halt
riscv_halt:
    /* Wait for next interrupt */
    wfi
    ret

/* Pause for spin-waiting */
.globl riscv_pause
riscv_pause:
    /* Hint to CPU that we're in a spin-wait loop */
    /* No specific pause instruction on RISC-V, use fence instead */
    fence
    ret

/* Get current stack pointer */
.globl riscv_get_sp
.type riscv_get_sp, @function
riscv_get_sp:
    mv a0, sp
    ret
.size riscv_get_sp, . - riscv_get_sp

/* Set stack pointer */
.globl riscv_set_sp
.type riscv_set_sp, @function
riscv_set_sp:
    mv sp, a0
    ret
.size riscv_set_sp, . - riscv_set_sp
