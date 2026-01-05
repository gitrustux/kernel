# Phase D — Kernel Objects & IPC (Zircon-style)

**Status:** 🔄 In Progress (Core objects implemented, integration pending)

---

## Overview

This phase implements the **capability-based kernel object model** inspired by Zircon. All kernel resources are accessed through **handles with rights**, ensuring fine-grained access control.

---

## D-1. Handle & Rights Model

### Handle Structure

```rust
pub struct Handle {
    id: HandleId,
    base: Arc<KernelObject>,
    rights: Rights,
}
```

### Rights Bitmask

| Right | Value | Description |
|-------|-------|-------------|
| RIGHT_NONE | 0x00 | No rights |
| RIGHT_READ | 0x01 | Read state |
| RIGHT_WRITE | 0x02 | Modify state |
| RIGHT_EXECUTE | 0x04 | Execute code |
| RIGHT_SIGNAL | 0x08 | Signal/wait |
| RIGHT_MAP | 0x10 | Map into VMAR |
| RIGHT_DUPLICATE | 0x20 | Duplicate handle |
| RIGHT_TRANSFER | 0x40 | Transfer to process |
| RIGHT_MANAGE | 0x80 | Admin control |
| RIGHT_BASIC | 0x03 | READ | WRITE |
| RIGHT_DEFAULT | 0x1F | Basic + SIGNAL + MAP + DUPLICATE |
| RIGHT_SAME_RIGHTS | 0x8000_0000 | Keep same rights on dup |

### Enforcement

Every syscall validates rights before operation:

```rust
pub fn syscall_vmo_read(handle: Handle, offset: usize, buf: &mut [u8]) -> Result<usize> {
    let vmo = handle.base.downcast::<Vmo>()?;
    handle.rights.require(RIGHT_READ)?;
    vmo.read(offset, buf)
}
```

---

## D-2. Core Objects

### Object Type Hierarchy

```rust
pub enum KernelObject {
    Process(Process),
    Thread(Thread),
    Vmo(Vmo),
    Vmar(Vmar),
    Channel(Channel),
    Event(Event),
    EventPair(EventPair),
    Timer(Timer),
    Job(Job),
    Port(Port),
}
```

### Process

```rust
pub struct Process {
    pub pid: ProcessId,
    pub address_space: Arc<AddressSpace>,
    pub handle_table: HandleTable,
    pub threads: Vec<ThreadId>,
    pub state: ProcessState,
    pub job: Arc<Job>,
}

pub enum ProcessState {
    Created,
    Running,
    Terminated { exit_code: i64 },
}
```

**Required Rights:** MANAGE

**Operations:**
- Create, start, kill
- Get/kill threads
- Read/write memory (with DEBUG rights)

---

### Thread

```rust
pub struct Thread {
    pub tid: ThreadId,
    pub process: Arc<Process>,
    pub state: ThreadState,
    pub context: ArchThreadContext,
    pub kernel_stack: KernelStack,
    pub blocked: Option<BlockReason>,
}

pub enum ThreadState {
    New,
    Ready,
    Running,
    Blocked(BlockReason),
    Dying,
    Dead,
}

pub enum BlockReason {
    Channel(ChannelId),
    Timer(TimerId),
    Futex(FutexKey),
    Event(EventId),
}
```

**Required Rights:** MANAGE

**Operations:**
- Create, start, exit
- Read state, suspend, resume
- Kill

---

### VMO (Virtual Memory Object)

```rust
pub struct Vmo {
    pub id: VmoId,
    pub size: usize,
    pub pages: PageMap,
    pub parent: Option<VmoParent>,
    pub clones: Vec<VmoId>,
}

pub struct VmoParent {
    pub vmo: Arc<Vmo>,
    pub offset: usize,
    pub is_cow: bool,
}
```

**Required Rights:** READ, WRITE, MAP, EXECUTE (separately)

