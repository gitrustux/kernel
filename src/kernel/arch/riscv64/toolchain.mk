# Copyright 2025 Rustux Authors
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT

# RISC-V 64-bit GCC toolchain
ifndef ARCH_riscv64_TOOLCHAIN_INCLUDED
ARCH_riscv64_TOOLCHAIN_INCLUDED := 1

ifndef ARCH_riscv64_TOOLCHAIN_PREFIX
ARCH_riscv64_TOOLCHAIN_PREFIX := riscv64-elf-
endif
FOUNDTOOL=$(shell which $(ARCH_riscv64_TOOLCHAIN_PREFIX)gcc)

endif # ifndef ARCH_riscv64_TOOLCHAIN_INCLUDED

# Clang
ifeq ($(call TOBOOL,$(USE_CLANG)),true)
FOUNDTOOL=$(shell which $(CLANG_TOOLCHAIN_PREFIX)clang)
endif # USE_CLANG==true

# Rust
ifeq ($(call TOBOOL,$(USE_RUST)),true)
ifndef RUST_TOOLCHAIN_PREFIX
RUST_TOOLCHAIN_PREFIX :=
endif
RUST_CC=$(shell which $(RUST_TOOLCHAIN_PREFIX)rustc)
ifeq ($(RUST_CC),)
$(error cannot find Rust compiler, please set RUST_TOOLCHAIN_PREFIX or add rustc to your path)
endif
endif # USE_RUST==true

ifeq ($(FOUNDTOOL),)
$(error cannot find toolchain, please set ARCH_riscv64_TOOLCHAIN_PREFIX, \
        CLANG_TOOLCHAIN_PREFIX, or add either to your path)
endif
