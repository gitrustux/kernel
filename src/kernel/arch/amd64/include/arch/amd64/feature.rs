// Rustux Authors 2025
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! x86 CPU feature detection and information
//!
//! This module provides functionality for detecting and querying CPU features,
//! topology, and other processor-specific information on x86 systems.

use core::sync::atomic::AtomicBool;
use crate::arch::amd64;
use crate::rustux::compiler::*;

// External static variables from C code (declared as pub for access across modules)
extern "C" {
    /// CPU vendor information from C code
    pub static X86_VENDOR_STATIC: X86VendorList;
    /// CPU microarchitecture information from C code
    pub static X86_MICROARCH_STATIC: X86MicroarchList;
    /// CPU hypervisor information from C code
    pub static X86_HYPERVISOR_STATIC: X86HypervisorList;
}

/// Maximum supported standard CPUID leaf
pub const MAX_SUPPORTED_CPUID: u32 = 0x17;
/// Maximum supported hypervisor CPUID leaf
pub const MAX_SUPPORTED_CPUID_HYP: u32 = 0x40000001;
/// Maximum supported extended CPUID leaf
pub const MAX_SUPPORTED_CPUID_EXT: u32 = 0x8000001e;

/// CPU identification leaf information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CpuidLeaf {
    /// EAX register value
    pub a: u32,
    /// EBX register value
    pub b: u32,
    /// ECX register value
    pub c: u32,
    /// EDX register value
    pub d: u32,
}

/// CPUID leaf numbers - using struct with constants instead of enum
/// to avoid enum discriminant overflow issues with values like 0x80000001
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86CpuidLeafNum {
    pub value: u32,
}

impl X86CpuidLeafNum {
    /// Base leaf
    pub const Base: u32 = 0;
    /// Model features leaf
    pub const ModelFeatures: u32 = 0x1;
    /// Cache info v1 leaf
    pub const CacheV1: u32 = 0x2;
    /// Cache info v2 leaf
    pub const CacheV2: u32 = 0x4;
    /// Monitor leaf
    pub const Mon: u32 = 0x5;
    /// Thermal and power leaf
    pub const ThermalAndPower: u32 = 0x6;
    /// Extended feature flags leaf
    pub const ExtendedFeatureFlags: u32 = 0x7;
    /// Performance monitoring leaf
    pub const PerformanceMonitoring: u32 = 0xa;
    /// Topology leaf
    pub const Topology: u32 = 0xb;
    /// XSAVE leaf
    pub const Xsave: u32 = 0xd;
    /// PT leaf
    pub const Pt: u32 = 0x14;
    /// TSC leaf
    pub const Tsc: u32 = 0x15;
    /// Hypervisor base leaf
    pub const HypBase: u32 = 0x40000000;
    /// Hypervisor vendor leaf
    pub const HypVendor: u32 = 0x40000000;
    /// KVM features leaf
    pub const KvmFeatures: u32 = 0x40000001;
    /// Extended base leaf
    pub const ExtBase: u32 = 0x80000000;
    /// Brand leaf
    pub const Brand: u32 = 0x80000002;
    /// Address width leaf
    pub const AddrWidth: u32 = 0x80000008;
    /// AMD topology leaf
    pub const AmdTopology: u32 = 0x8000001e;
}

impl From<u32> for X86CpuidLeafNum {
    fn from(value: u32) -> Self {
        Self { value }
    }
}

impl X86CpuidLeafNum {
    /// Get the raw u32 value
    pub fn as_u32(&self) -> u32 {
        self.value
    }
}

/// Structure to represent a specific CPU feature bit
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86CpuidBit {
    /// CPUID leaf number
    pub leaf_num: X86CpuidLeafNum,
    /// Register index (0-3 for EAX, EBX, ECX, EDX)
    pub word: u8,
    /// Bit position (0-31)
    pub bit: u8,
}

