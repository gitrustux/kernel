# Rustux / Rustica C++ Stub Audit & Rust Migration Checklist

This checklist guides the conversion of remaining C++ stub files to Rust, enforcing a Rust-first kernel policy.

---

## 🎯 Objective

- Eliminate non-functional C++ stubs
- Replace them with Rust stubs where appropriate
- Remove obsolete `.cpp/.cc/.h` files
- Enforce Rust-first development
- Rename legacy branding to Rustica / Rustux

---

## 📊 Current State

### Existing Rust Modules (Already Converted)
- VM subsystem (`vm/`): `page_table.rs`, `aspace.rs`, `pmm.rs`, `layout.rs`, `boottables.rs`, `fault.rs`, etc.
- Architecture support: `arch/arm64/`, `arch/amd64/`, `arch/riscv64/`
- Core kernel: `init.rs`, `exception.rs`, `mutex.rs`, `percpu.rs`, `pmm.rs`
- Sync primitives: `sync/spin.rs`, `sync/mutex.rs`, `sync/event.rs`
- Libraries: `lib/heap.rs`, `lib/console.rs`, `lib/pci.rs`, `lib/crypto/entropy.rs`

### Remaining C++ Files (≈300+ files)

#### Bootloader (Keep - C/C++ required)
- [ ] `src/bootloader/include/*.h` (16 headers - C standard library wrappers)
- [ ] `src/bootloader/src/*.h` (8 headers - platform-specific)

---

## 🔍 Step 1 — Enumerate Remaining C++ Files

- [x] ~~Repository scanned for `.cpp`, `.cc`, `.cxx`, `.h`, `.hpp` files~~
- [x] ~~Generated comprehensive list of C++-related files~~

---

## 🧪 Step 2 — Classify Files (Stub vs Active)

### Classification Criteria

**Mark file as STUB if:**
- [ ] Contains only license headers or comments
- [ ] Contains forward declarations only
- [ ] Contains empty namespaces, classes, or functions
- [ ] Has zero or near-zero lines of executable code
- [ ] Is a placeholder with no functional logic

**Mark file as ACTIVE if:**
- [ ] Contains real logic or state machines
- [ ] Interacts with hardware
- [ ] Implements non-trivial control flow

---

## 🔁 Step 3 — Replace STUB Files with Rust

### Device Drivers (Priority: High)

#### Hardware RNG
- [ ] `dev/hw_rng/debug.cpp` → `kernel/dev/hw_rng/debug.rs`
- [ ] `dev/hw_rng/include/dev/hw_rng.h` → integrate into `kernel/dev/hw_rng.rs`

#### Intel RNG
- [ ] `dev/intel_rng/intel-rng.cpp` → `kernel/dev/intel_rng.rs`

#### Interrupt Controllers
- [ ] `dev/interrupt/arm_gic/v3/arm_gicv3.cpp` → Rust stub needed
- [ ] `dev/interrupt/arm_gic/v3/arm_gicv3_pcie.cpp` → Rust stub needed
- [ ] `dev/interrupt/arm_gic/v3/include/dev/interrupt/arm_gicv3_regs.h` → types in `kernel/arch/arm64/gicv3.rs`
- [ ] `dev/interrupt/include/dev/interrupt.h` → Already in `kernel/interrupt.rs`
- [ ] `dev/interrupt/msi.cpp` → `kernel/dev/msi.rs`

#### IOMMU
- [ ] `dev/iommu/dummy/dummy_iommu.cpp` → `kernel/dev/iommu/dummy.rs`
- [ ] `dev/iommu/dummy/include/dev/iommu/dummy.h`
- [ ] `dev/iommu/intel/*.cpp` (10 files) → `kernel/dev/iommu/intel.rs` + modules
- [ ] `dev/iommu/intel/*.h` (10 headers)
- [ ] `dev/iommu/include/dev/iommu.h` → Already in `kernel/vm/aspace.rs`

#### Platform Device Framework
- [ ] `dev/pdev/*.cpp` (4 files) → `kernel/dev/pdev.rs`
- [ ] `dev/pdev/include/pdev/*.h` (5 headers)

#### Power Management
- [ ] `dev/power/hisi/power.cpp` → `kernel/dev/power/hisi.rs`
- [ ] `dev/psci/psci.cpp` → `kernel/dev/psci.rs`
- [ ] `dev/psci/include/dev/psci.h`

