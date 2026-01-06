/*
 * Minimal linker script for Rustux kernel (Rust/Cargo build)
 *
 * Note: This project was originally designed to use a Makefile-based
 * build system. The Cargo build compiles the code successfully,
 * but full linking and QEMU booting requires the original build system.
 */

ENTRY(kmain)

SECTIONS
{
    . = 1M;

    .text : {
        *(.multiboot)
        *(.text.boot)
        *(.text.*)
        *(.text*)
    } :text

    .rodata : {
        *(.rodata.*)
        *(.rodata*)
    } :text

    .data : {
        *(.data.*)
        *(.data*)
    } :data

    .bss : {
        *(.bss.*)
        *(.bss*)
        *(COMMON)
    } :data

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note.GNU-stack)
        *(.gcc_except_table*)
    }
}

PHDRS
{
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
