# Copyright 2025 The Rustux Authors
# Copyright (c) 2008-2015 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT

LOCAL_DIR := $(GET_LOCAL_DIR)

MODULE := $(LOCAL_DIR)

BOOT_HEADER_SIZE ?= 0x50
KERNEL_LOAD_OFFSET ?= 0x00100000 # 1MB
KERNEL_BASE ?= 0xffffffff80100000 # has KERNEL_LOAD_OFFSET baked into it
KERNEL_SIZE ?= 0x40000000 # 1GB
KERNEL_ASPACE_BASE ?= 0xffffff8000000000UL # -512GB
KERNEL_ASPACE_SIZE ?= 0x0000008000000000UL
USER_ASPACE_BASE   ?= 0x0000000001000000UL # 16MB
USER_ASPACE_SIZE   ?= 0x00007ffffefff000UL

LOCAL_BUILDDIR := $(call TOBUILDDIR,$(LOCAL_DIR))

KERNEL_DEFINES += \
	ARCH_$(ARCH)=1 \
	KERNEL_BASE=$(KERNEL_BASE) \
	KERNEL_SIZE=$(KERNEL_SIZE) \
	KERNEL_LOAD_OFFSET=$(KERNEL_LOAD_OFFSET)

GLOBAL_DEFINES += \
	KERNEL_ASPACE_BASE=$(KERNEL_ASPACE_BASE) \
	KERNEL_ASPACE_SIZE=$(KERNEL_ASPACE_SIZE) \
	USER_ASPACE_BASE=$(USER_ASPACE_BASE) \
	USER_ASPACE_SIZE=$(USER_ASPACE_SIZE)

# Assembly source files (kept as .S for low-level code)
# Note: asm.S, ops.S, uspace_entry.S have been translated to Rust
MODULE_SRCS += \
	$(LOCAL_DIR)/acpi.S \
	$(LOCAL_DIR)/exceptions.S \
	$(LOCAL_DIR)/gdt.S \
	$(LOCAL_DIR)/mexec.S \
	$(LOCAL_DIR)/start.S \
	$(LOCAL_DIR)/syscall.S \
	$(LOCAL_DIR)/user_copy.S

# Rust source files
MODULE_RUST_SRCS += \
	$(LOCAL_DIR)/aal.rs \
	$(LOCAL_DIR)/arch.rs \
	$(LOCAL_DIR)/asm.rs \
	$(LOCAL_DIR)/cache.rs \
	$(LOCAL_DIR)/debugger.rs \
	$(LOCAL_DIR)/faults.rs \
	$(LOCAL_DIR)/interrupts.rs \
	$(LOCAL_DIR)/ops.rs \
	$(LOCAL_DIR)/smp.rs \
	$(LOCAL_DIR)/syscall.rs \
	$(LOCAL_DIR)/timer.rs \
	$(LOCAL_DIR)/uspace_entry.rs

MODULE_DEPS += \
	kernel/arch/amd64/page_tables \
	kernel/dev/iommu/dummy \
	kernel/lib/bitmap \
	kernel/lib/crashlog \
	kernel/lib/code_patching \
	kernel/lib/fbl \
	kernel/object

include $(LOCAL_DIR)/toolchain.mk

MODULE_SRCS += \
	$(LOCAL_DIR)/bootstrap16.cpp \
	$(LOCAL_DIR)/start16.S

# default to 16 cpu max support
SMP_MAX_CPUS ?= 16
KERNEL_DEFINES += \
	SMP_MAX_CPUS=$(SMP_MAX_CPUS)

# set the default toolchain to x86 elf and set a #define
ifndef TOOLCHAIN_PREFIX
TOOLCHAIN_PREFIX := $(ARCH_x86_64_TOOLCHAIN_PREFIX)
endif

# Rust compiler configuration for x86_64
RUST_ARCH_FLAGS := --target=x86_64-unknown-none
RUST_OPT_LEVEL ?= 2
RUST_EDITION ?= 2021

RUST_FLAGS += \
	$(RUST_ARCH_FLAGS) \
	-C opt-level=$(RUST_OPT_LEVEL) \
	--edition=$(RUST_EDITION) \
	-C panic=abort \
	-C codegen-units=1

# Rust-specific build rules
define RUST_COMPILE_RULE
$(BUILDDIR)/$(MODULE)/$(notdir $(1)).o: $(1) $(RUSTFMT_DEPS)
	@$(MKDIR)
	$(RUST_CC) $(RUST_FLAGS) $(RUST_MODULE_FLAGS) -c $$< -o $$@
endef

$(foreach file,$(MODULE_RUST_SRCS),$(eval $(call RUST_COMPILE_RULE,$(file))))

# disable SSP if the compiler supports it
GLOBAL_CFLAGS += $(call cc-option,$(CC),-fno-stack-protector,)

# set the default architecture
GLOBAL_COMPILEFLAGS += -march=x86-64 -mcx16

CLANG_ARCH := x86_64
ifeq ($(call TOBOOL,$(USE_CLANG)),true)
GLOBAL_LDFLAGS += -m elf_x86_64
GLOBAL_MODULE_LDFLAGS += -m elf_x86_64
endif
GLOBAL_LDFLAGS += -z max-page-size=4096
ifeq ($(call TOBOOL,$(USE_CLANG)),false)
KERNEL_COMPILEFLAGS += -falign-jumps=1 -falign-loops=1 -falign-functions=4
GLOBAL_COMPILEFLAGS += -malign-data=abi
endif

# hard disable floating point in the kernel
KERNEL_COMPILEFLAGS += -msoft-float -mno-mmx -mno-sse -mno-sse2 -mno-3dnow -mno-avx -mno-avx2
ifeq ($(call TOBOOL,$(USE_CLANG)),false)
KERNEL_COMPILEFLAGS += -mno-80387 -mno-fp-ret-in-387
endif

KERNEL_COMPILEFLAGS += -fPIE -include kernel/include/hidden.h
KERNEL_COMPILEFLAGS += -mno-red-zone

ifeq ($(call TOBOOL,$(USE_CLANG)),true)
KERNEL_COMPILEFLAGS += -mcmodel=kernel
endif

ifeq ($(call TOBOOL,$(USE_CLANG)),false)
KERNEL_COMPILEFLAGS += -mskip-rax-setup
endif

ifeq ($(call TOBOOL,$(ENABLE_NEW_BOOTDATA)),true)
MODULE_DEFINES += ENABLE_NEW_BOOTDATA=1
endif

include make/module.mk
