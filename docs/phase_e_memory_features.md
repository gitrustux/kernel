# Phase E — Memory Management Features

**Status:** 🔄 In Progress (Pager interface and stats implemented)

---

## Overview

This phase implements advanced memory management features on top of the base VM system: demand paging, copy-on-write optimization, shared memory, and VDSO.

---

## E-1. Demand Paging & COW

### Pager Interface

```rust
pub trait Pager {
    /// Supply pages for a VMO range
    fn supply_pages(&self, vmo: &Vmo, offset: usize, pages: &[Frame]) -> Result<()>;

    /// Handle page fault
    fn fault(&self, vmo: &VMo, offset: usize) -> Result<Frame>;

    /// Pin pages (prevent eviction)
    fn pin(&self, vmo: &Vmo, offset: usize, len: usize) -> Result<()>;

    /// Unpin pages
    fn unpin(&self, vmo: &Vmo, offset: usize, len: usize) -> Result<()>;
}
```

### Page Fault Flow

```
User access → TLB miss → Page Table Walk → Invalid PTE → Trap
→ Page Fault Handler → VMO::fault() → Pager::supply_pages()
→ Update PTE → Resume user → Retry access
```

### Tasks

- [ ] Implement pager interface
- [ ] Lazy page allocation on first access
- [ ] COW page split on first write
- [ ] Zero page optimization
- [ ] Page eviction policy (LRU/ARC)

---

## E-2. Copy-on-Write Optimization

### COW VMO

```rust
pub struct Vmo {
    // ...
    pub parent: Option<VmoParent>,
    pub cow_pages: HashSet<usize>, // Pages that have been COW'd
}

pub struct VmoParent {
    pub vmo: Arc<Vmo>,
    pub offset: usize,
    pub is_cow: bool,
}
```

### COW Algorithm

```
1. VMO A created
2. VMO B = A.clone() → B points to A as parent, is_cow=true
3. Mapping read → accesses parent pages directly
4. Mapping write → fault → allocate new page → copy parent → update mapping
5. B.cow_pages.add(offset)
```

### Tasks

- [ ] Implement COW fault handler
- [ ] Track COW children per VMO
- [ ] Atomic COW split (no races)
- [ ] Optimize for read-only (no copy until write)

---

## E-3. Shared Memory

### Shared VMO Mapping

```rust
// VMO can be mapped into multiple processes
let vmo = Vmo::create(4096)?;

proc1.map_vmo(&vmo, 0, 0x1000, MAP_RW)?;
proc2.map_vmo(&vmo, 0, 0x2000, MAP_RW)?;

// Both see the same physical pages
```

### Use Cases

- IPC buffers (zero-copy)
- Shared configuration
- Graphics buffers
- Database shared memory

### Tasks

- [ ] Multi-process VMO mapping
- [ ] Coherence protocol (cache ops)
- [ ] Synchronization primitives (futex on shared memory)

---

## E-4. VDSO Support

### VDSO (Virtual Dynamic Shared Object)

A small shared library mapped into every process for fast syscalls.

### VDSO Contents

```rust
// VDSO provides fast implementations of:
pub fn rx_clock_get_monotonic() -> u64;      // Direct timer read
pub fn rx_clock_get_realtime() -> u64;       // Direct timer read
pub fn rx_thread_tls_base() -> usize;        // Direct register access
pub fn rx_cpu_features() -> u32;             // Cached CPU features
pub fn rx_get_syscall_number(name: &str) -> u32; // Syscall number lookup
```

### VDSO Layout

```
Process Address Space:
+------------------+ 0x100000
| User binary      |
+------------------+
| User stack       |
+------------------+
| ...              |
+------------------+
| VDSO (R-X)       | ← Fixed address or randomized
| rx_clock_get     |
| rx_get_syscall   |
+------------------+
| Kernel mappings  |
+------------------+
```

### Tasks

- [ ] Create VDSO binary (position-independent)
- [ ] Map VDSO into every process on creation
- [ ] Provide VDSO symbols via linker script
- [ ] Optimize critical paths

---

## E-5. Memory Commit Policies

### Commitment Models

