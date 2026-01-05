# Copyright 2025 Rustux Authors
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT

LOCAL_DIR := $(GET_LOCAL_DIR)

MODULE := $(LOCAL_DIR)

# Rust source files
MODULE_RUST_SRCS += \
	$(LOCAL_DIR)/aal.rs \
	$(LOCAL_DIR)/arch.rs \
	$(LOCAL_DIR)/boot_mmu.rs \
	$(LOCAL_DIR)/debugger.rs \
	$(LOCAL_DIR)/exceptions_c.rs \
	$(LOCAL_DIR)/feature.rs \
	$(LOCAL_DIR)/fpu.rs \
	$(LOCAL_DIR)/mmu.rs \
	$(LOCAL_DIR)/mp.rs \
	$(LOCAL_DIR)/periphmap.rs \
	$(LOCAL_DIR)/plic.rs \
	$(LOCAL_DIR)/registers.rs \
	$(LOCAL_DIR)/spinlock.rs \
	$(LOCAL_DIR)/thread.rs \
	$(LOCAL_DIR)/user_copy_c.rs

# Assembly source files
MODULE_SRCS += \
	$(LOCAL_DIR)/asm.S \
	$(LOCAL_DIR)/exceptions.S \
	$(LOCAL_DIR)/mexec.S \
	$(LOCAL_DIR)/start.S \
	$(LOCAL_DIR)/user_copy.S \
	$(LOCAL_DIR)/uspace_entry.S

MODULE_DEPS += \
	kernel/dev/iommu/dummy \
	kernel/lib/bitmap \
	kernel/lib/crashlog \
	kernel/object \

KERNEL_DEFINES += \
	RISCV_ISA_RV64=1

SMP_MAX_CPUS ?= 16

SMP_CPU_MAX_CLUSTERS ?= 1
SMP_CPU_MAX_CLUSTER_CPUS ?= $(SMP_MAX_CPUS)

KERNEL_DEFINES += \
	SMP_MAX_CPUS=$(SMP_MAX_CPUS) \
	SMP_CPU_MAX_CLUSTERS=$(SMP_CPU_MAX_CLUSTERS) \
	SMP_CPU_MAX_CLUSTER_CPUS=$(SMP_CPU_MAX_CLUSTER_CPUS) \

# RISC-V 64-bit virtual memory layout (Sv39 or Sv48)
KERNEL_ASPACE_BASE ?= 0xffffff0000000000
KERNEL_ASPACE_SIZE ?= 0x0000000100000000
USER_ASPACE_BASE   ?= 0x0000000001000000
USER_ASPACE_SIZE   ?= 0x0000fffe00000000

GLOBAL_DEFINES += \
	KERNEL_ASPACE_BASE=$(KERNEL_ASPACE_BASE) \
	KERNEL_ASPACE_SIZE=$(KERNEL_ASPACE_SIZE) \
	USER_ASPACE_BASE=$(USER_ASPACE_BASE) \
	USER_ASPACE_SIZE=$(USER_ASPACE_SIZE)

# Kernel base address for RISC-V 64-bit
KERNEL_BASE := 0xffffffc000000000
BOOT_HEADER_SIZE ?= 0x50

KERNEL_DEFINES += \
	KERNEL_BASE=$(KERNEL_BASE) \

# Try to find the toolchain
include $(LOCAL_DIR)/toolchain.mk
TOOLCHAIN_PREFIX := $(ARCH_$(ARCH)_TOOLCHAIN_PREFIX)

# Setup rust compiler flags for RISC-V 64-bit
RUST_ARCH_FLAGS := --target=riscv64gc-unknown-none-elf
RUST_OPT_LEVEL ?= 2
RUST_EDITION ?= 2021

RUST_FLAGS += \
	$(RUST_ARCH_FLAGS) \
	-C opt-level=$(RUST_OPT_LEVEL) \
	--edition=$(RUST_EDITION) \
	-C panic=abort \
	-C codegen-units=1

# Assembly and C compilation flags
ARCH_COMPILEFLAGS += $(ARCH_$(ARCH)_COMPILEFLAGS)

# Generic RV64GC ISA (IMAFDC)
ARCH_COMPILEFLAGS += -march=rv64gc -mabi=lp64

CLANG_ARCH := riscv64
ifeq ($(call TOBOOL,$(USE_CLANG)),true)
GLOBAL_LDFLAGS += -m elf64lriscv
GLOBAL_MODULE_LDFLAGS += -m elf64lriscv
endif
GLOBAL_LDFLAGS += -z max-page-size=4096

# Kernel hard disables floating point
KERNEL_COMPILEFLAGS += -mgeneral-regs-only

# See engine.mk.
KEEP_FRAME_POINTER_COMPILEFLAGS += -mno-omit-leaf-frame-pointer

KERNEL_COMPILEFLAGS += -fPIE -include kernel/include/hidden.h

# Reserve x4 (tp) for per-CPU pointer in RISC-V ABI
ARCH_COMPILEFLAGS += -ffixed-x4

# Rust-specific build rules
define RUST_COMPILE_RULE
$(BUILDDIR)/$(MODULE)/$(notdir $(1)).o: $(1) $(RUSTFMT_DEPS)
	@$(MKDIR)
	$(RUST_CC) $(RUST_FLAGS) $(RUST_MODULE_FLAGS) -c $$< -o $$@
endef

$(foreach file,$(MODULE_RUST_SRCS),$(eval $(call RUST_COMPILE_RULE,$(file))))

include make/module.mk