#### UART Drivers
- [ ] `dev/uart/amlogic_s905/uart.cpp` → `kernel/dev/uart/amlogic_s905.rs`
- [ ] `dev/uart/mt8167/uart.cpp` → `kernel/dev/uart/mt8167.rs`
- [ ] `dev/uart/nxp-imx/uart.cpp` → `kernel/dev/uart/nxp_imx.rs`

#### Display
- [ ] `dev/udisplay/udisplay.cpp` → `kernel/dev/udisplay.rs`
- [ ] `dev/udisplay/include/dev/udisplay.h`

#### HDCP
- [ ] `dev/hdcp/amlogic_s912/hdcp.cpp` → `kernel/dev/hdcp/amlogic_s912.rs`

### Core Kernel Subsystems

#### Object/IPC System (Priority: High - Complex)
- [ ] `object/*.cpp` (40+ dispatcher files) → `kernel/object/*.rs`
- [ ] `object/include/object/*.h` (50+ headers)
- [ ] Key files to convert:
  - [ ] `dispatcher.cpp` → `kernel/object/dispatcher.rs`
  - [ ] `process_dispatcher.cpp` → `kernel/object/process.rs`
  - [ ] `thread_dispatcher.cpp` → `kernel/object/thread.rs`
  - [ ] `channel_dispatcher.cpp` → `kernel/object/channel.rs`
  - [ ] `port_dispatcher.cpp` → `kernel/object/port.rs`
  - [ ] `event_dispatcher.cpp` → `kernel/object/event.rs`
  - [ ] `job_dispatcher.cpp` → `kernel/object/job.rs`
  - [ ] `fifo_dispatcher.cpp` → `kernel/object/fifo.rs`
  - [ ] `futex_context.cpp` → `kernel/object/futex.rs`
  - [ ] `handle.cpp` → `kernel/object/handle.rs`
  - [ ] `vcpu_dispatcher.cpp` → `kernel/object/vcpu.rs`
  - [ ] `vm_object_dispatcher.cpp` → Already in `kernel/vm/`

#### VM System (Partially Converted)
- [ ] `vm/arch_vm_aspace.h` → Already in `kernel/vm/arch_vm_aspace.rs`
- [ ] `vm/bootalloc.cpp` → `kernel/vm/bootalloc.rs`
- [ ] `vm/bootreserve.cpp` → `kernel/vm/bootreserve.rs`
- [ ] `vm/include/vm/*.h` (20 headers) → Mostly in `kernel/vm/*.rs`
- [ ] `vm/kstack.cpp` → Already in `kernel/vm/stacks.rs`
- [ ] `vm/page.cpp` → Integrate into `kernel/vm/`
- [ ] `vm/page_source.cpp` → `kernel/vm/page_source.rs`
- [ ] `vm/pmm.cpp` → Already in `kernel/vm/pmm.rs` and `kernel/pmm.rs`
- [ ] `vm/pmm_arena.cpp` → `kernel/vm/pmm_arena.rs`
- [ ] `vm/pmm_node.cpp` → `kernel/vm/pmm_node.rs`
- [ ] `vm/pinned_vm_object.cpp` → `kernel/vm/pinned_vm_object.rs`
- [ ] `vm/vm.cpp` → Partially in `kernel/vm/mod.rs`
- [ ] `vm/vm_object*.cpp` (3 files) → Already in `kernel/vm/vm_object.rs`
- [ ] `vm/vm_address_region*.cpp` (2 files) → `kernel/vm/vm_address_region.rs`
- [ ] `vm/vm_aspace.cpp` → Already in `kernel/vm/aspace.rs`
- [ ] `vm/vm_mapping.cpp` → `kernel/vm/vm_mapping.rs`
- [ ] `vm/vmm.cpp` → `kernel/vm/vmm.rs`
- [ ] `vm/vm_page_list.cpp` → `kernel/vm/vm_page_list.rs`
- [ ] `vm/vm_priv.h` → Private module in `kernel/vm/`
- [ ] `vm/vm_unittest.cpp` → `kernel/vm/tests.rs`

#### Platform Support (Priority: Medium)

##### Generic ARM
- [ ] `platform/generic-arm/platform.cpp` → `kernel/platform/arm64/generic.rs`

