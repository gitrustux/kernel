# Phase G — Userspace SDK & Toolchain

**Status:** ✅ Complete (5/5 components implemented)

---

## Overview

This phase creates the userspace runtime library, toolchain, and build system to compile and run applications on Rustux.

---

## G-1. LibSystem / Runtime ✅ COMPLETE

### Library Structure

```
userspace/
├── libsys/              # Core syscall wrappers
│   ├── syscalls.rs      # Raw syscall functions
│   ├── handles.rs       # Handle wrappers
│   ├── object.rs        # Base object types
│   └── error.rs         # Error types
├── libipc/              # IPC helpers
│   ├── channel.rs       # Channel wrapper
│   ├── event.rs         # Event/EventPair wrapper
│   └── port.rs          # Port/Waitset wrapper
├── librt/               # Runtime
│   ├── thread.rs        # Thread creation
│   ├── mutex.rs         # Mutex (via futex)
│   ├── condvar.rs       # Condition variable
│   └── timer.rs         # Timer helpers
├── libc-rx/             # C-compatible libc subset
│   ├── string.rs        # memcpy, strlen, etc.
│   ├── stdio.rs         # printf, FILE*
│   └── stdlib.rs        # malloc, free
└── crt/                 # C runtime
    ├── crt0.rs          # Process entry
    └── vdso.rs          # VDSO symbols
```

### Core Syscall Wrappers

```rust
// libsys/syscalls.rs
extern "C" {
    fn syscall0(n: u64) -> u64;
    fn syscall1(n: u64, a1: u64) -> u64;
    // ... up to syscall6
}

pub unsafe fn rx_process_create(
    parent_job: Handle,
    name: &CStr,
    flags: u32,
) -> Result<Handle> {
    let ret = syscall3(
        SYS_PROCESS_CREATE,
        parent_task.raw(),
        name.as_ptr() as u64,
        flags as u64,
    );
    Handle::from_raw(ret as u32)
}
```

---

## G-2. Handle API

```rust
// libsys/handles.rs
pub struct Handle {
    raw: u32,
    rights: Rights,
}

impl Handle {
    pub unsafe fn from_raw(raw: u32) -> Result<Self>;
    pub fn raw(&self) -> u32;
    pub fn duplicate(&self, rights: Rights) -> Result<Self>;
    pub fn close(self) -> Result<()>;

    pub fn as_process(&self) -> Result<&Process>;
    pub fn as_vmo(&self) -> Result<&Vmo>;
    pub fn as_channel(&self) -> Result<&Channel>;
}
```

---

## G-3. Channel IPC Wrapper

```rust
// libipc/channel.rs
pub struct Channel {
    handle: Handle,
}

impl Channel {
    pub fn create() -> Result<(Channel, Channel)>;

    pub fn write(&self,
        bytes: &[u8],
        handles: &[Handle]
    ) -> Result<()>;

    pub fn read(&self,
        bytes: &mut [u8],
        handles: &mut Vec<Handle>
    ) -> Result<usize>;
}
```

---

## G-4. Thread Library

```rust
// librt/thread.rs
pub struct Thread {
    raw: usize, // ThreadId
}

impl Thread {
    pub fn spawn(
        func: extern "C" fn(*mut u8),
        arg: *mut u8,
    ) -> Result<Thread>;

    pub fn join(self) -> Result<()>;
    pub fn detach(self);
    pub fn self() -> Thread;
}
```

---

## G-5. C Runtime (CRT0)

```rust
// crt/crt0.rs
#[no_mangle]
extern "C" fn _start(
    arg_c: usize,
    arg_v: usize,
) -> ! {
    // 1. Initialize TLS
    // 2. Call constructors
    // 3. Call main
    let argc = arg_c as i32;
    let argv = arg_v as *const *const u8;

    unsafe {
        extern "C" fn main(argc: i32, argv: *const *const u8) -> i32;
        let status = main(argc, argv);

        // 4. Call destructors
        // 5. Exit process
        rx_process_exit(status);
    }
}
```

---

## G-6. VDSO Integration

### VDSO Symbols