**Operations:**
- Create (with size and flags)
- Read/write (for direct access)
- Clone (COW)
- Resize (if resizable)
- Set cache policy

---

### VMAR (Virtual Memory Address Region)

```rust
pub struct Vmar {
    pub base: VAddr,
    pub size: usize,
    pub parent: Option<VmarParent>,
    pub children: RangeTree<VmarRegion>,
    pub mappings: RangeTree<VmoMapping>,
}

pub struct VmoMapping {
    pub vmo: Arc<Vmo>,
    pub offset: usize,
    pub vaddr: VAddr,
    pub size: usize,
    pub flags: MapFlags,
}
```

**Required Rights:** READ, WRITE, EXECUTE (mapping operations)

**Operations:**
- Map VMO (create new mapping)
- Protect (change flags)
- Unmap (remove mapping)
- Destroy (remove all children)

---

### Channel

```rust
pub struct Channel {
    pub id: ChannelId,
    pub peer: Option<ChannelId>,
    pub queue: RingBuf<Message>,
    pub waiter_count: AtomicUsize,
}

pub struct Message {
    pub bytes: Vec<u8>,
    pub handles: Vec<Handle>,
}

pub struct MessagePacket {
    pub tx_id: u64,
    pub data: Bytes,
    pub handles: Vec<Handle>,
}
```

**Required Rights:** READ, WRITE

**Operations:**
- Create (returns two endpoints)
- Write (send bytes + optional handles)
- Read (receive bytes + optional handles)
- Close

**Guarantees:**
- FIFO delivery order
- Bounded queue capacity
- Backpressure via SHOULD_WAIT

---

### Event / EventPair

```rust
pub struct Event {
    pub signaled: AtomicBool,
    pub waiters: WaitQueue,
}

pub struct EventPair {
    pub a: Event,
    pub b: Event,
}
```

**Required Rights:** SIGNAL, WAIT

**Operations:**
- Signal (set or clear)
- Wait (block until signaled)

---

### Timer

```rust
pub struct Timer {
    pub id: TimerId,
    pub deadline: AtomicU64,
    pub slack: u64,
    pub period: Option<NonZeroU64>,
    pub state: TimerState,
}

pub enum TimerState {
    Disarmed,
    Armed { deadline: u64 },
    Fired,
    Canceled,
}
```

**Required Rights:** SIGNAL, WRITE

**Operations:**
- Create
- Set (arm one-shot or repeating)
- Cancel

---

### Job

```rust
pub struct Job {
    pub id: JobId,
    pub parent: Option<Arc<Job>>,
    pub children: Vec<JobId>,
    pub processes: Vec<ProcessId>,
    pub policy: JobPolicy,
}

pub struct JobPolicy {
    pub timer_slack: SlackPolicy,
    pub cpu_affinity: CpuMask,
}
```

**Required Rights:** MANAGE, CREATE (child jobs)

**Operations:**
- Create (under parent)
- Add/remove processes
- Set policy
- Kill (terminates all children)

---

### Port (Waitset)

```rust
pub struct Port {
    pub id: PortId,
    pub queue: RingBuf<PacketEntry>,
    pub waiters: WaitQueue,
}

pub struct PacketEntry {
    pub key: u64,
    pub packet: Packet,
    pub status: PacketStatus,
}
```

**Required Rights:** READ, WRITE

**Operations:**
- Create
- Queue (add packet)
- Wait (receive packets)
- Cancel (remove pending)

---

## D-3. Capability Transfer via IPC

### Handle Passing

When sending handles via channel:

```rust
pub fn channel_write(&self, msg: Message, handles: Vec<Handle>) -> Result<()> {
    // Validate rights on each handle
    for h in &handles {
        h.rights.require(RIGHT_TRANSFER)?;
    }

    // Reduce rights by mask
    let reduced: Vec<_> = handles.iter()
        .map(|h| h.duplicate_with_mask(requested_rights))
        .collect();

    // Clear source handles
    handles.clear();

    // Add to message
    self.queue.push(Message { bytes: msg.bytes, handles: reduced });
}
```

