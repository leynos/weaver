//! Call graph structure with bidirectional indexing.

use std::collections::{HashMap, HashSet};

use crate::edge::CallEdge;
use crate::error::GraphError;
use crate::node::{CallNode, NodeId};

/// A call graph with bidirectional indexing for efficient traversal.
///
/// The graph maintains indices for both incoming calls (callers) and outgoing
/// calls (callees) to support efficient queries in either direction.
#[derive(Debug, Clone, Default)]
pub struct CallGraph {
    /// All nodes in the graph, keyed by node ID.
    nodes: HashMap<NodeId, CallNode>,
    /// All edges in the graph.
    edges: Vec<CallEdge>,
    /// Index of callers for each node (incoming edges).
    callers_index: HashMap<NodeId, HashSet<usize>>,
    /// Index of callees for each node (outgoing edges).
    callees_index: HashMap<NodeId, HashSet<usize>>,
}

impl CallGraph {
    /// Creates a new empty call graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node to the graph.
    ///
    /// If a node with the same ID already exists, it is replaced.
    pub fn add_node(&mut self, node: CallNode) {
        let id = node.id().clone();
        self.nodes.insert(id.clone(), node);
        // Ensure index entries exist for new nodes
        self.callers_index.entry(id.clone()).or_default();
        self.callees_index.entry(id).or_default();
    }

    /// Adds an edge to the graph.
    ///
    /// The caller and callee nodes should already be present in the graph.
    pub fn add_edge(&mut self, edge: CallEdge) {
        let edge_index = self.edges.len();
        let from_id = edge.caller().clone();
        let to_id = edge.callee().clone();

        self.edges.push(edge);

        // Update indices
        self.callees_index
            .entry(from_id)
            .or_default()
            .insert(edge_index);
        self.callers_index
            .entry(to_id)
            .or_default()
            .insert(edge_index);
    }

    /// Returns the node with the given ID.
    #[must_use]
    pub fn node(&self, id: &NodeId) -> Option<&CallNode> {
        self.nodes.get(id)
    }

    /// Returns an iterator over all nodes in the graph.
    pub fn nodes(&self) -> impl Iterator<Item = &CallNode> {
        self.nodes.values()
    }

    /// Returns the number of nodes in the graph.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns an iterator over all edges in the graph.
    pub fn edges(&self) -> impl Iterator<Item = &CallEdge> {
        self.edges.iter()
    }

    /// Returns the number of edges in the graph.
    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns the edges representing calls *to* the given node.
    ///
    /// These are the incoming edges where the given node is the callee.
    pub fn incoming_edges(&self, node_id: &NodeId) -> impl Iterator<Item = &CallEdge> {
        self.callers_index
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|&idx| self.edges.get(idx))
    }

    /// Returns the edges representing calls *from* the given node.
    ///
    /// These are the outgoing edges where the given node is the caller.
    pub fn outgoing_edges(&self, node_id: &NodeId) -> impl Iterator<Item = &CallEdge> {
        self.callees_index
            .get(node_id)
            .into_iter()
            .flatten()
            .filter_map(|&idx| self.edges.get(idx))
    }

    /// Returns the nodes that call the given node.
    pub fn callers_of(&self, node_id: &NodeId) -> impl Iterator<Item = &CallNode> {
        self.incoming_edges(node_id)
            .filter_map(|edge| self.nodes.get(edge.caller()))
    }

    /// Returns the nodes that are called by the given node.
    pub fn callees_of(&self, node_id: &NodeId) -> impl Iterator<Item = &CallNode> {
        self.outgoing_edges(node_id)
            .filter_map(|edge| self.nodes.get(edge.callee()))
    }

    /// Returns whether the graph contains a node with the given ID.
    #[must_use]
    pub fn contains_node(&self, id: &NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Returns whether the graph is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Merges another graph into this one.
    ///
    /// Nodes with the same ID are replaced. All edges from the other graph
    /// are added.
    pub fn merge(&mut self, other: Self) {
        for node in other.nodes.into_values() {
            self.add_node(node);
        }
        for edge in other.edges {
            self.add_edge(edge);
        }
    }

    /// Finds a node by name.
    ///
    /// Returns the first node with a matching name. For methods, searches both
    /// the simple name and qualified name.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<&CallNode> {
        self.nodes
            .values()
            .find(|node| node.name() == name || node.qualified_name() == name)
    }

    /// Returns the node with the given ID, or an error if not found.
    ///
    /// # Errors
    /// Returns `GraphError::NodeNotFound` if no node with the given ID exists.
    pub fn get_node(&self, id: &NodeId) -> Result<&CallNode, GraphError> {
        self.node(id)
            .ok_or_else(|| GraphError::node_not_found(id.as_str()))
    }
}