| Model | Description |
|-------|-------------|
| **Eager commit** | All pages allocated on VMO create |
| **Lazy commit** | Pages allocated on first fault |
| **Overcommit** | Allow more than physical memory |
| **No overcommit** | Guarantee allocation succeeds |

### Configuration

```rust
pub struct VmoCreateFlags {
    pub commit: CommitPolicy,
    pub resizable: bool,
    pub overcommit: bool,
}

pub enum CommitPolicy {
    Eager,      // Allocate all pages immediately
    Lazy,       // Allocate on fault
    Explicit,   // Require explicit commit syscall
}
```

### Tasks

- [ ] Implement commit policies
- [ ] Memory pressure detection
- [ ] OOM handling
- [ ] Per-job commit limits

---

## E-6. Page Cache Integration

### Page Cache for File-Backed VMOs

```rust
pub struct PageCache {
    pages: LruCache<PageKey, Frame>,
    dirty: HashSet<PageKey>,
}

pub struct FileVmo {
    pub vmo: Vmo,
    pub file: FileHandle,
    pub offset: u64,
}
```

### Tasks

- [ ] Page cache for filesystem (future)
- [ ] Dirty page tracking
- [ ] Writeback daemon
- [ ] mmap(MAP_SHARED) support

---

## E-7. Memory Statistics

```rust
pub struct MemoryStats {
    pub total_bytes: usize,
    pub free_bytes: usize,
    pub wired_bytes: usize,     // Non-pageable
    pub active_bytes: usize,    // Recently used
    pub inactive_bytes: usize,  // Not recently used
    pub compressed_bytes: usize,// Compressed (future)
    pub page_ins: u64,          // Page-ins from disk
    pub page_outs: u64,         // Page-outs to disk
}
```

### Tasks

- [ ] Track memory statistics
- [ ] `rx_object_get_info` for memory stats
- [ ] Per-process memory usage
- [ ] Kernel diagnostics

---

## Implementation Summary

### Files Implemented

| File | Purpose | Status | Lines |
|------|---------|--------|-------|
| `vm/pager.rs` | Pager interface for demand paging | ✅ Complete | ~420 |
| `vm/stats.rs` | Memory statistics tracking | ✅ Complete | ~380 |

**Total:** ~800 lines of Rust code

### E-1: Demand Paging & COW - ✅ Partial Complete

**Implemented:**
- `Pager` trait with fault, supply_pages, pin, unpin methods
- `DefaultPager` implementation
- `PageFaultFlags` for fault classification
- `Frame` type for physical frame management
- `CowTracker` for COW page tracking
- Global page fault handler

**Remaining:**
- Integration with VMO page faults
- Zero page optimization
- Page eviction policy (LRU/ARC)

### E-7: Memory Statistics - ✅ Complete

**Implemented:**
- `MemoryStats` struct with comprehensive metrics
- `MemoryStatsTracker` for global statistics
- Per-process memory statistics
- Page fault/page-in/page-out tracking
- Usage percentage calculations

### E-2: Copy-on-Write Optimization - 🚧 Partial

**Implemented:**
- `CowTracker` in pager.rs

**Remaining:**
- COW page split on first write
- Track COW children per VMO
- Atomic COW split (no races)

### E-3: Shared Memory - 🚧 Pending

- Multi-process VMO mapping
- Coherence protocol (cache ops)
- Synchronization primitives (futex on shared memory)

### E-4: VDSO Support - 🚧 Pending

- Create VDSO binary (position-independent)
- Map VDSO into every process on creation
- Provide VDSO symbols via linker script
- Optimize critical paths

### E-5: Memory Commit Policies - 🚧 Pending

- Implement commit policies (Eager, Lazy, Explicit)
- Memory pressure detection
- OOM handling
- Per-job commit limits

### E-6: Page Cache Integration - 🚧 Deferred

This is deferred until filesystem implementation.

---

## Done Criteria (Phase E)

- [ ] Demand paging works for anonymous VMOs
- [ ] COW clone verified with tests
- [ ] Shared memory works across processes
- [ ] VDSO mapped and functional
- [ ] Memory pressure handling tested

---

## Next Steps

→ **Proceed to [Phase F — Multiplatform Enablement](phase_f_multiplatform.md)**

---

*Phase E status updated: 2025-01-03*
