# Copyright 2025 The Rustux Authors
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT

LOCAL_DIR := $(GET_LOCAL_DIR)

KERNEL_INCLUDES += $(LOCAL_DIR)/include

MODULE := $(LOCAL_DIR)

MODULE_SRCS += \
    $(LOCAL_DIR)/page_tables.cpp \
    $(LOCAL_DIR)/another_file.cpp  # Add another source file

MODULE_DEPS += \
    kernel/lib/fbl \
    kernel/lib/hwreg \
    kernel/lib/additional_lib  # Add an additional dependency

include make/module.mk
