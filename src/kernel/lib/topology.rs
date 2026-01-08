// Copyright 2025 The Rustux Authors
//
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT

//! System Topology
//!
//! This module provides system topology representation for NUMA and SMP systems.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use spin::Mutex;

use crate::rustux::types::*;

/// Maximum topology depth
const MAX_TOPOLOGY_DEPTH: usize = 20;

/// No parent index
pub const ZBI_TOPOLOGY_NO_PARENT: u16 = 0xFFFF;

/// Topology entity types
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyEntityType {
    Undefined = 0,
    Processor = 1,
    Cluster = 2,
    NumARegion = 3,
}

/// Processor information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TopologyProcessor {
    /// Logical IDs for this processor
    pub logical_ids: &'static [u16],
    /// Number of logical IDs
    pub logical_id_count: u16,
    /// Flags
    pub flags: u32,
    /// Bootstrap ID
    pub bootstrap_id: u16,
    /// ACPI ID (for x86)
    pub acpi_id: u32,
    /// Architecture-specific data
    pub arch_id: u32,
}

/// Cluster information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TopologyCluster {
    /// Performance class
    pub performance_class: u8,
    /// Reserved
    pub reserved: [u8; 3],
}

/// NUMA region information
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TopologyNumaRegion {
    /// Region ID
    pub region_id: u64,
}

/// Topology node entity data
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub union TopologyEntity {
    pub processor: TopologyProcessor,
    pub cluster: TopologyCluster,
    pub numa_region: TopologyNumaRegion,
}

/// Topology node
#[repr(C)]
#[derive(Debug)]
pub struct TopologyNode {
    /// Entity type
    pub entity_type: TopologyEntityType,
    /// Entity data
    pub entity: TopologyEntity,
    /// Parent index
    pub parent_index: u16,
    /// Parent pointer (set after linking)
    pub parent: Option<*mut TopologyNode>,
    /// Children
    pub children: Vec<*mut TopologyNode>,
}

/// System topology graph
pub struct SystemTopologyGraph {
    /// All nodes in the topology
    nodes: Vec<TopologyNode>,
    /// List of processors
    processors: Vec<*mut TopologyNode>,
    /// Processors indexed by logical ID
    processors_by_logical_id: BTreeMap<u16, *mut TopologyNode>,
}

