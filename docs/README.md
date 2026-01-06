# Rustux Microkernel - Feature Documentation

## Overview

Rustux is a **microkernel** operating system written in Rust, designed for embedded systems, virtualization, and security-focused applications. It follows a capability-based security model with minimal trusted computing base (TCB).

### Design Principles

- **Microkernel Architecture**: Only essential services run in kernel mode
- **Capability-Based Security**: All kernel objects accessed via capabilities
- **Memory Safety**: Leverages Rust's type system for memory safety
- **no_std**: Bare metal kernel without standard library dependencies
- **Multi-Architecture**: Support for x86-64, ARM64, and RISC-V

---

## Core Features

### 1. Memory Management

#### 1.1 Physical Memory Manager (PMM)

**Location**: `src/kernel/pmm.rs`

The Physical Memory Manager provides efficient allocation and deallocation of physical memory pages using a bitmap-based arena allocator.

**Key Features**:
- **Bitmap-based Allocation**: Tracks free/used pages with bitmap
- **Arena Management**: Multiple memory arenas for different memory regions
- **Atomic Operations**: Thread-safe allocation with atomic CAS operations
- **Contiguous Allocation**: Allocate physically contiguous pages with alignment support

**API**:
```rust
// Allocate a single page
pmm::alloc_page() -> Result<PAddr>

// Allocate contiguous pages
pmm::pmm_alloc_contiguous(count: usize, flags: u32, align_log2: u8) -> Result<PAddr>

// Free a single page
pmm::free_page(paddr: PAddr)

// Free contiguous pages
pmm::pmm_free_contiguous(paddr: PAddr, count: usize)
```

**Status**: ✅ **Implemented** - Production Ready

---

#### 1.2 Virtual Memory Manager (VMM)

**Location**: `src/kernel/vm/`

The Virtual Memory Manager provides virtual address space management with demand paging, copy-on-write, and memory mapping capabilities.

##### 1.2.1 Page Tables

**Location**: `src/kernel/vm/page_table.rs`

Provides unified page table management across architectures with trait-based abstraction.

**Key Features**:
- **Multi-Architecture Support**: x86-64 (MMU & EPT), ARM64, RISC-V
- **Large Page Support**: 4KB, 2MB, and 1GB page sizes
- **Page Splitting**: Dynamically split large pages into smaller pages
- **Page Table Traits**: 11 standardized methods for page table operations

**Page Table Methods**:
```rust
trait PageTableOps {
    // Create page table
    fn new(arch: PageTableArch) -> Result<Self>;

    // Map pages
    fn map(&mut self, vaddr: VAddr, paddr: PAddr, flags: PageTableFlags) -> Result;
    fn map_range(&mut self, start: VAddr, end: VAddr, phys_start: PAddr, flags: PageTableFlags) -> Result;

    // Unmap pages
    fn unmap(&mut self, vaddr: VAddr) -> Result;
    fn unmap_range(&mut self, start: VAddr, end: VAddr) -> Result;

    // Query mappings
    fn query(&mut self, vaddr: VAddr) -> Result<PageTableEntry>;
    fn translate(&mut self, vaddr: VAddr) -> Result<PAddr>;

    // Page protection
    fn protect(&mut self, vaddr: VAddr, flags: PageTableFlags) -> Result;

    // Large page operations
    fn split_large_page(&mut self, level: PageTableLevel, vaddr: VAddr, pte: *mut PtEntry) -> Result;

    // Physical address
    fn phys(&self) -> PAddr;
}
```

**Status**: ✅ **Implemented** - Production Ready

---

##### 1.2.2 Demand Paging

**Location**: `src/kernel/vm/pager.rs`, `src/kernel/vm/fault.rs`

Implements lazy allocation of pages on first access, reducing memory usage and improving startup time.

**Key Features**:
- **Lazy Allocation**: Pages allocated on first access
- **Zero Page Optimization**: Shared zero-filled page for read-only accesses
- **Page Fault Handling**: Seamless handling of page faults
- **Page Pinning**: Prevent critical pages from being evicted

**API**:
```rust
// Pager trait for handling page faults
trait Pager {
    fn fault(&self, vmo_id: u64, offset: usize, flags: PageFaultFlags) -> Result<Frame>;
    fn supply_pages(&self, vmo_id: u64, offset: usize, len: usize) -> Result;
    fn pin(&self, vmo_id: u64, offset: usize, len: usize) -> Result;
    fn unpin(&self, vmo_id: u64, offset: usize, len: usize) -> Result;
}

// Handle page fault
handle_page_fault(vmo_id: u64, offset: usize, flags: PageFaultFlags) -> Result<Frame>
```

**Status**: ✅ **Implemented** - Production Ready

---

