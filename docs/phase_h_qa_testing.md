# Phase H — Reliability, Security, & QA

**Status:** ⏳ Pending (depends on Phase G completion)

---

## Overview

This phase establishes the quality assurance, testing infrastructure, and security validation for the Rustux microkernel.

---

## H-1. Kernel Invariants

### Memory Safety Rules

| Rule | Description |
|------|-------------|
| No unsafe outside arch | `unsafe` blocks only in arch + HAL layers |
| Document unsafe | Every unsafe block has rationale comment |
| Runtime guards | Debug mode checks array bounds, null pointers |
| Slab canaries | Debug mode slab allocator red zones |

### Capability Integrity

| Rule | Description |
|------|-------------|
| No ambient authority | All operations require explicit handle |
| Unforgeable handles | Handle IDs are process-local and validated |
| Rights validation | Every syscall checks required rights |
| No pointer sharing | Pointers never cross process boundaries |

---

## H-2. Testing Infrastructure

### Test Categories

| Type | Description | Location |
|------|-------------|----------|
| Unit tests | Per-module tests | `kernel/*/tests/` |
| Integration tests | Cross-module tests | `kernel/tests/` |
| Conformance tests | ABI compatibility | `tests/conformance/` |
| Stress tests | Load testing | `tests/stress/` |
| Fuzz tests | Randomized inputs | `tests/fuzz/` |

### Conformance Test Suite

```rust
// tests/conformance/syscalls.rs
#[test]
fn conformance_process_create() {
    let root = get_root_job();
    let proc = rx_process_create(root, cstr!("test"), 0)?;
    assert_eq!(rx_handle_close(proc), OK);
}

#[test]
fn conformance_vmo_cow() {
    let vmo = rx_vmo_create(4096, VMO_COW)?;
    let clone = rx_vmo_clone(vmo)?;
    // Write to original
    rx_vmo_write(vmo, 0, b"hello")?;
    // Clone should see original data
    let mut buf = [0u8; 5];
    rx_vmo_read(clone, 0, &mut buf)?;
    assert_eq!(&buf, b"hello");
    // Write to clone
    rx_vmo_write(clone, 0, b"world")?;
    // Original unchanged
    rx_vmo_read(vmo, 0, &mut buf)?;
    assert_eq!(&buf, b"hello");
}
```

---

## H-3. Stress Testing

### Scenarios

```rust
// tests/stress/threads.rs
#[test]
fn stress_many_threads() {
    const N: usize = 1000;
    let threads: Vec<_> = (0..N)
        .map(|_| Thread::spawn(worker_fn, ptr::null_mut()).unwrap())
        .collect();
    for t in threads {
        t.join().unwrap();
    }
}

// tests/stress/ipc.rs
#[test]
fn stress_ipc_flood() {
    let (ca, cb) = Channel::create()?;
    for i in 0..10000 {
        ca.write(&i.to_le_bytes(), &[])?;
    }
}

// tests/stress/mmap.rs
#[test]
fn stress_many_mappings() {
    let proc = rx_process_create(root_job, cstr!("test"), 0)?;
    for i in 0..1000 {
        let vmo = rx_vmo_create(4096, 0)?;
        rx_vmar_map(proc, vmo, 0, 0x1000_0000 + i * 0x10000, 4096, MAP_RW)?;
    }
}
```

---

## H-4. Fuzzing

### Syscall Fuzzer

```rust
// tests/fuzz/syscall_fuzzer.rs
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut iter = data.iter().cloned();

    // Random syscall sequence
    while let Some(syscall_num) = iter.next() {
        let args: Vec<u64> = iter.by_ref().take(6).map(|x| x as u64).collect();

        let result = unsafe {
            match syscall_num {
                1 => syscall1(SYS_PROCESS_CREATE, args[0]),
                2 => syscall2(SYS_THREAD_CREATE, args[0], args[1]),
                // ... all syscalls
                _ => continue,
            }
        };

        // Should not panic
        assert!(result != 0xFFFFFFFF); // Not crashed
    }
});
```

### IPC Fuzzer

```rust
fuzz_target!(|data: &[u8]| {
    let (ca, cb) = Channel::create().unwrap();

    // Fuzz channel writes with malformed data
    let _ = ca.write(data, &[]);
    let mut buf = [0u8; 1024];
    let _ = cb.read(&mut buf, &mut vec![]);

    // Should not panic
});
```

---

## H-5. Security Testing

### Capability Escape Tests

