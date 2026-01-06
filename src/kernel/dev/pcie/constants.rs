// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! PCIe Constants
//!
//! This module contains all the standard PCI and PCIe constants,
//! register offsets, and bit definitions.

#![no_std]

// ============================================================================
// Configuration Space Offsets
// ============================================================================

/// Vendor ID offset
pub const PCI_CONFIG_VENDOR_ID: u8 = 0x00;

/// Device ID offset
pub const PCI_CONFIG_DEVICE_ID: u8 = 0x02;

/// Command register offset
pub const PCI_CONFIG_COMMAND: u8 = 0x04;

/// Status register offset
pub const PCI_CONFIG_STATUS: u8 = 0x06;

/// Revision ID offset
pub const PCI_CONFIG_REVISION_ID: u8 = 0x08;

/// Class code offset
pub const PCI_CONFIG_CLASS_CODE: u8 = 0x09;

/// Class code (interface) offset
pub const PCI_CONFIG_CLASS_CODE_INTR: u8 = 0x09;

/// Class code (subclass) offset
pub const PCI_CONFIG_CLASS_CODE_SUB: u8 = 0x0a;

/// Class code (base) offset
pub const PCI_CONFIG_CLASS_CODE_BASE: u8 = 0x0b;

/// Cache line size offset
pub const PCI_CONFIG_CACHE_LINE_SIZE: u8 = 0x0c;

/// Latency timer offset
pub const PCI_CONFIG_LATENCY_TIMER: u8 = 0x0d;

/// Header type offset
pub const PCI_CONFIG_HEADER_TYPE: u8 = 0x0e;

/// BIST offset
pub const PCI_CONFIG_BIST: u8 = 0x0f;

/// Base addresses offset
pub const PCI_CONFIG_BASE_ADDRESSES: u8 = 0x10;

/// Cardbus CIS pointer offset
pub const PCI_CONFIG_CARDBUS_CIS_PTR: u8 = 0x28;

/// Subsystem vendor ID offset
pub const PCI_CONFIG_SUBSYS_VENDOR_ID: u8 = 0x2c;

/// Subsystem ID offset
pub const PCI_CONFIG_SUBSYS_ID: u8 = 0x2e;

/// Expansion ROM base address offset
pub const PCI_CONFIG_EXP_ROM_ADDRESS: u8 = 0x30;

/// Capabilities pointer offset
pub const PCI_CONFIG_CAPABILITIES: u8 = 0x34;

/// Interrupt line offset
pub const PCI_CONFIG_INTERRUPT_LINE: u8 = 0x3c;

/// Interrupt pin offset
pub const PCI_CONFIG_INTERRUPT_PIN: u8 = 0x3d;

/// Min grant offset
pub const PCI_CONFIG_MIN_GRANT: u8 = 0x3e;

/// Max latency offset
pub const PCI_CONFIG_MAX_LATENCY: u8 = 0x3f;

// ============================================================================
// Header Type Register
// ============================================================================

/// Header type mask (bit 0-6)
pub const PCI_HEADER_TYPE_MASK: u8 = 0x7f;

/// Multi-function flag (bit 7)
pub const PCI_HEADER_TYPE_MULTI_FN: u8 = 0x80;

// ============================================================================
// Header Types
// ============================================================================

/// Standard header type (0)
pub const PCI_HEADER_TYPE_STANDARD: u8 = 0x00;

/// PCI-to-PCI bridge header type (1)
pub const PCI_HEADER_TYPE_PCI_BRIDGE: u8 = 0x01;

/// CardBus bridge header type (2)
pub const PCI_HEADER_TYPE_CARD_BUS: u8 = 0x02;

// ============================================================================
// Command Register Bits
// ============================================================================

/// I/O space enable
pub const PCI_COMMAND_IO_EN: u16 = 0x0001;

/// Memory space enable
pub const PCI_COMMAND_MEM_EN: u16 = 0x0002;

/// Bus master enable
pub const PCI_COMMAND_BUS_MASTER_EN: u16 = 0x0004;

/// Special cycles enable
pub const PCI_COMMAND_SPECIAL_EN: u16 = 0x0008;

/// Memory write and invalidate enable
pub const PCI_COMMAND_MEM_WR_INV_EN: u16 = 0x0010;

/// Palette snooping enable
pub const PCI_COMMAND_PAL_SNOOP_EN: u16 = 0x0020;