##### 1.2.3 Page Eviction Policies

**Location**: `src/kernel/vm/pager.rs`

Advanced page replacement algorithms for managing limited physical memory.

**Supported Policies**:
1. **LRU (Least Recently Used)**: Evicts pages that haven't been accessed for the longest time
2. **ARC (Adaptive Replacement Cache)**: Balances between recency and frequency
3. **Clock Algorithm**: Approximation of LRU with lower overhead

**API**:
```rust
pub enum EvictionPolicy {
    None = 0,
    LRU = 1,
    ARC = 2,
    Clock = 3,
}

// Track page accesses
tracker.track_page(vmo_id: u64, offset: usize, paddr: PAddr);
tracker.record_access(vmo_id: u64, offset: usize);
tracker.record_dirty(vmo_id: u64, offset: usize);

// Find eviction candidate
candidate = tracker.find_eviction_candidate() -> Option<(u64, usize, PAddr)>
```

**Status**: ✅ **Implemented** - Production Ready

---

##### 1.2.4 Copy-on-Write (COW)

**Location**: `src/kernel/vm/fault.rs`, `src/kernel/vm/pager.rs`

Efficient memory sharing with deferred copying on write access.

**Key Features**:
- **COW Page Splitting**: Automatically splits shared pages on write
- **COW Tracking**: Tracks which pages are COW'd
- **Zero Copy**: Shares pages until modification is needed
- **COW Fault Handler**: Handles write faults on COW pages

**API**:
```rust
// COW Tracker
pub struct CowTracker {
    cow_pages: BTreeSet<usize>,
    pinned_pages: BTreeSet<usize>,
}

impl CowTracker {
    pub fn is_cow(&self, offset: usize) -> bool;
    pub fn mark_cow(&mut self, offset: usize);
    pub fn pin(&mut self, offset: usize);
    pub fn unpin(&mut self, offset: usize);
}

// Check if fault is COW
is_cow_fault(info: PageFaultInfo) -> bool

// Handle COW allocation
try_cow_allocation(addr: VAddr, aspace: &Arc<AddressSpace>, info: PageFaultInfo) -> Result
```

**Status**: ✅ **Implemented** - Production Ready

---

##### 1.2.5 Memory Debugging

**Location**: `src/kernel/vm/debug.rs`

Tools for debugging virtual memory configurations.

**Features**:
- **VA→PA Translation**: Walk page tables to translate virtual to physical addresses
- **Page Table Dumps**: Dump entire page table hierarchy
- **Mapping Inspection**: Inspect page table entries and flags

**API**:
```rust
// Translate virtual address to physical
vm_translate_virt_to_phys(vaddr: VAddr) -> Result<PAddr>

// Dump page tables
vm_dump_page_tables(vaddr: VAddr) -> Result
vm_dump_page_table_recursive(paddr: PAddr, level: u8, vaddr: VAddr) -> Result
```

**Status**: ✅ **Implemented** - Production Ready

---

#### 1.3 Virtual Memory Objects (VMO)

**Location**: `src/kernel/object/vmo.rs`

VMOs represent contiguous regions of virtual memory that can be mapped into address spaces.

**Key Features**:
- **Page-Based Memory**: Memory managed in 4KB page chunks
- **COW Clones**: Efficient copy-on-write for memory sharing
- **Resizable VMOs**: Dynamic resizing with RESIZABLE flag
- **Cache Policy Control**: Uncached, write-combining, write-through

**API**:
```rust
// Create VMO
Vmo::create(size: usize, flags: VmoFlags) -> Result<Vmo>

// Read/Write
vmo.read(offset: usize, buf: &mut [u8]) -> Result<usize>;
vmo.write(offset: usize, buf: &[u8]) -> Result<usize>;

// Clone (COW)
vmo.clone(offset: usize, size: usize) -> Result<Vmo>

// Resize (if RESIZABLE flag set)
vmo.resize(new_size: usize) -> Result

// Cache policy
vmo.set_cache_policy(policy: CachePolicy)
vmo.cache_policy() -> CachePolicy
```

**Status**: ✅ **Implemented** - Production Ready

---

#### 1.4 Shared Memory

**Location**: `src/kernel/object/vmo.rs`

Track VMO mappings across multiple address spaces for shared memory support.

**Key Features**:
- **Mapping Tracking**: Track which address spaces have a VMO mapped
- **Share Count**: Determine if a VMO is shared across processes
- **Add/Remove Mappings**: Update mapping list on map/unmap operations

**API**:
```rust
// Add mapping to address space
vmo.add_mapping(aspace_id: u64)

// Remove mapping
vmo.remove_mapping(aspace_id: u64)

// Check if shared
vmo.is_shared() -> bool
vmo.share_count() -> u32
```

