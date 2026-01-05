# Rustux Kernel Testing Guide via QEMU

---

## Overview

This guide provides instructions for testing **Rustux kernels** across multiple architectures using QEMU. Both **headless (CLI)** and **graphical** testing modes are included. All tests should ensure that the kernel boots, system calls respond correctly, and cross-architecture behavior is consistent.

---

## Table of Contents

1. [Pre-requisites & Setup](#1-pre-requisites--setup)
2. [Rust Toolchain Setup Checklist](#2-rust-toolchain-setup-checklist)
3. [QEMU Installation & Verification](#3-qemu-installation--verification)
4. [Building the Kernel](#4-building-the-kernel)
5. [Common QEMU CLI Flags](#5-common-cli-flags)
6. [Testing by Architecture](#6-testing-by-architecture)
7. [Automated Testing Checklist](#7-automated-testing-checklist)
8. [Optional Networking Tests](#8-optional-networking-tests)

---

## 1. Pre-requisites & Setup

### Required Components

- **QEMU** installed for all target architectures
- **Rust toolchain** with cross-compilation targets
- **Kernel images** compiled for each architecture
- **Disk images** or initramfs (optional for initial boot tests)

### System Requirements

- x86_64 Linux host (Ubuntu 24.04 recommended)
- Minimum 4GB RAM
- 10GB free disk space
- Internet connection for initial package installation

---

## 2. Rust Toolchain Setup Checklist

### Step 1: Install Rust via rustup

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

### Step 2: Source Rust Environment

```bash
source "$HOME/.cargo/env"
```

### Step 3: Verify Installation

```bash
rustc --version
cargo --version
```

Expected output (versions may vary):
```
rustc 1.92.0 (ded5c06cf 2025-12-08)
cargo 1.92.0 (344c4567c 2025-10-21)
```

### Step 4: Install Cross-Compilation Targets

```bash
rustup target add aarch64-unknown-none x86_64-unknown-none riscv64gc-unknown-none-elf
```

### Step 5: Verify Targets

```bash
rustup target list | grep -E "(aarch64-unknown-none|x86_64-unknown-none|riscv64gc-unknown-none-elf)"
```

Expected output:
```
aarch64-unknown-none (installed)
x86_64-unknown-none (installed)
riscv64gc-unknown-none-elf (installed)
```

### Rust Toolchain Summary

| Target | Architecture | Purpose |
|--------|--------------|---------|
| `aarch64-unknown-none` | ARM64 | Bare-metal kernel |
| `x86_64-unknown-none` | AMD64 | Bare-metal kernel |
| `riscv64gc-unknown-none-elf` | RISC-V 64-bit | Bare-metal kernel |

---

## 3. QEMU Installation & Verification

### Step 1: Install QEMU

```bash
apt update && apt install -y qemu-system-x86 qemu-system-arm qemu-system-misc qemu-utils
```

### Step 2: Verify Installation

```bash
qemu-system-x86_64 --version
qemu-system-aarch64 --version
qemu-system-riscv64 --version
```

Expected output (QEMU 8.2.2 or later):
```
QEMU emulator version 8.2.2 (Debian 1:8.2.2+ds-0ubuntu1.11)
Copyright (c) 2003-2023 Fabrice Bellard and the QEMU Project developers
```

### QEMU Emulators Summary

| Architecture | Emulator Command | Status |
|--------------|-----------------|--------|
| **AMD64/x86-64** | `qemu-system-x86_64` | ✅ Installed |
| **ARM64** | `qemu-system-aarch64` | ✅ Installed |
| **RISC-V 64-bit** | `qemu-system-riscv64` | ✅ Installed |

### Additional Components

- **qemu-utils** - Disk image management tools
- **qemu-system-gui** - Graphical display support
- **qemu-system-misc** - Additional architecture support
- **ovmf** - UEFI firmware for x86-64
- **qemu-efi-aarch64** - UEFI firmware for ARM64

---

## 4. Building the Kernel

### Build Commands (Per Architecture)

```bash
# AMD64/x86-64
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --target x86_64-unknown-none --release

# ARM64
cargo build --target aarch64-unknown-none --release

# RISC-V 64-bit
cargo build --target riscv64gc-unknown-none-elf --release
```

### Expected Kernel Output Files

| Architecture | Binary Name | Location |
|--------------|-------------|----------|
| AMD64 | `rustux-amd64.bin` | `target/x86_64-unknown-none/release/` |
| ARM64 | `rustux-arm64.bin` | `target/aarch64-unknown-none/release/` |
| RISC-V | `rustux-riscv64.bin` | `target/riscv64gc-unknown-none-elf/release/` |

---

## 5. Common QEMU CLI Flags

| Flag | Description |
|------|-------------|
| `-nographic` | Disable graphical display (serial console only) |
| `-serial mon:stdio` | Redirect serial console to standard input/output |
| `-serial stdio` | Simple serial output to terminal |
| `-append "args"` | Kernel boot arguments |
| `-hda <disk>` | Assign disk image to first IDE controller |
| `-drive file=<img>,if=virtio` | Use virtio block device |
| `-m <size>` | Set RAM size (e.g., `512M`, `2G`) |
| `-cpu <type>` | Specify CPU model |
| `-smp <cores>` | Number of CPU cores |
| `-machine <type>` | Machine type (e.g., `virt` for ARM64/RISC-V) |
| `-bios none` | Skip BIOS (for direct kernel boot) |
| `-kernel <bin>` | Load kernel binary |

---

## 6. Testing by Architecture

### Quick Smoke Tests (Minimal Boot)

These commands test basic kernel boot without disk I/O:

#### AMD64/x86-64

```bash
qemu-system-x86_64 \
  -kernel target/x86_64-unknown-none/release/rustux-amd64.bin \
  -m 512M \
  -serial stdio \
  -nographic
```

#### ARM64 (AArch64)

```bash
qemu-system-aarch64 \
  -M virt \
  -cpu cortex-a57 \
  -kernel target/aarch64-unknown-none/release/rustux-arm64.bin \
  -m 512M \
  -serial stdio \
  -nographic \
  -bios none
```

#### RISC-V 64-bit

```bash
qemu-system-riscv64 \
  -M virt \
  -kernel target/riscv64gc-unknown-none-elf/release/rustux-riscv64.bin \
  -m 512M \
  -serial stdio \
  -nographic \
  -bios none
```

### Full Tests with Disk I/O

#### AMD64/x86-64

**Headless mode (CLI only):**
```bash
qemu-system-x86_64 \
  -kernel rustux-amd64.bin \
  -hda rootfs-amd64.img \
  -append "console=ttyS0 root=/dev/sda" \
  -nographic \
  -serial mon:stdio \
  -m 2G \
  -smp 2
```

**Graphical mode:**
```bash
qemu-system-x86_64 \
  -kernel rustux-amd64.bin \
  -hda rootfs-amd64.img \
  -append "root=/dev/sda" \
  -m 2G \
  -smp 2
```

#### ARM64 (AArch64)

**Headless mode:**
```bash
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a57 \
  -m 2G \
  -smp 2 \
  -kernel rustux-arm64.bin \
  -append "console=ttyAMA0 root=/dev/vda" \
  -nographic \
  -serial mon:stdio \
  -drive file=rootfs-arm64.img,if=virtio,format=raw
```

**Graphical mode:**
```bash
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a57 \
  -m 2G \
  -smp 2 \
  -kernel rustux-arm64.bin \
  -append "root=/dev/vda" \
  -drive file=rootfs-arm64.img,if=virtio,format=raw
```

#### RISC-V 64-bit

**Headless mode:**
```bash
qemu-system-riscv64 \
  -machine virt \
  -nographic \
  -kernel rustux-riscv64.bin \
  -append "console=ttyS0 root=/dev/vda" \
  -m 2G \
  -smp 2 \
  -drive file=rootfs-riscv64.img,if=virtio,format=raw
```

**Graphical mode:**
```bash
qemu-system-riscv64 \
  -machine virt \
  -kernel rustux-riscv64.bin \
  -append "root=/dev/vda" \
  -m 2G \
  -smp 2 \
  -drive file=rootfs-riscv64.img,if=virtio,format=raw
```

---

## 7. Automated Testing Checklist

Use this checklist when running automated kernel tests:

- [ ] **Boot Test**: Kernel boots successfully within 60 seconds
- [ ] **Architecture Detection**: Proper architecture and device type detected
- [ ] **Console Output**: Serial console displays boot messages without errors
- [ ] **Memory Management**: Basic memory allocation works
- [ ] **System Calls**: Basic system calls (process creation, IPC) function
- [ ] **File System**: File creation, read/write operations succeed
- [ ] **Cross-Arch Consistency**: Behavior is consistent across ARM64, AMD64, RISC-V
- [ ] **Error Handling**: Graceful handling of error conditions
- [ ] **Logging**: Test results logged per architecture
- [ ] **Failure Reports**: Failures include exact console output snippets

---

## 8. Optional Networking Tests

### Enable User-Mode Networking

Add these flags to any QEMU command:

```bash
-netdev user,id=net0 -device virtio-net-pci,netdev=net0
```

### Full Example (AMD64 with networking)

```bash
qemu-system-x86_64 \
  -kernel rustux-amd64.bin \
  -hda rootfs-amd64.img \
  -append "console=ttyS0 root=/dev/sda" \
  -nographic \
  -serial mon:stdio \
  -m 2G \
  -smp 2 \
  -netdev user,id=net0 -device virtio-net-pci,netdev=net0
```

### Network Tests to Run

- [ ] Ping local gateway (10.0.2.2)
- [ ] DNS lookup (e.g., `example.com`)
- [ ] TCP socket creation and connection
- [ ] UDP packet transmission
- [ ] Network interface detection

---

## Troubleshooting

### Kernel Won't Boot

1. **Check binary format**: Ensure kernel is in ELF format that QEMU supports
2. **Verify architecture**: Match QEMU architecture to compiled binary
3. **Increase memory**: Try `-m 1G` or `-m 2G`
4. **Enable debug**: Add `-d int,cpu_reset` for QEMU debug output

### No Serial Output

1. **Verify serial flag**: Use `-serial stdio` or `-serial mon:stdio`
2. **Check console argument**: Match kernel console device (ttyS0, ttyAMA0, etc.)
3. **Enable early output**: Ensure kernel writes to serial port early in boot

### Cross-Architecture Inconsistencies

1. **Endianness**: Check that all architectures use correct byte ordering
2. **Memory alignment**: Verify data structures are properly aligned
3. **Atomic operations**: Ensure atomic ops work on all architectures
4. **Page sizes**: ARM64/RISC-V may use different page sizes than x86_64

---

## Notes

- Adjust memory (`-m`) and CPU cores (`-smp`) per test requirements
- For CI/CD automation, use the headless (nographic) mode
- Ensure all disk images are pre-populated with minimal OS or initramfs
- These instructions can be scripted for automated testing
- The `-serial stdio` flag is recommended for initial kernel debugging

---

## Quick Reference Card

```bash
# Quick boot tests (no disk, 512MB RAM)
AMD64:   qemu-system-x86_64 -kernel rustux-amd64.bin -m 512M -serial stdio -nographic
ARM64:   qemu-system-aarch64 -M virt -cpu cortex-a57 -kernel rustux-arm64.bin -m 512M -serial stdio -nographic -bios none
RISC-V:  qemu-system-riscv64 -M virt -kernel rustux-riscv64.bin -m 512M -serial stdio -nographic -bios none
```

---

*Document version: 1.0*
*Last updated: 2025-01-04*