##### PC Platform
- [ ] `platform/pc/*.cpp` (18 files) → `kernel/platform/pc/*.rs`
- [ ] `platform/pc/include/platform/pc/*.h` (10 headers)
- [ ] Key files:
  - [ ] `platform.cpp` → `kernel/platform/pc/mod.rs`
  - [ ] `acpi.cpp` → `kernel/platform/pc/acpi.rs`
  - [ ] `console.cpp` → `kernel/platform/pc/console.rs`
  - [ ] `debug.cpp` → `kernel/platform/pc/debug.rs`
  - [ ] `hpet.cpp` → `kernel/platform/pc/hpet.rs`
  - [ ] `interrupts.cpp` → `kernel/platform/pc/interrupts.rs`
  - [ ] `keyboard.cpp` → `kernel/platform/pc/keyboard.rs`
  - [ ] `memory.cpp` → `kernel/platform/pc/memory.rs`
  - [ ] `timer.cpp` → `kernel/platform/pc/timer.rs`

##### Common Platform
- [ ] `platform/debug.cpp` → `kernel/platform/debug.rs`
- [ ] `platform/init.cpp` → `kernel/platform/init.rs`
- [ ] `platform/power.cpp` → `kernel/platform/power.rs`

#### ARM64 Target/Board Support (Priority: Low)
- [ ] `target/arm64/boot-shim/*.h` (4 headers) → Keep for bootloader
- [ ] `target/arm64/board/*/boot-shim-config.h` (10 headers) → Board configs in Rust

##### PC Target
- [ ] `target/pc/empty.cpp` → Can delete (empty stub)
- [ ] `target/pc/multiboot/trampoline.h` → Keep for bootloader

#### Target Init
- [ ] `target/init.cpp` → Integrate into `kernel/init.rs`

### Libraries (lib/)

#### Counters
- [ ] `lib/counters/counters_tests.cpp` → `kernel/lib/counters/tests.rs`
- [ ] `lib/counters/counters_private.h` → Private module

#### Crypto
- [ ] `lib/crypto/entropy/collector_unittest.cpp` → `kernel/lib/crypto/entropy/tests.rs`
- [ ] `lib/crypto/entropy/quality_test.cpp` → `kernel/lib/crypto/entropy/quality.rs`
- [ ] `lib/crypto/global_prng_unittest.cpp` → `kernel/lib/crypto/prng/tests.rs`
- [ ] `lib/crypto/prng_unittest.cpp` → Already in `kernel/lib/crypto/prng.rs`
- [ ] `lib/crypto/include/lib/crypto/*.h` (11 headers) → Mostly in `kernel/lib/crypto/`

#### FBL (Fuchsia Base Library)
- [ ] `lib/fbl/*_tests.cpp` (3 test files) → `kernel/lib/fbl/tests.rs`
- [ ] `lib/fbl/include/fbl/*.h` (3 headers) → Consider replacing with standard Rust types

#### Fixed Point
- [ ] `lib/fixed_point/include/lib/fixed_point*.h` (2 headers) → Already in `kernel/lib/fixed_point.rs`

#### Heap
- [ ] `lib/heap/cmpctmalloc/include/lib/cmpctmalloc.h` → Already in `kernel/lib/heap/cmpctmalloc.rs`
- [ ] `lib/heap/include/lib/heap.h` → Already in `kernel/lib/heap.rs`

#### Hypervisor
- [ ] `lib/hypervisor/hypervisor_unittest.cpp` → `kernel/lib/hypervisor/tests.rs`
- [ ] `lib/hypervisor/include/hypervisor/*.h` (10 headers) → Partially in `kernel/lib/hypervisor/*.rs`

#### I/O
- [ ] `lib/io/include/lib/io.h` → Already in `kernel/lib/io.rs`

#### Libc (Keep - C compatibility layer)
- [ ] `lib/libc/include/*.h` (15 headers) → Keep for C compatibility
- [ ] `lib/libc/string/arch/amd64/tests.cpp` → `kernel/lib/libc/string/tests.rs`

#### Memory Management
- [ ] `lib/memory_limit/include/lib/memory_limit.h` → Already in `kernel/lib/memory_limit.rs`
- [ ] `lib/oom/include/lib/oom.h` → Already in `kernel/lib/oom.rs`

#### PCI
- [ ] `lib/pci/include/lib/pci/pio.h` → Already in `kernel/lib/pci.rs`

#### Pow2 Allocator
- [ ] `lib/pow2_range_allocator/include/lib/pow2_range_allocator.h` → Already in `kernel/lib/pow2_range_allocator.rs`

#### Topology
- [ ] `lib/topology/include/lib/system-topology.h` → Already in `kernel/lib/topology.rs`

