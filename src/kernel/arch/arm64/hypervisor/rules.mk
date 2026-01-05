# Copyright 2025 Rustux Authors
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT

LOCAL_DIR := $(GET_LOCAL_DIR)

MODULE := $(LOCAL_DIR)

# Assembly sources remain as .S files
MODULE_SRCS := \
	$(LOCAL_DIR)/el2.S \
	$(LOCAL_DIR)/gic/el2.S \

# Rust source files
MODULE_RUST_SRCS := \
	$(LOCAL_DIR)/el2_cpu_state.rs \
	$(LOCAL_DIR)/guest.rs \
	$(LOCAL_DIR)/vcpu.rs \
	$(LOCAL_DIR)/vmexit.rs \
	$(LOCAL_DIR)/gic/gicv2.rs \
	$(LOCAL_DIR)/gic/gicv3.rs \

include make/module.mk