### Rights Reduction

```
new_rights = old_rights ∧ requested_mask
```

Cannot add rights via transfer or duplication.

---

## D-4. Object Lifecycle

### Reference Counting

```rust
pub struct KernelObject {
    ref_count: AtomicUsize,
}
```

Rules:
- Handle holds reference
- Last handle closed → object destroyed (if no other kernel refs)
- Explicit `close` syscall is idempotent

### Termination Behavior

| Object | On Close |
|--------|----------|
| Process | Kill if running |
| Thread | Kill if running |
| Channel | Peer gets PEER_CLOSED |
| VMO | Unmap all mappings |
| Timer | Cancel if armed |
| Job | Kill all children |

---

## Done Criteria (Phase D)

- [ ] All core objects implemented
- [ ] Rights enforcement on all syscalls
- [ ] Handle passing via channels works
- [ ] IPC stress tests pass
- [ ] Object lifecycle verified

---

## Integration

### Files Implemented

| File | Purpose | Status | Lines |
|------|---------|--------|-------|
| `kernel/object/mod.rs` | Module exports | ✅ Complete | ~30 |
| `kernel/object/handle.rs` | Handle & Rights Model | ✅ Complete | ~550 |
| `kernel/object/vmo.rs` | Virtual Memory Objects | ✅ Complete | ~580 |
| `kernel/object/channel.rs` | IPC Channels | ✅ Complete | ~550 |
| `kernel/object/event.rs` | Event Objects | ✅ Complete | ~380 |
| `kernel/object/timer.rs` | Timer Objects | ✅ Complete | ~420 |

**Total:** ~2,510 lines of Rust code

### Files Remaining

| File | Purpose | Priority |
|------|---------|----------|
| `kernel/object/vmar.rs` | VM Address Regions | High |
| `kernel/object/job.rs` | Job Policy | Medium |
| `kernel/object/port.rs` | Port/Waitset | Medium |
| `kernel/object/process.rs` | Process Object | High |
| `kernel/object/thread.rs` | Thread Object | High |

---

## Implementation Summary

### D-1: Handle & Rights Model - ✅ Complete

**File:** `kernel/object/handle.rs`

- `Rights` struct with bitmask operations
- `Handle` struct with ID, object pointer, and rights
- `HandleTable` for per-process handle management
- `ObjectType` enum for type identification
- `KernelObjectBase` for reference counting
- Rights validation, duplication, and reduction

### D-2: Core Objects - ✅ Partial Complete

**VMO (`kernel/object/vmo.rs`):**
- Page-based memory management
- COW cloning support
- Resizable VMOs
- Read/write operations
- Cache policy control

**Channel (`kernel/object/channel.rs`):**
- Bidirectional message passing
- FIFO delivery ordering
- Bounded queue with backpressure
- Handle passing support
- Peer closure detection

**Event (`kernel/object/event.rs`):**
- Simple signaling primitive
- Auto-reset and manual-reset modes
- EventPair for mutual signaling

**Timer (`kernel/object/timer.rs`):**
- High-resolution timers (nanosecond precision)
- One-shot and periodic modes
- Slack policy for power efficiency
- Deadline-based firing

### D-3: Capability Transfer - 🚧 Pending

Handle passing infrastructure exists in `channel.rs` but needs:
- Integration with syscall layer
- Rights reduction on transfer
- Handle lifecycle management

### D-4: Object Lifecycle - 🚧 Pending

Reference counting infrastructure exists but needs:
- Integration with process cleanup
- Handle table management
- Object destruction callbacks

---

## Next Steps

→ **Proceed to [Phase E — Memory Management Features](phase_e_memory_features.md)**

---

*Phase D status updated: 2025-01-03*
