/* Copyright 2025 The Rustux Authors
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */

/* RISC-V 64-bit kernel entry point
 *
 * This is the first code executed after the bootloader/SBI hands control
 * to the kernel. The hart is in M-mode at this point.
 *
 * Calling convention:
 * a0 = hart ID
 * a1 = device tree pointer (may be 0)
 */

.section .text.boot
.globl _start
.type _start, @function
_start:
    /* Disable interrupts while setting up */
    csrw mie, zero

    /* Save hart ID from a0 */
    csrw tp, a0  /* Use tp (thread pointer) as scratch register */

    /* Check if we're the boot hart (hart 0) or a secondary hart */
    li t0, 0
    bne a0, t0, secondary_hart

boot_hart:
    /* ============ Boot Hart (hart 0) Initialization ============ */

    /* Set up stack pointer */
    la sp, _stack_start
    li t0, -16
    and sp, sp, t0  /* Align to 16 bytes */

    /* Clear BSS */
    la t0, _bss_start
    la t1, _bss_end
clear_bss:
    beq t0, t1, bss_done
    sb zero, 0(t0)
    addi t0, t0, 1
    j clear_bss
bss_done:

    /* ============ Set up S-mode configuration ============ */

    /* Configure mstatus to enable S-mode */
    /* Set MPP to S (01) for return to S-mode */
    li t0, (0x11 << 11)  /* MPP=01, MPIE=1 */
    csrw mstatus, t0

    /* Configure mstatus to enable FPU */
    li t0, (1 << 13)  /* FS=11 (dirty) in mstatus */
    csrw mstatus, t0

    /* Delegate all interrupts and exceptions to S-mode */
    li t0, -1
    csrw mideleg, t0
    csrw medeleg, t0

    /* Set S-mode exception vector */
    la t0, trap_entry
    csrw stvec, t0

    /* Set up S-mode stack pointer */
    la t0, _stack_start
    /* Adjust for the fact that we'll be in S-mode */
    /* We need a separate stack for S-mode */
    /* For now, use the same stack */
    /* (TODO: set up separate S-mode stack) */

    /* Configure PMP (Physical Memory Protection) to give S-mode access */
    /* PMP entry 0: allow all memory */
    li t0, (0xF << 3)  /* R=1, W=1, X=1 (A=4 = NAPOT) */
    csrw pmpaddr0, t0
    li t0, 0xFFFFFFFF  /* Highest address */
    csrw pmpcfg0, t0

    /* ============ Configure page tables ============ */

    /* Load kernel page table root (PNN) into satp */
    /* Mode = Sv39 (8), ASID = 0, PPN = kernel_pml4 */
    la t0, kernel_pml4
    srli t0, t0, 12  /* Convert physical address to PPN */
    li t1, (8 << 60)  /* Sv39 mode */
    or t0, t0, t1
    csrw satp, t0

    /* Flush TLB */
    sfence.vma

    /* ============ Configure S-mode CSRs ============ */

    /* Set SIE (enable S-mode interrupts) */
    li t0, (1 << 1)  /* SSIE */
    csrw sie, t0

    /* Set counter-enable for S-mode */
    li t0, (1 << 2) | (1 << 1)  /* Enable cycle and time counters */
    csrw scounteren, t0

    /* ============ Transition to S-mode ============ */

    /* Set return address */
    la t0, kmain
    csrw mepc, t0

    /* Save device tree pointer (if provided) in a1 */
    /* (We'll pass it to kmain later) */

    /* Enable S-mode and disable MMU for mret */
    li t0, (1 << 12)  /* MXR (allow execute from read-only) */
    csrw mstatus, t0

    /* Use mret to transition to S-mode */
    mret

secondary_hart:
    /* ============ Secondary Hart Initialization ============ */

    /* Set up stack pointer for secondary harts */
    /* TODO: allocate separate stack per hart */
    la sp, _stack_start
    li t0, -16
    and sp, sp, t0

    /* Secondary harts should wait for the boot hart to finish init */
    /* TODO: implement proper MP bringup */
    j secondary_hart_wait

