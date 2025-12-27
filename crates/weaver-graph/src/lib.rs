//! Call graph generation for the Weaver toolchain.
//!
//! This crate provides relational intelligence capabilities for understanding
//! code structure through call graph analysis. It implements a provider-based
//! architecture that can fuse data from multiple sources:
//!
//! - **LSP Provider**: Uses `textDocument/callHierarchy` requests for semantic
//!   call graph information
//! - **Static Analysis Provider** (planned): Wraps language-specific tools like
//!   `PyCG` for deeper analysis
//! - **Dynamic Analysis Provider** (planned): Ingests profiling data from tools
//!   like gprof and callgrind
//!
//! # Core Types
//!
//! - [`CallNode`] - Represents a function or method in the call graph
//! - [`CallEdge`] - Represents a call relationship between two nodes
//! - [`CallGraph`] - The complete graph structure with bidirectional indexing
//!
//! # Providers
//!
//! The [`CallGraphProvider`] trait abstracts over different data sources. The
//! initial implementation provides [`LspCallGraphProvider`] which queries LSP
//! servers for call hierarchy information.
//!
//! # Example
//!
//! ```ignore
//! use weaver_graph::{CallGraph, LspCallGraphProvider, CallGraphProvider};
//!
//! // Create a provider backed by an LSP client
//! let provider = LspCallGraphProvider::new(lsp_client);
//!
//! // Build a call graph starting from a symbol
//! let graph = provider.build_graph(start_position, depth)?;
//!
//! // Query callers and callees
//! for caller in graph.callers_of(&node_id) {
//!     // Process caller
//! }
//! ```

mod edge;
mod error;
mod graph;
mod node;
mod provider;
mod uri;

pub use edge::{CallEdge, EdgeSource};
pub use error::GraphError;
pub use graph::CallGraph;
pub use node::{CallNode, NodeId, Position, SymbolKind};
pub use provider::{CallGraphProvider, CallHierarchyClient, LspCallGraphProvider, SourcePosition};

#[cfg(test)]
mod tests;
