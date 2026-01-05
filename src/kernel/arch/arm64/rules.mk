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
	$(LOCAL_DIR)/interrupts.rs \
	$(LOCAL_DIR)/mmu.rs \
	$(LOCAL_DIR)/mp.rs \
	$(LOCAL_DIR)/periphmap.rs \
	$(LOCAL_DIR)/registers.rs \
	$(LOCAL_DIR)/spinlock.rs \
	$(LOCAL_DIR)/sysreg.rs \
	$(LOCAL_DIR)/thread.rs \
	$(LOCAL_DIR)/timer.rs \
	$(LOCAL_DIR)/user_copy_c.rs

# Assembly source files (unchanged)
MODULE_SRCS += \
	$(LOCAL_DIR)/asm.S \
	$(LOCAL_DIR)/cache-ops.S \
	$(LOCAL_DIR)/exceptions.S \
	$(LOCAL_DIR)/mexec.S \
	$(LOCAL_DIR)/smccc.S \
	$(LOCAL_DIR)/start.S \
	$(LOCAL_DIR)/user_copy.S \
	$(LOCAL_DIR)/uspace_entry.S

MODULE_DEPS += \
	kernel/dev/iommu/dummy \
	kernel/lib/bitmap \
	kernel/lib/crashlog \
	kernel/object \

KERNEL_DEFINES += \
	ARM_ISA_ARMV8=1 \
	ARM_ISA_ARMV8A=1

SMP_MAX_CPUS ?= 16

SMP_CPU_MAX_CLUSTERS ?= 2
SMP_CPU_MAX_CLUSTER_CPUS ?= $(SMP_MAX_CPUS)

KERNEL_DEFINES += \
	SMP_MAX_CPUS=$(SMP_MAX_CPUS) \
	SMP_CPU_MAX_CLUSTERS=$(SMP_CPU_MAX_CLUSTERS) \
	SMP_CPU_MAX_CLUSTER_CPUS=$(SMP_CPU_MAX_CLUSTER_CPUS) \

KERNEL_ASPACE_BASE ?= 0xffff000000000000
KERNEL_ASPACE_SIZE ?= 0x0001000000000000
USER_ASPACE_BASE   ?= 0x0000000001000000
USER_ASPACE_SIZE   ?= 0x0000fffffe000000

GLOBAL_DEFINES += \
	KERNEL_ASPACE_BASE=$(KERNEL_ASPACE_BASE) \
	KERNEL_ASPACE_SIZE=$(KERNEL_ASPACE_SIZE) \
	USER_ASPACE_BASE=$(USER_ASPACE_BASE) \
	USER_ASPACE_SIZE=$(USER_ASPACE_SIZE)

# kernel is linked to run at the arbitrary address of -4GB
# peripherals will be mapped just below this mark
KERNEL_BASE := 0xffffffff00000000
BOOT_HEADER_SIZE ?= 0x50

KERNEL_DEFINES += \
	KERNEL_BASE=$(KERNEL_BASE) \

# try to find the toolchain
include $(LOCAL_DIR)/toolchain.mk
TOOLCHAIN_PREFIX := $(ARCH_$(ARCH)_TOOLCHAIN_PREFIX)

# Setup rust compiler flags
RUST_ARCH_FLAGS := --target=aarch64-unknown-none
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

# generate code for the fairly generic cortex-a53
ARCH_COMPILEFLAGS += -mcpu=cortex-a53

CLANG_ARCH := aarch64
ifeq ($(call TOBOOL,$(USE_CLANG)),true)
GLOBAL_LDFLAGS += -m aarch64elf
GLOBAL_MODULE_LDFLAGS += -m aarch64elf
endif
GLOBAL_LDFLAGS += -z max-page-size=4096

# The linker writes instructions to work around a CPU bug.
GLOBAL_LDFLAGS += --fix-cortex-a53-843419

# kernel hard disables floating point
KERNEL_COMPILEFLAGS += -mgeneral-regs-only

# See engine.mk.
KEEP_FRAME_POINTER_COMPILEFLAGS += -mno-omit-leaf-frame-pointer

KERNEL_COMPILEFLAGS += -fPIE -include kernel/include/hidden.h

# Clang needs -mcmodel=kernel to tell it to use the right safe-stack ABI for
# the kernel.
ifeq ($(call TOBOOL,$(USE_CLANG)),true)
KERNEL_COMPILEFLAGS += -mcmodel=kernel
endif

# x18 is reserved in the Rustux userland ABI so it can be used
# for things like -fsanitize=shadow-call-stack.  In the kernel,
# it's reserved so we can use it to point at the per-CPU structure.
ARCH_COMPILEFLAGS += -ffixed-x18

# Rust-specific build rules
define RUST_COMPILE_RULE
$(BUILDDIR)/$(MODULE)/$(notdir $(1)).o: $(1) $(RUSTFMT_DEPS)
	@$(MKDIR)
	$(RUST_CC) $(RUST_FLAGS) $(RUST_MODULE_FLAGS) -c $$< -o $$@
endef

$(foreach file,$(MODULE_RUST_SRCS),$(eval $(call RUST_COMPILE_RULE,$(file))))

include make/module.mk