```rust
// tests/security/capability.rs
#[test]
fn test_no_ambient_authority() {
    // Should not be able to access objects without handle
    let result = rx_vmo_write(0x12345678, 0, b"test"); // Invalid handle
    assert_eq!(result, Err(RX_ERR_BAD_HANDLE));
}

#[test]
fn test_rights_enforcement() {
    let vmo = rx_vmo_create(4096, VMO_READ)?;
    // Write should fail without WRITE right
    let result = rx_vmo_write(vmo, 0, b"test");
    assert_eq!(result, Err(RX_ERR_ACCESS_DENIED));
}

#[test]
fn test_rights_escalation_blocked() {
    let vmo = rx_vmo_create(4096, VMO_READ)?;
    // Cannot add rights via duplicate
    let result = rx_handle_duplicate(vmo, RIGHT_WRITE);
    assert_eq!(result, Err(RX_ERR_ACCESS_DENIED));
}
```

### Memory Isolation Tests

```rust
// tests/security/isolation.rs
#[test]
fn test_process_isolation() {
    let proc1 = rx_process_create(root_job, cstr!("p1"), 0)?;
    let proc2 = rx_process_create(root_job, cstr!("p2"), 0)?;

    // Create VMO in proc1, should not be accessible from proc2
    let vmo = rx_vmo_create(4096, 0)?;

    // Try to access from proc2 without transfer
    let result = rx_vmo_write_for_process(proc2, vmo, 0, b"test");
    assert_eq!(result, Err(RX_ERR_ACCESS_DENIED));
}
```

---

## H-6. Performance Benchmarks

### Benchmark Suite

```rust
// tests/bench/syscall_latency.rs
#[bench]
fn bench_syscall_null(b: &mut Bencher) {
    b.iter(|| {
        rx_clock_get(CLOCK_MONOTONIC).unwrap();
    });
}

#[bench]
fn bench_channel_roundtrip(b: &mut Bencher) {
    let (ca, cb) = Channel::create().unwrap();
    b.iter(|| {
        ca.write(b"ping", &[]).unwrap();
        let mut buf = [0u8; 4];
        cb.read(&mut buf, &mut vec![]).unwrap();
    });
}

#[bench]
fn bench_vmo_clone(b: &mut Bencher) {
    let vmo = rx_vmo_create(1024 * 1024, 0).unwrap();
    b.iter(|| {
        rx_vmo_clone(vmo).unwrap();
    });
}
```

### Baseline Targets

| Metric | ARM64 | x86-64 | RISC-V |
|--------|-------|--------|--------|
| Null syscall | < 100ns | < 100ns | < 100ns |
| Channel RTT | < 500ns | < 500ns | < 500ns |
| Page fault | < 1μs | < 1μs | < 1μs |
| Context switch | < 500ns | < 500ns | < 500ns |

---

## H-7. Debug & Diagnostics

### Crash Dump Format

```rust
pub struct CrashDump {
    pub exception: ExceptionType,
    pub registers: RegisterDump,
    pub stack_trace: Vec<usize>,
    pub fault_address: Option<usize>,
    pub fault_access: Option<AccessType>,
}

pub struct RegisterDump {
    pub pc: u64,
    pub sp: u64,
    pub gp: [u64; 32],
    pub flags: u64,
}
```

### Tracing Infrastructure

```rust
// Kernel trace points
trace!(syscall, name="rx_vmo_create", size = size);
trace!(trap, cause = cause, addr = addr);
trace!(schedule, from = from_tid, to = to_tid);
```

---

## H-8. Static Analysis

### Clippy Lints

```rust
// deny by default
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_unsafe)]

// allow specific cases
#![allow(clippy::too_many_arguments)] // Common in FFI
```

### KLEE Symbolic Execution (Optional)

```c
// Test MMU logic with symbolic addresses
void test_mmu_map() {
    symbolic_addr = klee_make_symbolic_value();
    result = mmu_map(symbolic_addr, phys, flags);
    klee_assert(result == OK || result == INVALID_ADDR);
}
```

---

## Done Criteria (Phase H)

- [ ] All kernel invariants documented and enforced
- [ ] Conformance test suite passes on all architectures
- [ ] Fuzzing finds no crashes
- [ ] Security tests demonstrate capability isolation
- [ ] Performance baselines met
- [ ] Crash dumps provide useful diagnostics

---

## Next Steps

→ **Proceed to [Phase I — Documentation & Governance](phase_i_docs_governance.md)**

---

*Phase H status updated: 2025-01-03*