**Status**: ✅ **Implemented** - Production Ready

---

### 2. Thread Management

#### 2.1 Thread Entry Point

**Location**: `src/kernel/thread.rs`

Provides standardized thread entry point handling with proper cleanup.

**Key Features**:
- **Entry Point Wrapper**: Standard `thread_entry()` function
- **Argument Passing**: Pass typed arguments to threads
- **Clean Exit**: Proper cleanup on thread exit
- **Result Reporting**: Threads can return results

**API**:
```rust
pub type ThreadEntryPoint = fn(*const u8) -> i32;

pub extern "C" fn thread_entry(arg: *const u8) -> i32 {
    // Call actual thread function
    // Handle exit
}
```

**Status**: ✅ **Implemented** - Production Ready

---

#### 2.2 Per-Thread Kernel Stacks

**Location**: `src/kernel/thread.rs`

Each thread gets its own kernel stack for kernel-mode execution.

**Key Features**:
- **Guard Pages**: Protected pages to detect stack overflow
- **Stack Allocation**: Automatic stack allocation on thread creation
- **Stack Size Configuration**: Configurable stack sizes
- **Stack Switching**: Proper context switching between thread stacks

**Status**: ✅ **Implemented** - Production Ready

---

#### 2.3 Idle Threads

**Location**: `src/kernel/thread.rs`

Per-CPU idle threads that run when no other work is available.

**Key Features**:
- **Halt on Idle**: CPU halts when idle to save power
- **Wake on Interrupt**: Interrupts wake the idle thread
- **Low Priority**: Lowest scheduling priority
- **One Per CPU**: Each CPU has its own idle thread

**Status**: ✅ **Implemented** - Production Ready

---

### 3. Interrupt Handling

#### 3.1 ARM64 GIC Support

**Location**: `src/kernel/arch/arm64/gic.rs`

Support for ARM Generic Interrupt Controller (GIC) v2/v3.

**Key Features**:
- **GICv2 and GICv3 Support**: Support for both GIC versions
- **IRQ Routing**: Proper interrupt routing to CPUs
- **Priority Handling**: Interrupt priority and masking
- **SGI/PPI/SPI**: Support for all GIC interrupt types

**Status**: ✅ **Implemented** - Production Ready

---

### 4. Memory Allocators

#### 4.1 Linked List Allocator

**Location**: `src/kernel/allocator.rs`

Heap allocator using linked list of free blocks.

**Key Features**:
- **First-Fit Allocation**: Fast allocation with first-fit strategy
- **Coalescing**: Merge adjacent free blocks
- **Splitting**: Split large blocks to satisfy small allocations
- **no_std Compatible**: Works without standard library

**Status**: ✅ **Implemented** - Production Ready

---

#### 4.2 Bump Allocator (Replaced)

**Status**: ❌ **Removed** - Replaced by Linked List Allocator

The bump allocator was replaced with the more efficient linked list allocator that supports deallocation.

---

### 5. Kernel Initialization

#### 5.1 Kernel Initialization Completion

**Location**: `src/kernel/init.rs`

Proper initialization sequence for all kernel subsystems.

**Initialization Order**:
1. Console (early output)
2. Physical Memory Manager (PMM)
3. Virtual Memory Manager (VMM)
4. Page Tables
5. Heap Allocator
6. Interrupt Controller
7. Scheduler
8. Idle Threads
9. System Calls

**Status**: ✅ **Implemented** - Production Ready

---

## Architecture Support

### x86-64 (AMD64)

**Location**: `src/kernel/arch/amd64/`

- **Page Tables**: MMU and EPT support
- **Large Pages**: 4KB, 2MB, 1GB
- **Page Splitting**: Dynamic large page splitting

**Status**: ✅ **Implemented** - Production Ready

---

### ARM64 (AArch64)

**Location**: `src/kernel/arch/arm64/`

- **Page Tables**: 4-level translation tables
- **Large Pages**: 4KB, 2MB, 1GB
- **GIC Support**: GICv2/v3 interrupt controller

**Status**: ✅ **Implemented** - Production Ready

---

### RISC-V

**Location**: `src/kernel/arch/riscv/`

- **Page Tables**: Sv39/Sv48 support
- **Large Pages**: 4KB, 2MB, 1GB
- **PLIC/CLINT**: Interrupt controller support

**Status**: ✅ **Implemented** - Production Ready

---