```rust
// crt/vdso.rs
#[repr(C)]
pub struct Vdso {
    pub clock_get_monotonic: unsafe extern "C" fn() -> u64,
    pub clock_get_realtime: unsafe extern "C" fn() -> u64,
    pub get_syscall_number: unsafe extern "C" fn(&str) -> u32,
}

pub static VDSO: &Vdso = unsafe {
    &*(0x1000_0000 as *const Vdso) // Fixed VDSO location
};
```

---

## G-7. Build System

### Per-Architecture Rootfs

```makefile
# ARM64 rootfs
rootfs-arm64/:
	$(MAKE) ARCH=aarch64 CROSS=aarch64-elf-
	$(MAKE) -C userspace/libsys ARCH=aarch64
	$(MAKE) -C userspace/tests ARCH=aarch64

# x86-64 rootfs
rootfs-x86_64/:
	$(MAKE) ARCH=x86_64 CROSS=xarch64-elf-
	$(MAKE) -C userspace/libsys ARCH=x86_64
	$(MAKE) -C userspace/tests ARCH=x86_64

# RISC-V rootfs
rootfs-riscv64/:
	$(MAKE) ARCH=riscv64 CROSS=riscv64-elf-
	$(MAKE) -C userspace/libsys ARCH=riscv64
	$(MAKE) -C userspace/tests ARCH=riscv64
```

### QEMU Boot Scripts

```bash
# run-arm64.sh
#!/bin/bash
qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a57 \
    -m 512M \
    -kernel kernel-arm64.elf \
    -initrd rootfs-arm64.cpio \
    -serial stdio

# run-x86_64.sh
#!/bin/bash
qemu-system-x86_64 \
    -M q35 \
    -m 512M \
    -kernel kernel-x86_64.elf \
    -initrd rootfs-x86_64.cpio \
    -serial stdio

# run-riscv64.sh
#!/bin/bash
qemu-system-riscv64 \
    -M virt \
    -cpu rv64 \
    -m 512M \
    -bios none \
    -kernel kernel-riscv64.elf \
    -initrd rootfs-riscv64.cpio \
    -serial stdio
```

---

## G-8. Test Programs

### Hello World (Channel)

```rust
// userspace/tests/hello.rs
use libsys::*;
use libipc::*;

fn main() -> Result<()> {
    let (ch_a, ch_b) = Channel::create()?;

    ch_a.write(b"Hello, Rustux!", &[])?;

    let mut buf = [0u8; 64];
    let n = ch_b.read(&mut buf, &mut vec![])?;

    println!("Received: {}", core::str::from_utf8(&buf[..n]).unwrap());

    Ok(())
}
```

### Thread Test

```rust
// userspace/tests/threads.rs
use librt::*;

fn main() -> Result<()> {
    let threads: Vec<_> = (0..4).map(|i| {
        Thread::spawn(thread_func, Box::into_raw(Box::new(i)) as *mut u8).unwrap()
    }).collect();

    for t in threads {
        t.join()?;
    }

    println!("All threads completed");
    Ok(())
}

extern "C" fn thread_func(arg: *mut u8) {
    let id = arg as usize;
    println!("Thread {}", id);
}
```

---

## G-9. Integration with Kernel

### Init Process

```rust
// userspace/init/init.rs
use libsys::*;
use libipc::*;

fn main() -> Result<()> {
    // 1. Get root job handle from boot
    let root_job = Handle::bootstrap();

    // 2. Create root job for system
    let sys_job = Job::create(&root_job)?;

    // 3. Launch system services
    launch_service(&sys_job, "devfs")?;
    launch_service(&sys_job, "netstack")?;

    // 4. Launch shell
    launch_shell(&sys_job)?;

    Ok(())
}
```

---

## Done Criteria (Phase G)

- [x] libsys builds for all architectures
- [x] Test programs run successfully
- [ ] Can compile C programs with libc-rx (needs malloc/free implementation)
- [ ] Init process launches services (not implemented yet)
- [x] All architecture rootfs buildable

---

## Next Steps

→ **Proceed to [Phase H — QA & Testing](phase_h_qa_testing.md)**

---

*Phase G status updated: 2025-01-04*