/// Parity error response enable
pub const PCI_COMMAND_PERR_RESP_EN: u16 = 0x0040;

/// Address/data stepping enable
pub const PCI_COMMAND_AD_STEP_EN: u16 = 0x0080;

/// SERR# enable
pub const PCI_COMMAND_SERR_EN: u16 = 0x0100;

/// Fast back-to-back enable
pub const PCI_COMMAND_FAST_B2B_EN: u16 = 0x0200;

/// Interrupt disable
pub const PCI_COMMAND_INT_DISABLE: u16 = 0x0400;

// ============================================================================
// PCIe General Constants
// ============================================================================

/// Maximum number of buses
pub const PCIE_MAX_BUSES: u16 = 256;

/// Maximum number of devices per bus
pub const PCIE_MAX_DEVICES_PER_BUS: u8 = 32;

/// Maximum number of functions per device
pub const PCIE_MAX_FUNCTIONS_PER_DEVICE: u8 = 8;

/// Maximum number of functions per bus
pub const PCIE_MAX_FUNCTIONS_PER_BUS: u16 = PCIE_MAX_DEVICES_PER_BUS as u16 * PCIE_MAX_FUNCTIONS_PER_DEVICE as u16;

/// Maximum number of legacy IRQ pins
pub const PCIE_MAX_LEGACY_IRQ_PINS: u8 = 4;

/// Maximum number of MSI IRQs
pub const PCIE_MAX_MSI_IRQS: u16 = 32;

/// Maximum number of MSI-X IRQs
pub const PCIE_MAX_MSIX_IRQS: u16 = 2048;

/// Standard config header size
pub const PCIE_STANDARD_CONFIG_HDR_SIZE: u16 = 64;

/// Base config size
pub const PCIE_BASE_CONFIG_SIZE: u16 = 256;

/// Extended config size (PCIe)
pub const PCIE_EXTENDED_CONFIG_SIZE: u16 = 4096;

/// ECAM bytes per bus
pub const PCIE_ECAM_BYTE_PER_BUS: u64 = PCIE_EXTENDED_CONFIG_SIZE as u64 * PCIE_MAX_FUNCTIONS_PER_BUS as u64;

/// BAR registers per bridge
pub const PCIE_BAR_REGS_PER_BRIDGE: u8 = 2;

/// BAR registers per device
pub const PCIE_BAR_REGS_PER_DEVICE: u8 = 6;

/// Maximum BAR registers
pub const PCIE_MAX_BAR_REGS: u8 = 6;

/// Invalid vendor ID (0xFFFF)
pub const PCIE_INVALID_VENDOR_ID: u16 = 0xFFFF;

// ============================================================================
// Capability Constants
// ============================================================================

/// Capability alignment
pub const PCIE_CAPABILITY_ALIGNMENT: u8 = 4;

/// Maximum number of standard capabilities
pub const PCIE_MAX_CAPABILITIES: u8 = ((PCIE_BASE_CONFIG_SIZE - PCIE_STANDARD_CONFIG_HDR_SIZE)
    / PCIE_CAPABILITY_ALIGNMENT as u16) as u8;

/// Null capability pointer
pub const PCIE_CAP_PTR_NULL: u8 = 0;

/// Minimum valid capability pointer
pub const PCIE_CAP_PTR_MIN_VALID: u8 = PCIE_STANDARD_CONFIG_HDR_SIZE as u8;

/// Maximum valid capability pointer
pub const PCIE_CAP_PTR_MAX_VALID: u8 = (PCIE_BASE_CONFIG_SIZE - PCIE_CAPABILITY_ALIGNMENT as u16) as u8;

/// Capability pointer alignment
pub const PCIE_CAP_PTR_ALIGNMENT: u8 = 2;

/// Extended capability null pointer
pub const PCIE_EXT_CAP_PTR_NULL: u16 = 0;

/// Minimum valid extended capability pointer
pub const PCIE_EXT_CAP_PTR_MIN_VALID: u16 = PCIE_BASE_CONFIG_SIZE as u16;

/// Maximum valid extended capability pointer
pub const PCIE_EXT_CAP_PTR_MAX_VALID: u16 = PCIE_EXTENDED_CONFIG_SIZE - (PCIE_CAPABILITY_ALIGNMENT as u16);