## Feature Status Summary

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| Thread Entry Point | ✅ Complete | P1 | Production Ready |
| Per-Thread Kernel Stacks | ✅ Complete | P1 | Production Ready |
| Kernel Initialization | ✅ Complete | P1 | Production Ready |
| VA→PA Walker | ✅ Complete | P1 | Production Ready |
| VM Debugging | ✅ Complete | P1 | Production Ready |
| COW Implementation | ✅ Complete | P2 | Production Ready |
| Shared Memory Support | ✅ Complete | P2 | Production Ready |
| ARM64 GIC Support | ✅ Complete | P2 | Production Ready |
| Page Tables Trait | ✅ Complete | P3 | 11 Methods Implemented |
| Replace Bump Allocator | ✅ Complete | P3 | Linked List Allocator |
| Linked List Allocator | ✅ Complete | P3 | Production Ready |
| Contiguous Allocation | ✅ Complete | P4 | Production Ready |
| Large Page Support | ✅ Complete | P4 | With Page Splitting |
| Demand Paging | ✅ Complete | P4 | With Zero Page Optimization |
| Page Eviction Policies | ✅ Complete | P4 | LRU, ARC, Clock |

---

## Usage Examples

### Creating a VMO

```rust
use kernel::object::vmo::{Vmo, VmoFlags};

// Create a 16KB VMO
let vmo = Vmo::create(0x4000, VmoFlags::empty())
    .expect("Failed to create VMO");

// Write data
let data = b"Hello, World!";
vmo.write(0, data).expect("Failed to write");

// Read data
let mut buf = [0u8; 13];
vmo.read(0, &mut buf).expect("Failed to read");
assert_eq!(&buf, data);
```

### COW Clone

```rust
// Create parent VMO
let parent = Vmo::create(0x1000, VmoFlags::empty())?;

// Clone with COW
let child = parent.clone(0, 0x1000)?;

// Writing to child creates new pages (copy-on-write)
child.write(0, b"Modified data")?;
// Parent still has original data
```

### Handling Page Faults

```rust
use kernel::vm::fault::{handle_page_fault, PageFaultInfo};

let info = PageFaultInfo::new(
    0x1000,        // faulting address
    0x01,          // write fault
    0x4000,        // instruction pointer
    true,          // from user mode
);

let aspace = get_current_address_space();
match handle_page_fault(info, &aspace) {
    PageFaultResult::Handled => { /* Resume execution */ }
    PageFaultResult::UserSpace => { /* Deliver signal */ }
    PageFaultResult::Fatal => { /* Kill process */ }
    PageFaultResult::Retry => { /* Retry instruction */ }
}
```

### Page Eviction

```rust
use kernel::vm::pager::{EvictionPolicy, PageEvictionTracker};

// Create eviction tracker with LRU policy
let tracker = PageEvictionTracker::new(EvictionPolicy::LRU);

// Track page accesses
tracker.track_page(vmo_id, offset, paddr);
tracker.record_access(vmo_id, offset);

// Find page to evict
if let Some((victim_vmo, victim_offset, victim_paddr)) = tracker.find_eviction_candidate() {
    // Evict the page
    free_physical_page(victim_paddr);
}
```

---

## Building

### Prerequisites

- Rust nightly toolchain
- QEMU (for testing)
- Target-specific toolchains (optional)

### Build Commands

```bash
# Build for x86-64
cargo build --target x86_64-unknown-none

# Build for ARM64
cargo build --target aarch64-unknown-none

# Build for RISC-V
cargo build --target riscv64gc-unknown-none

# Run tests
cargo test

# Build release version
cargo build --release
```

---

## Testing

### QEMU Testing

```bash
# Test x86-64
qemu-system-x86_64 -kernel target/x86_64-unknown-none/release/rustux

# Test ARM64
qemu-system-aarch64 -kernel target/aarch64-unknown-none/release/rustux -M virt

# Test RISC-V
qemu-system-riscv64 -kernel target/riscv64gc-unknown-none/release/rustux
```

---

## Documentation Structure

- **architecture.md**: Overall architecture design
- **phase_a_boot_bringup.md**: Boot and bringup procedures
- **phase_b_virtual_memory.md**: Virtual memory implementation
- **phase_c_threads_syscalls.md**: Thread and syscall handling
- **phase_d_kernel_objects.md**: Kernel object design
- **phase_e_memory_features.md**: Advanced memory features
- **phase_f_multiplatform.md**: Multi-platform support
- **phase_g_userspace_sdk.md**: Userspace SDK
- **phase_h_qa_testing.md**: QA and testing procedures
- **phase_i_docs_governance.md**: Documentation governance
- **todo.md**: Current TODO list

---

## Contributing

See `phase_i_docs_governance.md` for contribution guidelines and coding standards.

---

## License

MIT License - See LICENSE file for details.

---

## Contact

For questions or issues, please refer to the project documentation.

---

*Last Updated: January 6, 2026*
*Kernel Version: 0.1.0*
*Status: Production Ready (Basic Embedded/Microkernel Use Cases)*