/// Macro to create an X86CpuidBit
#[macro_export]
macro_rules! x86_cpuid_bit {
    ($leaf:expr, $word:expr, $bit:expr) => {
        X86CpuidBit {
            leaf_num: X86CpuidLeafNum { value: $leaf },
            word: $word,
            bit: $bit,
        }
    };
}

// List of x86 CPU features
// format: x86_cpuid_bit!(cpuid leaf, register (eax-edx:0-3), bit)
pub const X86_FEATURE_SSE3: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 0);
pub const X86_FEATURE_MON: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 3);
pub const X86_FEATURE_VMX: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 5);
pub const X86_FEATURE_TM2: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 8);
pub const X86_FEATURE_SSSE3: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 9);
pub const X86_FEATURE_PDCM: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 15);
pub const X86_FEATURE_PCID: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 17);
pub const X86_FEATURE_SSE4_1: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 19);
pub const X86_FEATURE_SSE4_2: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 20);
pub const X86_FEATURE_X2APIC: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 21);
pub const X86_FEATURE_TSC_DEADLINE: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 24);
pub const X86_FEATURE_AESNI: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 25);
pub const X86_FEATURE_XSAVE: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 26);
pub const X86_FEATURE_AVX: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 28);
pub const X86_FEATURE_RDRAND: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 30);
pub const X86_FEATURE_HYPERVISOR: X86CpuidBit = x86_cpuid_bit!(0x1, 2, 31);
pub const X86_FEATURE_FPU: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 0);
pub const X86_FEATURE_SEP: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 11);
pub const X86_FEATURE_CLFLUSH: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 19);
pub const X86_FEATURE_ACPI: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 22);
pub const X86_FEATURE_MMX: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 23);
pub const X86_FEATURE_FXSR: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 24);
pub const X86_FEATURE_SSE: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 25);
pub const X86_FEATURE_SSE2: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 26);
pub const X86_FEATURE_TM: X86CpuidBit = x86_cpuid_bit!(0x1, 3, 29);
pub const X86_FEATURE_DTS: X86CpuidBit = x86_cpuid_bit!(0x6, 0, 0);
pub const X86_FEATURE_PLN: X86CpuidBit = x86_cpuid_bit!(0x6, 0, 4);
pub const X86_FEATURE_PTM: X86CpuidBit = x86_cpuid_bit!(0x6, 0, 6);
pub const X86_FEATURE_HWP: X86CpuidBit = x86_cpuid_bit!(0x6, 0, 7);
pub const X86_FEATURE_HWP_NOT: X86CpuidBit = x86_cpuid_bit!(0x6, 0, 8);
pub const X86_FEATURE_HWP_ACT: X86CpuidBit = x86_cpuid_bit!(0x6, 0, 9);
pub const X86_FEATURE_HWP_PREF: X86CpuidBit = x86_cpuid_bit!(0x6, 0, 10);
pub const X86_FEATURE_HW_FEEDBACK: X86CpuidBit = x86_cpuid_bit!(0x6, 2, 0);
pub const X86_FEATURE_PERF_BIAS: X86CpuidBit = x86_cpuid_bit!(0x6, 2, 3);
pub const X86_FEATURE_FSGSBASE: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 0);
pub const X86_FEATURE_TSC_ADJUST: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 1);
pub const X86_FEATURE_AVX2: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 5);
pub const X86_FEATURE_SMEP: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 7);
pub const X86_FEATURE_ERMS: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 9);
pub const X86_FEATURE_INVPCID: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 10);
pub const X86_FEATURE_RDSEED: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 18);
pub const X86_FEATURE_SMAP: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 20);
pub const X86_FEATURE_CLFLUSHOPT: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 23);
pub const X86_FEATURE_CLWB: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 24);
pub const X86_FEATURE_PT: X86CpuidBit = x86_cpuid_bit!(0x7, 1, 25);
pub const X86_FEATURE_UMIP: X86CpuidBit = x86_cpuid_bit!(0x7, 2, 2);
pub const X86_FEATURE_PKU: X86CpuidBit = x86_cpuid_bit!(0x7, 2, 3);
pub const X86_FEATURE_IBRS_IBPB: X86CpuidBit = x86_cpuid_bit!(0x7, 3, 26);
pub const X86_FEATURE_STIBP: X86CpuidBit = x86_cpuid_bit!(0x7, 3, 27);
pub const X86_FEATURE_SSBD: X86CpuidBit = x86_cpuid_bit!(0x7, 3, 31);
pub const X86_FEATURE_KVM_PVCLOCK_STABLE: X86CpuidBit = x86_cpuid_bit!(0x40000001, 0, 24);
pub const X86_FEATURE_AMD_TOPO: X86CpuidBit = x86_cpuid_bit!(0x80000001, 2, 22);
pub const X86_FEATURE_SYSCALL: X86CpuidBit = x86_cpuid_bit!(0x80000001, 3, 11);
pub const X86_FEATURE_NX: X86CpuidBit = x86_cpuid_bit!(0x80000001, 3, 20);
pub const X86_FEATURE_HUGE_PAGE: X86CpuidBit = x86_cpuid_bit!(0x80000001, 3, 26);
pub const X86_FEATURE_RDTSCP: X86CpuidBit = x86_cpuid_bit!(0x80000001, 3, 27);
pub const X86_FEATURE_INVAR_TSC: X86CpuidBit = x86_cpuid_bit!(0x80000007, 3, 8);