impl SystemTopologyGraph {
    /// Create a new empty topology graph
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            processors: Vec::new(),
            processors_by_logical_id: BTreeMap::new(),
        }
    }

    /// Update the topology graph from flat node data
    ///
    /// # Arguments
    ///
    /// * `flat_nodes` - Array of topology nodes
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err(status) on failure
    pub fn update(&mut self, flat_nodes: &[TopologyNode]) -> Result<(), i32> {
        if flat_nodes.is_empty() || !self.validate(flat_nodes) {
            return Err(-1); // ZX_ERR_INVALID_ARGS
        }

        if !self.nodes.is_empty() {
            return Err(-2); // ZX_ERR_ALREADY_EXISTS
        }

        // Create nodes from flat data
        for flat_node in flat_nodes {
            let mut node = TopologyNode {
                entity_type: flat_node.entity_type,
                entity: flat_node.entity,
                parent_index: flat_node.parent_index,
                parent: None,
                children: Vec::new(),
            };

            // Handle processor-specific initialization
            if node.entity_type == TopologyEntityType::Processor {
                self.processors.push(unsafe { core::mem::transmute(&mut node) });

                // Index by logical ID
                for i in 0..node.entity.processor.logical_id_count {
                    let logical_id = node.entity.processor.logical_ids[i as usize];
                    self.processors_by_logical_id
                        .insert(logical_id, unsafe { core::mem::transmute(&mut node) });
                }
            }

            self.nodes.push(node);
        }

        // Link parents and children
        for i in 0..self.nodes.len() {
            let node_ptr = unsafe { core::mem::transmute::<*mut TopologyNode, *mut TopologyNode>(&mut self.nodes[i]) };

            if self.nodes[i].parent_index != ZBI_TOPOLOGY_NO_PARENT {
                let parent_index = self.nodes[i].parent_index as usize;

                if parent_index < self.nodes.len() {
                    self.nodes[i].parent = Some(node_ptr);

                    // Add to parent's children
                    unsafe {
                        (*node_ptr).parent = Some(core::mem::transmute(&mut self.nodes[parent_index]));
                        (*(*node_ptr).parent.as_mut().unwrap()).children.push(node_ptr);
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate the topology graph structure
    ///
    /// # Arguments
    ///
    /// * `flat_nodes` - Array of topology nodes to validate
    ///
    /// # Returns
    ///
    /// true if valid, false otherwise
    pub fn validate(&self, flat_nodes: &[TopologyNode]) -> bool {
        let mut parents = [ZBI_TOPOLOGY_NO_PARENT; MAX_TOPOLOGY_DEPTH];
        let mut current_type = TopologyEntityType::Undefined;
        let mut current_depth = 0i32;

        for current_index in (0..flat_nodes.len()).rev() {
            let node = &flat_nodes[current_index];

            // Initialize current type
            if current_type == TopologyEntityType::Undefined {
                current_type = node.entity_type;
            }

            // Check type consistency
            if current_type != node.entity_type {
                if current_index as u16 == parents[current_depth as usize] {
                    // Type change - moving up a level
                    current_depth += 1;

                    if current_depth >= MAX_TOPOLOGY_DEPTH as i32 {
                        self.validation_error(
                            current_index,
                            "Structure is too deep, we only support 20 levels.",
                        );
                        return false;
                    }
                } else if node.entity_type == TopologyEntityType::Processor {
                    // New branch - reset to bottom
                    for i in 0..current_depth {
                        parents[i as usize] = ZBI_TOPOLOGY_NO_PARENT;
                    }
                    current_depth = 0;
                } else {
                    self.validation_error(
                        current_index,
                        "Graph is not stored in correct order, with children adjacent to parents",
                    );
                    return false;
                }

                current_type = node.entity_type;
            }

            // Check parent consistency
            if parents[current_depth as usize] == ZBI_TOPOLOGY_NO_PARENT {
                parents[current_depth as usize] = node.parent_index;
            } else if parents[current_depth as usize] != node.parent_index {
                self.validation_error(current_index, "Parents at level do not match.");
                return false;
            }

            // Ensure leaf nodes are processors
            if current_depth == 0 && node.entity_type != TopologyEntityType::Processor {
                self.validation_error(current_index, "Encountered a leaf node that isn't a processor.");
                return false;
            }

            // Ensure processors are leaf nodes
            if current_depth != 0 && node.entity_type == TopologyEntityType::Processor {
                self.validation_error(current_index, "Encountered a processor that isn't a leaf node.");
                return false;
            }

            // Top-level node should not have a parent
            if current_index == 0
                && parents[current_depth as usize] != ZBI_TOPOLOGY_NO_PARENT
                && (current_depth == MAX_TOPOLOGY_DEPTH as i32 - 1
                    || parents[current_depth as usize + 1] == ZBI_TOPOLOGY_NO_PARENT)
            {
                self.validation_error(current_index, "Top level of tree should not have a parent");
                return false;
            }
        }

        true
    }

    /// Print a validation error
    fn validation_error(&self, index: usize, message: &str) {
        println!("Error validating topology at node {}: {}", index, message);
    }

    /// Get all processors
    pub fn processors(&self) -> &[*mut TopologyNode] {
        &self.processors
    }

    /// Get processor by logical ID
    pub fn processor_by_logical_id(&self, logical_id: u16) -> Option<*mut TopologyNode> {
        self.processors_by_logical_id.get(&logical_id).copied()
    }

    /// Get the number of processors
    pub fn processor_count(&self) -> usize {
        self.processors.len()
    }

    /// Get all nodes
    pub fn nodes(&self) -> &[TopologyNode] {
        &self.nodes
    }
}

impl Default for SystemTopologyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Global system topology instance
static SYSTEM_TOPOLOGY: Mutex<Option<SystemTopologyGraph>> = Mutex::new(None);

/// Initialize the system topology
///
/// # Arguments
///
/// * `flat_nodes` - Array of topology nodes from ZBI
pub fn system_topology_init(flat_nodes: &[TopologyNode]) -> Result<(), i32> {
    let mut topology = SYSTEM_TOPOLOGY.lock();

    if topology.is_some() {
        return Err(-2); // Already initialized
    }

    let mut graph = SystemTopologyGraph::new();
    graph.update(flat_nodes)?;

    *topology = Some(graph);

    println!(
        "System topology: {} processors initialized",
        flat_nodes.iter()
            .filter(|n| n.entity_type == TopologyEntityType::Processor)
            .count()
    );

    Ok(())
}

/// Get the system topology
pub fn system_topology_get() -> Option<SystemTopologyGraph> {
    // Return a copy (since we can't return a reference to the locked data)
    SYSTEM_TOPOLOGY.lock().as_ref().map(|_| {
        // TODO: Implement proper cloning
        SystemTopologyGraph::new()
    })
}

/// Get processor count
pub fn system_topology_processor_count() -> usize {
    SYSTEM_TOPOLOGY
        .lock()
        .as_ref()
        .map(|t| t.processor_count())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_graph_create() {
        let graph = SystemTopologyGraph::new();
        assert_eq!(graph.processor_count(), 0);
        assert!(graph.nodes().is_empty());
    }

    #[test]
    fn test_topology_entity_types() {
        assert_eq!(TopologyEntityType::Undefined as u8, 0);
        assert_eq!(TopologyEntityType::Processor as u8, 1);
        assert_eq!(TopologyEntityType::Cluster as u8, 2);
        assert_eq!(TopologyEntityType::NumARegion as u8, 3);
    }

    #[test]
    fn test_topology_constants() {
        assert_eq!(ZBI_TOPOLOGY_NO_PARENT, 0xFFFF);
        assert_eq!(MAX_TOPOLOGY_DEPTH, 20);
    }
}
