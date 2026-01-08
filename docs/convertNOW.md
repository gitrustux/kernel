# Rustux / Rustica C++ → Rust Conversion Checklist

This checklist defines what C++ code **must**, **should**, and **must not** be converted to Rust in the Rustux kernel.  
Goal: **Rust-first kernel with zero long-term C++ dependency** (except ASM + linker glue).

---

## 🔴 HIGH PRIORITY — Convert to Rust (Core Value)

### Kernel Object Model (`src/kernel/object/`)
- [ ] dispatcher.cpp
- [ ] handle.cpp
- [ ] channel_dispatcher.cpp
- [ ] process_dispatcher.cpp
- [ ] thread_dispatcher.cpp
- [ ] job_dispatcher.cpp
- [ ] resource_dispatcher.cpp
- [ ] vm_object_dispatcher.cpp
- [ ] pager_dispatcher.cpp
- [ ] exception.cpp
- [ ] event_dispatcher.cpp
- [ ] futex_context.cpp

---

### Virtual Memory Subsystem (`src/kernel/vm/`)
- [ ] vm.cpp
- [ ] vm_aspace.cpp
- [ ] vm_object.cpp
- [ ] vm_object_paged.cpp
- [ ] vm_mapping.cpp
- [ ] page.cpp
- [ ] pmm.cpp
- [ ] pmm_node.cpp
- [ ] vm_address_region.cpp
- [ ] vm_address_region_list.cpp

---

### Crypto & Entropy (`src/kernel/lib/crypto/`)
- [ ] prng.cpp
- [ ] global_prng.cpp
- [ ] entropy/*.cpp

---

## 🟡 MEDIUM PRIORITY — Convert Later

### Hypervisor / VCPU (`src/kernel/lib/hypervisor/`)
- [ ] cpu.cpp
- [ ] guest_physical_address_space.cpp
- [ ] trap_map.cpp

---

### Debugging & Diagnostics
- [ ] ktrace.cpp
- [ ] debuglog.cpp
- [ ] crashlog.cpp
- [ ] debugcommands.cpp

---

## 🟢 LOW PRIORITY — Keep Temporarily

### Platform & Firmware Glue
- [ ] platform/pc/*
- [ ] platform/generic-arm/*
- [ ] acpi.cpp
- [ ] smbios.cpp
- [ ] hpet.cpp
- [ ] interrupts.cpp

---

## Keep For Now

### Device Drivers (Short Term)
- [ ] dev/uart/*
- [ ] dev/iommu/*
- [ ] dev/interrupt/*
- [ ] dev/pci*

---

### Boot / libc Glue
- [ ] libc/cxa_atexit.cpp
- [ ] userboot.cpp
- [ ] top/main.cpp

---

## 🚫 DO NOT CONVERT

### Tests
- [ ] *_tests.cpp
- [ ] *_unittest.cpp
- [ ] header_tests/*

---

## 🚨 Hard Rules

- [ ] No new C++ files
- [ ] No STL / RTTI / exceptions
- [ ] Rust-only for new kernel code

---

## 🧭 Migration Order

- [ ] Phase 1: Object model + VM + crypto
- [ ] Phase 2: IPC, scheduler, diagnostics
- [ ] Phase 3: Hypervisor + drivers
- [ ] Phase 4: Remove remaining C++

---

## ✅ End State

- Rust ≥ 90% of kernel logic
- C only for boot + firmware glue
- Zero C++ in steady state