/// External CPUID leaf data arrays and limits
extern "C" {
    static _cpuid: [CpuidLeaf; MAX_SUPPORTED_CPUID as usize + 1];
    static _cpuid_hyp: [CpuidLeaf; MAX_SUPPORTED_CPUID_HYP as usize - X86CpuidLeafNum::HypBase as usize + 1];
    static _cpuid_ext: [CpuidLeaf; MAX_SUPPORTED_CPUID_EXT as usize - X86CpuidLeafNum::ExtBase as usize + 1];
    static max_cpuid: u32;
    static max_ext_cpuid: u32;
    static max_hyp_cpuid: u32;
}

/// Initialize x86 CPU feature detection
pub fn x86_feature_init() {
    unsafe { sys_x86_feature_init() }
}

/// Get a CPUID leaf
///
/// # Arguments
///
/// * `leaf` - The CPUID leaf number to retrieve
///
/// # Returns
///
/// A reference to the CPUID leaf data, or None if the leaf is not supported
pub fn x86_get_cpuid_leaf(leaf: X86CpuidLeafNum) -> Option<&'static CpuidLeaf> {
    unsafe {
        let leaf_num = leaf.as_u32();

        if leaf_num < X86CpuidLeafNum::HypBase {
            if unlikely(leaf_num > max_cpuid) {
                return None;
            }
            return Some(&_cpuid[leaf_num as usize]);
        } else if leaf_num < X86CpuidLeafNum::ExtBase {
            if unlikely(leaf_num > max_hyp_cpuid) {
                return None;
            }
            return Some(&_cpuid_hyp[(leaf_num - X86CpuidLeafNum::HypBase) as usize]);
        } else {
            if unlikely(leaf_num > max_ext_cpuid) {
                return None;
            }
            return Some(&_cpuid_ext[(leaf_num - X86CpuidLeafNum::ExtBase) as usize]);
        }
    }
}

/// Get a CPUID subleaf (non-cached)
///
/// # Arguments
///
/// * `leaf` - The CPUID leaf number
/// * `subleaf` - The subleaf index
/// * `out` - Output buffer for the CPUID data
///
/// # Returns
///
/// true if the leaf is valid and data was retrieved, false otherwise
pub fn x86_get_cpuid_subleaf(leaf: X86CpuidLeafNum, subleaf: u32, out: &mut CpuidLeaf) -> bool {
    unsafe { sys_x86_get_cpuid_subleaf(leaf, subleaf, out) }
}

