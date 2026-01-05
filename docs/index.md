# Rustux Microkernel Documentation Index

## Overview

**Rustux** is a Zircon-style microkernel written in Rust that supports multiple 64-bit architectures with **identical behavior across platforms**.

### Supported Architectures (64-bit only)
- **ARM64** (aarch64) - Status: ✅ Converted
- **AMD64** (x86_64) - Status: 🔄 In progress
- **RISC-V** (riscv64gc) - Status: ✅ Implemented

### Design Principles
- **One syscall/object model** across all architectures
- **Per-architecture kernel binaries** with stable ABI
- **Capability-based security** (Zircon-style handles and rights)
- **MIT License** for maximum ecosystem adoption

---

## Project Structure

```
/var/www/rustux.com/rustux/
├── src/kernel/
│   ├── arch/
│   │   ├── arm64/          # ARM64 support (converted)
│   │   ├── amd64/          # AMD64/x86-64 support (in progress)
│   │   └── riscv64/        # RISC-V support (implemented)
│   ├── object/             # Kernel objects
│   ├── syscalls/           # Syscall layer
│   ├── vm/                 # Virtual memory
│   └── ...
└── docs/                   # This documentation
    ├── index.md            # This file
    ├── syscall_abi_spec.md # Syscall reference
    ├── hal_traits_spec.md  # HAL interfaces
    └── phase_*.md          # Implementation phases
```

---

## Implementation Phases

| Phase | Title | Status | Description |
|-------|-------|--------|-------------|
| [Phase A](phase_a_boot_bringup.md) | Boot & Core Services | 🔄 In Progress | Repository, boot, MMU, early services |
| [Phase B](phase_b_virtual_memory.md) | Virtual Memory | ⏳ Pending | Address space model, page tables, VMO |
| [Phase C](phase_c_threads_syscalls.md) | Threads & Syscalls | ⏳ Pending | Scheduler, syscall ABI, process model |
| [Phase D](phase_d_kernel_objects.md) | Kernel Objects & IPC | ⏳ Pending | Handles, channels, events, ports |
| [Phase E](phase_e_memory_features.md) | Memory Features | ⏳ Pending | Demand paging, COW, shared memory, VDSO |
| [Phase F](phase_f_multiplatform.md) | Multiplatform Enablement | ⏳ Pending | AMD64, RISC-V ports, conformance |
| [Phase G](phase_g_userspace_sdk.md) | Userspace SDK | ⏳ Pending | LibSystem, build toolchain |
| [Phase H](phase_h_qa_testing.md) | QA & Testing | ⏳ Pending | Fuzzing, property tests, invariants |
| [Phase I](phase_i_docs_governance.md) | Docs & Governance | ⏳ Pending | Spec publication, contribution guide |

---

## Reference Specifications

- **[Syscall ABI Specification](syscall_abi_spec.md)** - Stable syscall interface and object model
- **[HAL Traits Specification](hal_traits_spec.md)** - Hardware Abstraction Layer interfaces

---

## Quick Links

### By Architecture
- [ARM64 Bring-Up Guide](phase_a_boot_bringup.md#arm64-first)
- [AMD64 Porting Guide](phase_f_multiplatform.md#amd64-port)
- [RISC-V Porting Guide](phase_f_multiplatform.md#risc-v-port)

### By Topic
- [Memory Management](phase_b_virtual_memory.md)
- [Process & Thread Model](phase_c_threads_syscalls.md)
- [IPC & Channels](phase_d_kernel_objects.md#ipc)
- [Testing Strategy](phase_h_qa_testing.md)

---

## Milestones

### M1 - Foundation Complete
- [ ] Boots on ARM64 + launches first user task
- [ ] Passes ABI conformance on AMD64 and RISC-V
- [ ] Same syscall/object semantics on all architectures

### M2 - Feature Complete
- [ ] Full IPC and capability model
- [ ] Demand paging and COW
- [ ] VDSO and userspace SDK

### M3 - Production Ready
- [ ] Security audit passed
- [ ] Comprehensive test coverage
- [ ] Documentation complete

---

## Contributing

See [Phase I - Documentation & Governance](phase_i_docs_governance.md) for contribution guidelines.

---

*Generated from `/var/www/rustux.com/rustux_kernel_architecture_checklist.md`*