#### Unit Test Framework
- [ ] `lib/unittest/unittest.cpp` → `kernel/lib/unittest/runner.rs`
- [ ] `lib/unittest/include/lib/unittest/*.h` (2 headers) → Already in `kernel/lib/unittest/`

#### User Copy
- [ ] `lib/user_copy/include/lib/user_copy/*.h` (2 headers) → Already in `kernel/usercopy/`

#### VDSO
- [ ] `lib/vdso/include/lib/vdso*.h` (3 headers) → Already in `kernel/lib/vdso.rs`

#### Version
- [ ] `lib/version/include/lib/version.h` → Already in `kernel/lib/version.rs`

#### Watchdog
- [ ] `lib/watchdog/include/lib/watchdog.h` → Already in `kernel/lib/watchdog.rs`

### Include Headers (Architecture Abstraction)

These headers define interfaces between architecture-independent kernel code and architecture-specific code. Most should be replaced with Rust traits.

- [ ] `include/arch/debugger.h` → `arch_traits::ArchDebugger` trait
- [ ] `include/arch/exception.h` → `arch_traits::ArchException` trait
- [ ] `include/arch.h` → `arch/mod.rs` traits
- [ ] `include/arch/mmu.h` → `arch_traits::ArchMMU` trait (already exists)
- [ ] `include/arch/mp.h` → `arch_traits::ArchMp` trait (already exists)
- [ ] `include/arch/ops.h` → `arch_traits::ArchOps` trait
- [ ] `include/arch/thread.h` → `arch_traits::ArchThread` trait
- [ ] `include/arch/user_copy.h` → `arch_traits::ArchUserCopy` trait

### Kernel Includes

#### Atomic/Lock
- [ ] `include/kernel/atomic.h` → Use `core::sync::atomic`
- [ ] `include/kernel/auto_lock.h` → Use `kernel/sync/mutex.rs`
- [ ] `include/kernel/spinlock.h` → Use `kernel/sync/spin.rs`

#### Core
- [ ] `include/kernel/align.h` → Use `kernel/align.rs`
- [ ] `include/kernel/cpu.h` → `kernel/cpu.rs`
- [ ] `include/kernel/cmdline.h` → Already in `kernel/cmdline.rs`
- [ ] `include/kernel/dpc.h` → Already in `kernel/dpc.rs`
- [ ] `include/kernel/event.h` → Use `kernel/sync/event.rs`
- [ ] `include/kernel/init.h` → Already in `kernel/init.rs`
- [ ] `include/kernel/interrupt.h` → `kernel/interrupt.rs`
- [ ] `include/kernel/lockdep.h` → Already in `kernel/lib/lockdep.rs`
- [ ] `include/kernel/mutex.h` → Use `kernel/sync/mutex.rs`
- [ ] `include/kernel/percpu.h` → Already in `kernel/percpu.rs`
- [ ] `include/kernel/sched.h` → Already in `kernel/sched/mod.rs`
- [ ] `include/kernel/thread.h` → Already in `kernel/thread/mod.rs`
- [ ] `include/kernel/thread_lock.h` → Already in `kernel/thread_lock.rs`
- [ ] `include/kernel/timer.h` → `kernel/timer.rs`
- [ ] `include/kernel/timer_slack.h` → `kernel/timer_slack.rs`
- [ ] `include/kernel/wait.h` → Use `kernel/sync/wait_queue.rs`

#### KTL (Kernel Template Library)
- [ ] `include/ktl/move.h` → Rust has move semantics by default
- [ ] `include/ktl/unique_ptr.h` → Use `Box` or `kernel/allocator.rs`

#### Misc
- [ ] `include/arm_acle.h` → Use `core::arch::asm`
- [ ] `include/asm.h` → Use `core::arch::asm`
- [ ] `include/bits.h` → Use `bitflags!` macro
- [ ] `include/debug.h` → Already in `kernel/debug.rs`
- [ ] `include/dev/*.h` (4 headers) → Already in `kernel/dev/`
- [ ] `include/err.h` → Use `rustux::types::err`
- [ ] `include/hidden.h` → Visibility attributes
- [ ] `include/lib/*.h` (3 headers) → Already in `kernel/lib/`
- [ ] `include/list.h` → Use `kernel/collections/` or `alloc::collections`
- [ ] `include/lk/*.h` (2 headers) → Legacy LK, replace
- [ ] `include/mexec.h` → `kernel/mexec.rs`
- [ ] `include/platform.h` → `kernel/platform/mod.rs`
- [ ] `include/pow2.h` → Use `kernel/lib/pow2_range_allocator.rs`
- [ ] `include/reg.h` → `kernel/reg.rs`
- [ ] `include/sys/types.h` → Use `rustux::types`
- [ ] `include/target.h` → `kernel/target.rs`
- [ ] `include/trace.h` → `kernel/trace.rs`