/// Test if a CPU feature is supported
///
/// # Arguments
///
/// * `bit` - The feature bit to test
///
/// # Returns
///
/// true if the feature is supported, false otherwise
pub fn x86_feature_test(bit: X86CpuidBit) -> bool {
    debug_assert!(bit.word <= 3 && bit.bit <= 31, "Invalid CPUID bit: word={}, bit={}", bit.word, bit.bit);

    if bit.word > 3 || bit.bit > 31 {
        return false;
    }

    if let Some(leaf) = x86_get_cpuid_leaf(bit.leaf_num) {
        match bit.word {
            0 => ((1u32 << bit.bit) & leaf.a) != 0,
            1 => ((1u32 << bit.bit) & leaf.b) != 0,
            2 => ((1u32 << bit.bit) & leaf.c) != 0,
            3 => ((1u32 << bit.bit) & leaf.d) != 0,
            _ => false,
        }
    } else {
        false
    }
}

/// Print debug information about CPU features
pub fn x86_feature_debug() {
    unsafe { sys_x86_feature_debug() }
}

/// CPU vendor identification
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86VendorList {
    /// Unknown vendor
    Unknown,
    /// Intel
    Intel,
    /// AMD
    Amd,
}

/// Global CPU vendor information
/// Note: This is initialized by C code and should be updated during boot
pub static X86_VENDOR: X86VendorList = X86VendorList::Unknown;

/// Topology level type constants
pub const X86_TOPOLOGY_INVALID: u8 = 0;
/// SMT (Simultaneous Multi-Threading) topology level
pub const X86_TOPOLOGY_SMT: u8 = 1;
/// Core topology level
pub const X86_TOPOLOGY_CORE: u8 = 2;

/// CPU topology level information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86TopologyLevel {
    /// The number of bits to right shift to identify the next-higher topological level
    pub right_shift: u8,
    /// The type of relationship this level describes (hyperthread/core/etc)
    pub type_: u8,
}

/// Enumerate CPU topology levels
///
/// This interface is uncached.
///
/// # Arguments
///
/// * `level` - The level to retrieve info for. Should initially be 0 and
///             incremented with each call.
/// * `info` - The structure to populate with the discovered information
///
/// # Returns
///
/// true if the requested level existed (and there may be higher levels),
/// false if the requested level does not exist (and no higher ones do).
pub fn x86_topology_enumerate(level: u8, info: &mut X86TopologyLevel) -> bool {
    unsafe { sys_x86_topology_enumerate(level, info) }
}

/// CPU model information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct X86ModelInfo {
    /// Processor type
    pub processor_type: u8,
    /// Family
    pub family: u8,
    /// Model
    pub model: u8,
    /// Stepping
    pub stepping: u8,

    /// Display family
    pub display_family: u32,
    /// Display model
    pub display_model: u32,
}

/// Get CPU model information
///
/// # Returns
///
/// Reference to the CPU model information
pub fn x86_get_model() -> *const X86ModelInfo {
    unsafe { sys_x86_get_model() }
}

/// CPU microarchitecture identification
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86MicroarchList {
    /// Unknown microarchitecture
    Unknown,
    /// Intel Nehalem
    IntelNehalem,
    /// Intel Westmere
    IntelWestmere,
    /// Intel Sandy Bridge
    IntelSandyBridge,
    /// Intel Ivy Bridge
    IntelIvyBridge,
    /// Intel Broadwell
    IntelBroadwell,
    /// Intel Haswell
    IntelHaswell,
    /// Intel Skylake
    IntelSkylake,
    /// Intel Kabylake
    IntelKabylake,
    /// Intel Silvermont
    IntelSilvermont,
    /// AMD Bulldozer
    AmdBulldozer,
    /// AMD Jaguar
    AmdJaguar,
    /// AMD Zen
    AmdZen,
}

