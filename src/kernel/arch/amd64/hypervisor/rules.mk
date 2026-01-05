# Copyright Rustux Authors 2025
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT

LOCAL_DIR := $(GET_LOCAL_DIR)

MODULE := $(LOCAL_DIR)

MODULE_SRCS := \
	$(LOCAL_DIR)/guest.rs \
	$(LOCAL_DIR)/vcpu.rs \
	$(LOCAL_DIR)/vmexit.rs \
	$(LOCAL_DIR)/vmx.S \
	$(LOCAL_DIR)/vmx_cpu_state.rs \
	$(LOCAL_DIR)/pvclock.rs \
	$(LOCAL_DIR)/pvclock_priv.rs \

include make/module.mk