### Top-Level Init
- [ ] `top/init.cpp` → Integrate into `kernel/init.rs`
- [ ] `top/main.cpp` → Integrate into `kernel/main.rs`

### Architecture-Specific

#### AMD64/x86_64
- [ ] `arch/amd64/sys_x86.h` → `kernel/arch/amd64/mod.rs`

---

## 🗑️ Step 4 — Remove Obsolete C++ Files

For each Rust replacement completed:

- [ ] Delete the `.cpp/.cc/.cxx` stub file
- [ ] Delete associated header files (`.h/.hpp`) if unused
- [ ] Remove build references:
  - [ ] Makefiles
  - [ ] build.rs
  - [ ] Include paths
- [ ] Update `mod.rs` to expose new module
- [ ] Update `Cargo.toml` if required

---

## 🔤 Step 5 — Rename Legacy Branding

After Rust replacement is in place, across all modified files:

- [ ] Replace `Fuchsia` → `Rustica`
- [ ] Replace `Zircon` → `Rustux`
- [ ] Replace `fuchsia` → `rustica`
- [ ] Replace `zircon` → `rustux`
- [ ] Replace `ZX` → `RX` (error codes)
- [ ] Replace `mx` → `rx` (handle prefix)

**Note:** Preserve original copyright, update project naming only.

---

## 🧱 Step 6 — Enforce Language Policy

- [ ] No new C++ files allowed
- [ ] No expansion of existing C++ logic
- [ ] All new kernel logic must be written in Rust
- [ ] C/C++ allowed only for:
  - [ ] Boot code
  - [ ] Firmware glue
  - [ ] Architecture assembly bridges
  - [ ] libc compatibility layer

---

## ✅ Step 7 — Verification

For each conversion batch:

- [ ] Run `cargo build --target <arch>`
- [ ] Confirm build succeeds (all architectures: `aarch64`, `x86_64`, `riscv64gc`)
- [ ] Ensure no deleted symbols are referenced
- [ ] Confirm Rust percentage increases
- [ ] Confirm no new C++ stubs remain
- [ ] Run tests: `cargo test`
- [ ] Check for warnings: `cargo clippy`

---

## 🏁 End State Goals

- [ ] Rust ≥ 80% of kernel logic
- [ ] C++ reduced to transitional or zero usage
- [ ] All placeholder subsystems owned by Rust
- [ ] Clean foundation for Rust-only future development
- [ ] All branding updated to Rustux/Rustica

---

## 📋 Rust Stub Template

For files marked as **STUB**, use this template:

```rust
// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! <Subsystem name> (Rustux)
//!
//! Rust replacement for legacy C++ stub at `<old_path>`.
//!
//! # TODO
//!
//! - Implement subsystem logic
//! - Add error handling
//! - Add tests

use crate::kernel::vm::{Result, VmError};

/// Initialize the subsystem
pub fn init() -> Result {
    // TODO: implement subsystem initialization
    log_info!("Subsystem initialized");
    Ok(())
}
```

---

## 📊 Progress Tracking

### Completed Conversions
- [x] VM core (page_table, aspace, pmm, layout)
- [x] Architecture support (ARM64, AMD64, RISC-V)
- [x] Core kernel (init, exception, mutex, percpu)
- [x] Sync primitives (spin, mutex, event)
- [x] Several libraries (heap, console, pci, crypto)

### In Progress
- [ ] VM extensions (vm_object, vm_address_region)
- [ ] Object/IPC system
- [ ] Device drivers

### Not Started
- [ ] Platform code
- [ ] Hypervisor support
- [ ] Target/board configs
- [ ] Legacy header cleanup

---

## 📝 Notes

1. **Bootloader code** (under `src/bootloader/`) should remain in C/C++ as it's a separate project
2. **Libc headers** should be kept for C compatibility
3. **Assembly bridges** may need C stubs for calling convention
4. **Architecture-specific headers** should be converted to Rust traits
5. **Test files** should be converted to Rust `#[cfg(test)]` modules
6. **Objective**: Minimize C++ to only what's absolutely necessary for boot/assembly compatibility

---

*Last updated: 2025-01-08*