/// Global CPU microarchitecture information
/// Note: This is initialized by C code and should be updated during boot
pub static X86_MICROARCH: X86MicroarchList = X86MicroarchList::Unknown;

/// Global FSGSBASE feature flag
pub static G_X86_FEATURE_FSGSBASE: AtomicBool = AtomicBool::new(false);

/// Hypervisor identification
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86HypervisorList {
    /// Unknown hypervisor
    Unknown,
    /// KVM
    Kvm,
}

/// Global hypervisor information
pub static X86_HYPERVISOR: X86HypervisorList = X86HypervisorList::Unknown;

/// Function type for getting timer frequency (returns 0 if unknown, otherwise value in Hz)
pub type X86GetTimerFreqFunc = unsafe extern "C" fn() -> u64;

/// Function type for rebooting the system
pub type X86RebootSystemFunc = unsafe extern "C" fn();

/// Configuration for microarchitecture-specific features
#[repr(C)]
pub struct X86MicroarchConfig {
    /// Function to get APIC timer frequency
    pub get_apic_freq: Option<X86GetTimerFreqFunc>,
    /// Function to get TSC frequency
    pub get_tsc_freq: Option<X86GetTimerFreqFunc>,
    /// Function to reboot system
    pub reboot_system: Option<X86RebootSystemFunc>,
    /// Flag to disable C1E
    pub disable_c1e: bool,
}

/// Get the microarchitecture-specific configuration
///
/// # Returns
///
/// Reference to the microarchitecture configuration
pub fn x86_get_microarch_config() -> *const X86MicroarchConfig {
    unsafe { sys_x86_get_microarch_config() }
}

// Legacy CPU information accessors

/// Get the linear address width of the CPU
///
/// # Returns
///
/// The number of bits in the linear address space, or 0 if unknown
pub fn x86_linear_address_width() -> u8 {
    if let Some(leaf) = x86_get_cpuid_leaf(X86CpuidLeafNum { value: X86CpuidLeafNum::AddrWidth }) {
        // Extracting bit 15:8 from eax register
        // Bits 15-08: #Linear Address Bits
        ((leaf.a >> 8) & 0xff) as u8
    } else {
        0
    }
}

/// Get the physical address width of the CPU
///
/// # Returns
///
/// The number of bits in the physical address space, or 0 if unknown
pub fn x86_physical_address_width() -> u8 {
    if let Some(leaf) = x86_get_cpuid_leaf(X86CpuidLeafNum { value: X86CpuidLeafNum::AddrWidth }) {
        // Extracting bit 7:0 from eax register
        // Bits 07-00: #Physical Address Bits
        (leaf.a & 0xff) as u8
    } else {
        0
    }
}

/// Get the CLFLUSH line size
///
/// # Returns
///
/// The CLFLUSH line size in bytes, or 0 if unknown
pub fn x86_get_clflush_line_size() -> u32 {
    if let Some(leaf) = x86_get_cpuid_leaf(X86CpuidLeafNum { value: X86CpuidLeafNum::ModelFeatures }) {
        // Extracting bit 15:8 from ebx register
        // Bits 15-08: #CLFLUSH line size in quadwords
        ((leaf.b >> 8) & 0xff) * 8
    } else {
        0
    }
}

// FFI declarations for system functions
extern "C" {
    fn sys_x86_feature_init();
    fn sys_x86_get_cpuid_subleaf(leaf: X86CpuidLeafNum, subleaf: u32, out: *mut CpuidLeaf) -> bool;
    fn sys_x86_feature_debug();
    fn sys_x86_topology_enumerate(level: u8, info: *mut X86TopologyLevel) -> bool;
    fn sys_x86_get_model() -> *const X86ModelInfo;
    fn sys_x86_get_microarch_config() -> *const X86MicroarchConfig;
}