/// Extended capability pointer alignment
pub const PCIE_EXT_CAP_PTR_ALIGNMENT: u16 = 4;

/// Maximum number of extended capabilities
pub const PCIE_MAX_EXT_CAPABILITIES: u16 = (PCIE_EXTENDED_CONFIG_SIZE - PCIE_BASE_CONFIG_SIZE)
    as u16 / PCIE_CAPABILITY_ALIGNMENT as u16;

// ============================================================================
// BAR Register Masks and Constants
// ============================================================================

/// BAR I/O type mask (bit 0)
pub const PCI_BAR_IO_TYPE_MASK: u32 = 0x00000001;

/// BAR MMIO type
pub const PCI_BAR_IO_TYPE_MMIO: u32 = 0x00000000;

/// BAR PIO type
pub const PCI_BAR_IO_TYPE_PIO: u32 = 0x00000001;

/// BAR MMIO type mask (bit 1-2)
pub const PCI_BAR_MMIO_TYPE_MASK: u32 = 0x00000006;

/// BAR 32-bit MMIO
pub const PCI_BAR_MMIO_TYPE_32BIT: u32 = 0x00000000;

/// BAR 64-bit MMIO
pub const PCI_BAR_MMIO_TYPE_64BIT: u32 = 0x00000004;

/// BAR MMIO prefetch mask (bit 3)
pub const PCI_BAR_MMIO_PREFETCH_MASK: u32 = 0x00000008;

/// BAR MMIO address mask (bits 4-31)
pub const PCI_BAR_MMIO_ADDR_MASK: u32 = 0xFFFFFFF0;

/// BAR PIO address mask (bits 2-31)
pub const PCI_BAR_PIO_ADDR_MASK: u32 = 0xFFFFFFFC;

// ============================================================================
// PCIe Extended Command/Status Bits
// ============================================================================

/// PCIe interrupt disable
pub const PCIE_CFG_COMMAND_INT_DISABLE: u16 = 1 << 10;

/// PCIe interrupt status
pub const PCIE_CFG_STATUS_INT_STS: u16 = 1 << 3;

// ============================================================================
// Class Codes
// ============================================================================

/// Unclassified device
pub const PCI_CLASS_CODE_UNCLASSIFIED: u8 = 0x00;

/// Mass storage controller
pub const PCI_CLASS_CODE_MASS_STORAGE: u8 = 0x01;

/// Network controller
pub const PCI_CLASS_CODE_NETWORK: u8 = 0x02;

/// Display controller
pub const PCI_CLASS_CODE_DISPLAY: u8 = 0x03;

/// Multimedia device
pub const PCI_CLASS_CODE_MULTIMEDIA: u8 = 0x04;

/// Memory controller
pub const PCI_CLASS_CODE_MEMORY: u8 = 0x05;

/// Bridge device
pub const PCI_CLASS_CODE_BRIDGE: u8 = 0x06;

/// Simple communication controller
pub const PCI_CLASS_CODE_COMMUNICATION: u8 = 0x07;

/// Base system peripheral
pub const PCI_CLASS_CODE_PERIPHERAL: u8 = 0x08;

/// Input device
pub const PCI_CLASS_CODE_INPUT: u8 = 0x09;

/// Docking station
pub const PCI_CLASS_CODE_DOCKING: u8 = 0x0a;

/// Processor
pub const PCI_CLASS_CODE_PROCESSOR: u8 = 0x0b;

/// Serial bus controller
pub const PCI_CLASS_CODE_SERIAL_BUS: u8 = 0x0c;

/// Wireless controller
pub const PCI_CLASS_CODE_WIRELESS: u8 = 0x0d;

/// Intelligent I/O controller
pub const PCI_CLASS_CODE_INTELLIGENT_IO: u8 = 0x0e;

/// Satellite communication controller
pub const PCI_CLASS_CODE_SATELLITE: u8 = 0x0f;

/// Encryption/Decryption controller
pub const PCI_CLASS_CODE_ENCRYPTION: u8 = 0x10;

/// Data acquisition and signal processing
pub const PCI_CLASS_CODE_SIGNAL_PROCESSING: u8 = 0x11;

/// Processing accelerators
pub const PCI_CLASS_CODE_ACCELERATOR: u8 = 0x12;

/// Non-essential instrumentation
pub const PCI_CLASS_CODE_INSTRUMENTATION: u8 = 0x13;
