// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! Object types
//!
//! This module defines the different types of kernel objects.

#![no_std]

/// Kernel object types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ObjectType {
    /// Process object
    Process = 1,
    /// Thread object
    Thread = 2,
    /// VMO (Virtual Memory Object)
    Vmo = 3,
    /// VMAR (Virtual Memory Address Region)
    Vmar = 4,
    /// Channel object
    Channel = 5,
    /// Event object
    Event = 6,
    /// EventPair object
    EventPair = 7,
    /// Port object
    Port = 8,
    /// Job object
    Job = 9,
    /// Socket object
    Socket = 10,
    /// Interrupt object
    Interrupt = 11,
    /// PCI device object
    PciDevice = 12,
    /// Log object
    Log = 13,
    /// Timer object
    Timer = 14,
    /// Profile object
    Profile = 15,
    /// IOMMU object
    Iommu = 16,
    /// Pager object
    Pager = 17,
    /// Hypervisor object
    Hypervisor = 18,
    /// VCPU object
    Vcpu = 19,
    /// None (invalid)
    None = 0,
}

impl ObjectType {
    /// Convert from raw value
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            1 => ObjectType::Process,
            2 => ObjectType::Thread,
            3 => ObjectType::Vmo,
            4 => ObjectType::Vmar,
            5 => ObjectType::Channel,
            6 => ObjectType::Event,
            7 => ObjectType::EventPair,
            8 => ObjectType::Port,
            9 => ObjectType::Job,
            10 => ObjectType::Socket,
            11 => ObjectType::Interrupt,
            12 => ObjectType::PciDevice,
            13 => ObjectType::Log,
            14 => ObjectType::Timer,
            15 => ObjectType::Profile,
            16 => ObjectType::Iommu,
            17 => ObjectType::Pager,
            18 => ObjectType::Hypervisor,
            19 => ObjectType::Vcpu,
            _ => ObjectType::None,
        }
    }

    /// Convert to raw value
    pub fn into_raw(self) -> u32 {
        self as u32
    }
}

/// Object information that can be queried
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum ObjectInfo {
    /// Handle basic info
    Handle(HandleInfo),
    /// VMO info
    Vmo(VmoInfo),
    /// Process info
    Process(ProcessInfo),
    /// Thread info
    Thread(ThreadInfo),
    /// Job info
    Job(JobInfo),
    /// Channel info
    Channel(ChannelInfo),
    /// Timer info
    Timer(TimerInfo),
}

/// Basic handle information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HandleInfo {
    /// Object type
    pub object_type: ObjectType,
    /// Rights
    pub rights: u64,
    /// Reserved
    pub reserved: [u64; 3],
}

/// VMO information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VmoInfo {
    /// Size of the VMO
    pub size: u64,
    /// Flags
    pub flags: u32,
    /// Padding
    pub _pad: u32,
    /// Committed size
    pub committed_bytes: u64,
    /// Cache policy
    pub cache_policy: u32,
    /// Reserved
    pub reserved: [u64; 3],
}

/// Process information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessInfo {
    /// Process return code
    pub return_code: u64,
    /// Process state
    pub state: u32,
    /// Padding
    pub _pad: u32,
    /// Thread count
    pub thread_count: u64,
    /// Reserved
    pub reserved: [u64; 3],
}

/// Thread information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadInfo {
    /// Thread state
    pub state: u32,
    /// Wait reason
    pub wait_reason: u32,
    /// CPU affinity
    pub cpu_affinity: u64,
    /// Reserved
    pub reserved: [u64; 3],
}

/// Job information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct JobInfo {
    /// Return code
    pub return_code: u64,
    /// Process count
    pub process_count: u64,
    /// Reserved
    pub reserved: [u64; 3],
}

/// Channel information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChannelInfo {
    /// Maximum message size
    pub max_message_bytes: u64,
    /// Maximum handle count
    pub max_message_handles: u64,
    /// Reserved
    pub reserved: [u64; 2],
}

/// Timer information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimerInfo {
    /// Deadline
    pub deadline: u64,
    /// Flags
    pub flags: u32,
    /// Padding
    pub _pad: u32,
    /// Slack
    pub slack: u64,
    /// Reserved
    pub reserved: [u64; 2],
}
