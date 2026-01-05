# 


# Rustica Server CLI Environment – Base Manifest & QEMU Validation

## Overview
This document defines the **required baseline CLI tools, services, networking utilities,
and validation steps** for Rustica server-install images.

The goal is to ensure:
- Minimal, functional, reliable server environment
- Deterministic build + test flow
- QEMU-validated image behavior
- AI-assisted reproducible implementation

Default repositories:

- **System Kernel** → http://rustux.com/kernel
- **Rustica OS** → http://rustux.com/rustica
- **Core Applications** → http://rustux.com/apps
- Repositories are editable at:

```
/etc/rustica/sources.list
```

---

# Splash screen at startup:


──────────────────────────────────────────────────────────────────────────────────────────────────────────
─████████████████───██████──██████─██████████████─██████████████─██████████─██████████████─██████████████─
─██░░░░░░░░░░░░██───██░░██──██░░██─██░░░░░░░░░░██─██░░░░░░░░░░██─██░░░░░░██─██░░░░░░░░░░██─██░░░░░░░░░░██─
─██░░████████░░██───██░░██──██░░██─██░░██████████─██████░░██████─████░░████─██░░██████████─██░░██████░░██─
─██░░██────██░░██───██░░██──██░░██─██░░██─────────────██░░██───────██░░██───██░░██─────────██░░██──██░░██─
─██░░████████░░██───██░░██──██░░██─██░░██████████─────██░░██───────██░░██───██░░██─────────██░░██████░░██─
─██░░░░░░░░░░░░██───██░░██──██░░██─██░░░░░░░░░░██─────██░░██───────██░░██───██░░██─────────██░░░░░░░░░░██─
─██░░██████░░████───██░░██──██░░██─██████████░░██─────██░░██───────██░░██───██░░██─────────██░░██████░░██─
─██░░██──██░░██─────██░░██──██░░██─────────██░░██─────██░░██───────██░░██───██░░██─────────██░░██──██░░██─
─██░░██──██░░██████─██░░██████░░██─██████████░░██─────██░░██─────████░░████─██░░██████████─██░░██──██░░██─
─██░░██──██░░░░░░██─██░░░░░░░░░░██─██░░░░░░░░░░██─────██░░██─────██░░░░░░██─██░░░░░░░░░░██─██░░██──██░░██─
─██████──██████████─██████████████─██████████████─────██████─────██████████─██████████████─██████──██████─
──────────────────────────────────────────────────────────────────────────────────────────────────────────
Operating System version: v.0.0.1
Rustux Kernel version: v.0.0.1
Visit: http://rustux.com

## 1️⃣ Server-Install Image Manifest (Checklist)

### ✔ Core Userland / Shell
- [ ] `sh` (Rust or POSIX-compatible shell)
- [ ] `busybox-compat layer` OR minimal Rust utilities
- [ ] `init` (super-minimal boot init)
- [ ] `login` (optional for headless server images)

---

### ✔ Essential CLI Utilities
- [ ] `ls`
- [ ] `cat`
- [ ] `cp`
- [ ] `mv`
- [ ] `rm`
- [ ] `mkdir`
- [ ] `touch`
- [ ] `echo`
- [ ] `ps`
- [ ] `kill`
- [ ] `dmesg`
- [ ] `uname`
- [ ] `date`

---

### ✔ Networking Tools
- [ ] `ip` (addr, link, route)
- [ ] `ping`
- [ ] `ifconfig` (optional fallback)
- [ ] `hostname`
- [ ] `nslookup` or `dig` (minimal resolver tool)
- [ ] `/etc/resolv.conf` support

---

### ✔ Firewall & Security
- [ ] `fwctl` (Rustica firewall controller — nftables-style)
- [ ] Default deny inbound
- [ ] Allow:
  - SSH (optional)
  - ICMP ping
- [ ] Persistent firewall config path:

```
/etc/rustica/firewall.rules
```

---

### ✔ Package & Repo Tools
- [ ] `pkg` (Rustica package client)
- [ ] Reads from:

```
http://rustux.com/kernel
http://rustux.com/rustica
http://rustux.com/apps
```

- [ ] Repo config file:

```
/etc/rustica/sources.list
```

- [ ] Commands:
  - [ ] `pkg update`
  - [ ] `pkg install <name>`
  - [ ] `pkg search <name>`
  - [ ] `pkg repo-list`

---

### ✔ Storage / Filesystem Tools
- [ ] `mount`
- [ ] `umount`
- [ ] `blklist`
- [ ] `mkfs-rfs` (Rustica FS if applicable)
- [ ] `/mnt/test` mount target exists

---

### ✔ System Management
- [ ] `svc` (service runner)
- [ ] Logs written to:

```
/var/log/system.log
```

- [ ] Health check command:

```
system-check
```

---

## 2️⃣ Server-Install Image Boot Validation (Checklist)

### 🟢 Must Boot To:
- [ ] Kernel loads
- [ ] Init executes
- [ ] Root FS mounts
- [ ] `/bin/sh` starts
- [ ] Network interface enumerates
- [ ] Loopback interface works
- [ ] Default firewall loads
- [ ] Package manager runs without crash

### 🟢 Runtime Validation
- [ ] `ping 127.0.0.1` works
- [ ] `ip addr` reports interface
- [ ] `pkg update` reaches repo
- [ ] `svc list` executes
- [ ] `dmesg` produces logs

---

## 3️⃣ QEMU Automated Validation Script

This script auto-boots a Rustica server image and validates core CLI tools.

> Replace `<IMAGE>.img` and `<KERNEL>.bin` with your build outputs.

```bash
#!/usr/bin/env bash
set -e

IMAGE="rustica-server.img"
KERNEL="rustica-kernel.bin"

echo "[+] Booting Rustica server image under QEMU..."

qemu-system-x86_64 \
  -kernel $KERNEL \
  -drive file=$IMAGE,format=raw \
  -serial mon:stdio \
  -m 1024 \
  -nographic \
  -no-reboot \
  -append "console=ttyS0 init=/bin/sh" \
  -netdev user,id=net0,hostfwd=tcp::2222-:22 \
  -device e1000,netdev=net0 \
| tee qemu-run.log &
PID=$!

sleep 5

echo "[+] Running automated smoke tests..."

cat <<'EOF' | nc localhost 2222

echo "[TEST] Core utilities"
ls /bin
uname -a

echo "[TEST] Networking"
ip addr
ping -c1 127.0.0.1

echo "[TEST] Package manager"
pkg repo-list
pkg update

echo "[TEST] Firewall"
fwctl status

echo "[TEST] System checks"
system-check
dmesg | tail

EOF

wait $PID || true
echo "[+] QEMU test session completed."
```

---

## 4️⃣ Expected Test Pass Criteria

- All commands run without crash
- Network stack responds
- Package manager reaches repo
- Firewall loads without error
- Kernel logs readable
- System boots repeatably

If any step fails → mark image **NOT RELEASE-READY**.

---

## 5️⃣ Future Extensions
- Automated ARM / RISC-V validation matrix
- Driver test harness for:
  - PCIe
  - Storage controllers
  - Serial + UART
- Headless SSH bootstrap mode
- Cloud-deployable server image profile

---

## ✅ Implementation Status Tracking
- [ ] Core CLI implemented
- [ ] Networking stable
- [ ] Firewall functional
- [ ] Package system wired to repos
- [ ] QEMU validation passing

---