secondary_hart_wait:
    /* Wait for interrupt (boot hart will wake us) */
    wfi
    j secondary_hart_wait

/* Exception vector entry point */
.globl trap_entry
trap_entry:
    /* Save current sp in user SP if in S-mode */
    csrrw sp, sscratch, sp

    /* Save context on stack */
    addi sp, sp, -272

    /* Save general registers */
    sd ra, 0(sp)
    sd gp, 8(sp)
    sd tp, 16(sp)
    sd t0, 24(sp)
    sd t1, 32(sp)
    sd t2, 40(sp)
    sd t3, 48(sp)
    sd t4, 56(sp)
    sd t5, 64(sp)
    sd t6, 72(sp)

    /* Save frame pointer and saved registers */
    sd fp, 80(sp)
    sd s1, 88(sp)
    sd s2, 96(sp)
    sd s3, 104(sp)
    sd s4, 112(sp)
    sd s5, 120(sp)
    sd s6, 128(sp)
    sd s7, 136(sp)
    sd s8, 144(sp)
    sd s9, 152(sp)
    sd s10, 160(sp)
    sd s11, 168(sp)

    /* Save argument registers */
    sd a0, 176(sp)
    sd a1, 184(sp)
    sd a2, 192(sp)
    sd a3, 200(sp)
    sd a4, 208(sp)
    sd a5, 216(sp)
    sd a6, 224(sp)
    sd a7, 232(sp)

    /* Save user SP */
    csrr t0, sscratch
    sd t0, 240(sp)

    /* Save SEPC and SSTATUS */
    csrr t0, sepc
    sd t0, 248(sp)
    csrr t0, sstatus
    sd t0, 256(sp)

    /* Call Rust trap handler */
    /* Pass pointer to saved context */
    mv a0, sp
    call rust_trap_handler

    /* Restore context */
    ld ra, 0(sp)
    ld gp, 8(sp)
    ld tp, 16(sp)
    ld t0, 24(sp)
    ld t1, 32(sp)
    ld t2, 40(sp)
    ld t3, 48(sp)
    ld t4, 56(sp)
    ld t5, 64(sp)
    ld t6, 72(sp)

    ld fp, 80(sp)
    ld s1, 88(sp)
    ld s2, 96(sp)
    ld s3, 104(sp)
    ld s4, 112(sp)
    ld s5, 120(sp)
    ld s6, 128(sp)
    ld s7, 136(sp)
    ld s8, 144(sp)
    ld s9, 152(sp)
    ld s10, 160(sp)
    ld s11, 168(sp)

    ld a0, 176(sp)
    ld a1, 184(sp)
    ld a2, 192(sp)
    ld a3, 200(sp)
    ld a4, 208(sp)
    ld a5, 216(sp)
    ld a6, 224(sp)
    ld a7, 232(sp)

    /* Restore user SP */
    ld t0, 240(sp)
    csrw sscratch, t0

    /* Restore SEPC and SSTATUS */
    ld t0, 248(sp)
    csrw sepc, t0
    ld t0, 256(sp)
    csrw sstatus, t0

    /* Restore stack pointer */
    addi sp, sp, 272

    /* Return from trap */
    sret

.size _start, . - _start

/* Boot stack (defined in linker script) */
.extern _stack_start

/* BSS section */
.extern _bss_start
.extern _bss_end

/* Kernel page table (defined elsewhere) */
.globl kernel_pml4
kernel_pml4:
    .quad 0

/* External symbols */
.extern kmain
.extern rust_trap_handler

/* Boot hart ID (will be set by bootloader) */
.section .data
.globl riscv_boot_hartid
.type riscv_boot_hartid, @object
.size riscv_boot_hartid, 8
riscv_boot_hartid:
    .quad 0

/* Device tree pointer (will be set by bootloader) */
.section .data
.globl riscv_device_tree
.type riscv_device_tree, @object
.size riscv_device_tree, 8
riscv_device_tree:
    .quad 0

/* Exception vector */
.section .data
.globl riscv_exception_vector
.type riscv_exception_vector, @object
.size riscv_exception_vector, 8
riscv_exception_vector:
    .quad trap_entry
