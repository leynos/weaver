//! Provider abstractions for call graph construction.
//!
//! This module defines the [`CallGraphProvider`] trait and its LSP-based
//! implementation. The provider pattern enables fusion of multiple data
//! sources (LSP, static analysis, profiling) for richer call graphs.

use camino::Utf8PathBuf;
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyItem, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
    WorkDoneProgressParams,
};

use crate::edge::{CallEdge, EdgeSource};
use crate::error::GraphError;
use crate::graph::CallGraph;
use crate::node::{CallNode, SymbolKind};

/// A position in a source file for initiating call graph queries.
#[derive(Debug, Clone)]
pub struct SourcePosition {
    /// Path to the source file.
    pub path: Utf8PathBuf,
    /// Line number (0-based).
    pub line: u32,
    /// Column number (0-based).
    pub column: u32,
}

impl SourcePosition {
    /// Creates a new source position.
    #[must_use]
    pub fn new(path: impl Into<Utf8PathBuf>, line: u32, column: u32) -> Self {
        Self {
            path: path.into(),
            line,
            column,
        }
    }
}

/// Trait for call graph data providers.
///
/// Implementations can source call graph data from various backends:
/// - LSP servers via `textDocument/callHierarchy`
/// - Static analysis tools
/// - Dynamic profiling data
pub trait CallGraphProvider {
    /// Builds a call graph starting from the given position.
    ///
    /// # Arguments
    /// * `position` - Starting position for the call graph
    /// * `depth` - Maximum traversal depth (0 = just the item at position)
    ///
    /// # Errors
    /// Returns an error if the symbol cannot be found or the provider fails.
    fn build_graph(
        &mut self,
        position: &SourcePosition,
        depth: u32,
    ) -> Result<CallGraph, GraphError>;

    /// Builds a call graph showing callers of the symbol at the position.
    ///
    /// # Arguments
    /// * `position` - Position of the symbol to find callers for
    /// * `depth` - Maximum traversal depth
    ///
    /// # Errors
    /// Returns an error if the symbol cannot be found or the provider fails.
    fn callers_graph(
        &mut self,
        position: &SourcePosition,
        depth: u32,
    ) -> Result<CallGraph, GraphError>;

    /// Builds a call graph showing callees of the symbol at the position.
    ///
    /// # Arguments
    /// * `position` - Position of the symbol to find callees for
    /// * `depth` - Maximum traversal depth
    ///
    /// # Errors
    /// Returns an error if the symbol cannot be found or the provider fails.
    fn callees_graph(
        &mut self,
        position: &SourcePosition,
        depth: u32,
    ) -> Result<CallGraph, GraphError>;
}

/// Client abstraction for LSP call hierarchy operations.
///
/// This trait enables testing with mock clients and abstracts over
/// different LSP client implementations.
pub trait CallHierarchyClient {
    /// Prepares call hierarchy items at the given position.
    ///
    /// # Errors
    /// Returns an error if the LSP request fails.
    fn prepare_call_hierarchy(
        &mut self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, GraphError>;

    /// Gets incoming calls for the given item.
    ///
    /// # Errors
    /// Returns an error if the LSP request fails.
    fn incoming_calls(
        &mut self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyIncomingCall>>, GraphError>;

    /// Gets outgoing calls for the given item.
    ///
    /// # Errors
    /// Returns an error if the LSP request fails.
    fn outgoing_calls(
        &mut self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>, GraphError>;
}

/// LSP-based call graph provider.
///
/// Uses `textDocument/callHierarchy` requests to build call graphs from
/// language server data.
pub struct LspCallGraphProvider<C> {
    client: C,
}

impl<C> LspCallGraphProvider<C> {
    /// Creates a new LSP call graph provider with the given client.
    #[must_use]
    pub const fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C: CallHierarchyClient> CallGraphProvider for LspCallGraphProvider<C> {
    fn build_graph(
        &mut self,
        position: &SourcePosition,
        depth: u32,
    ) -> Result<CallGraph, GraphError> {
        let mut graph = CallGraph::new();

        // Prepare call hierarchy at the position
        let items = self.prepare_at_position(position)?;

        if items.is_empty() {
            return Err(GraphError::symbol_not_found(
                &position.path,
                position.line,
                position.column,
            ));
        }

        // Add the root nodes and explore both directions
        for item in &items {
            let node = call_hierarchy_item_to_node(item);
            graph.add_node(node);
        }

        if depth > 0 {
            // Explore callers and callees
            for item in &items {
                self.explore_callers(&mut graph, item, depth)?;
                self.explore_callees(&mut graph, item, depth)?;
            }
        }

        Ok(graph)
    }

    fn callers_graph(
        &mut self,
        position: &SourcePosition,
        depth: u32,
    ) -> Result<CallGraph, GraphError> {
        let mut graph = CallGraph::new();

        let items = self.prepare_at_position(position)?;

        if items.is_empty() {
            return Err(GraphError::symbol_not_found(
                &position.path,
                position.line,
                position.column,
            ));
        }

        for item in &items {
            let node = call_hierarchy_item_to_node(item);
            graph.add_node(node);
            self.explore_callers(&mut graph, item, depth)?;
        }

        Ok(graph)
    }

    fn callees_graph(
        &mut self,
        position: &SourcePosition,
        depth: u32,
    ) -> Result<CallGraph, GraphError> {
        let mut graph = CallGraph::new();

        let items = self.prepare_at_position(position)?;

        if items.is_empty() {
            return Err(GraphError::symbol_not_found(
                &position.path,
                position.line,
                position.column,
            ));
        }

        for item in &items {
            let node = call_hierarchy_item_to_node(item);
            graph.add_node(node);
            self.explore_callees(&mut graph, item, depth)?;
        }

        Ok(graph)
    }
}

impl<C: CallHierarchyClient> LspCallGraphProvider<C> {
    fn prepare_at_position(
        &mut self,
        position: &SourcePosition,
    ) -> Result<Vec<CallHierarchyItem>, GraphError> {
        let uri = path_to_uri(&position.path)?;
        let params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position::new(position.line, position.column),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.client
            .prepare_call_hierarchy(params)
            .map(Option::unwrap_or_default)
    }

    fn explore_callers(
        &mut self,
        graph: &mut CallGraph,
        item: &CallHierarchyItem,
        remaining_depth: u32,
    ) -> Result<(), GraphError> {
        if remaining_depth == 0 {
            return Ok(());
        }

        let params = CallHierarchyIncomingCallsParams {
            item: item.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let incoming = self.client.incoming_calls(params)?.unwrap_or_default();

        let target_node = call_hierarchy_item_to_node(item);
        let target_id = target_node.id().clone();

        for call in incoming {
            let from_node = call_hierarchy_item_to_node(&call.from);
            let from_id = from_node.id().clone();

            if !graph.contains_node(&from_id) {
                graph.add_node(from_node);
            }

            // Create edge from caller to callee
            let mut edge = CallEdge::new(from_id, target_id.clone(), EdgeSource::Lsp);

            // Use the first call site range if available
            if let Some(range) = call.from_ranges.first() {
                edge = edge.with_call_site(range.start.line, range.start.character);
            }

            graph.add_edge(edge);

            // Recursively explore callers
            if remaining_depth > 1 {
                self.explore_callers(graph, &call.from, remaining_depth.saturating_sub(1))?;
            }
        }

        Ok(())
    }

    fn explore_callees(
        &mut self,
        graph: &mut CallGraph,
        item: &CallHierarchyItem,
        remaining_depth: u32,
    ) -> Result<(), GraphError> {
        if remaining_depth == 0 {
            return Ok(());
        }

        let params = CallHierarchyOutgoingCallsParams {
            item: item.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let outgoing = self.client.outgoing_calls(params)?.unwrap_or_default();

        let source_node = call_hierarchy_item_to_node(item);
        let source_id = source_node.id().clone();

        for call in outgoing {
            let target_node = call_hierarchy_item_to_node(&call.to);
            let target_id = target_node.id().clone();

            if !graph.contains_node(&target_id) {
                graph.add_node(target_node);
            }

            // Create edge from caller to callee
            let mut edge = CallEdge::new(source_id.clone(), target_id, EdgeSource::Lsp);

            // Use the first call site range if available
            if let Some(range) = call.from_ranges.first() {
                edge = edge.with_call_site(range.start.line, range.start.character);
            }

            graph.add_edge(edge);

            // Recursively explore callees
            if remaining_depth > 1 {
                self.explore_callees(graph, &call.to, remaining_depth.saturating_sub(1))?;
            }
        }

        Ok(())
    }
}

/// Converts an LSP `CallHierarchyItem` to our domain `CallNode`.
fn call_hierarchy_item_to_node(item: &CallHierarchyItem) -> CallNode {
    let path = uri_to_path(&item.uri);
    let kind = SymbolKind::from_lsp(item.kind);
    let line = item.selection_range.start.line;
    let column = item.selection_range.start.character;

    let mut node = CallNode::new(&item.name, kind, path, line, column);

    if let Some(detail) = &item.detail {
        node = node.with_container(detail.clone());
    }

    node
}

/// Converts a URI to a `Utf8PathBuf`.
fn uri_to_path(uri: &Uri) -> Utf8PathBuf {
    // Try to extract the path from a file:// URI
    let uri_str = uri.as_str();
    uri_str.strip_prefix("file://").map_or_else(
        || Utf8PathBuf::from(uri_str),
        |path| Utf8PathBuf::from(percent_decode(path)),
    )
}

/// Converts a path to a file:// URI.
fn path_to_uri(path: &Utf8PathBuf) -> Result<Uri, GraphError> {
    let uri_string = format!("file://{path}");
    uri_string.parse().map_err(|_| {
        GraphError::io(
            format!("failed to convert path to URI: {path}"),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path"),
        )
    })
}

/// Simple percent-decoding for URI paths.
fn percent_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Try to decode a percent-encoded sequence
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                result.push(byte as char);
                continue;
            }
            // If decoding failed, keep the original
            result.push('%');
            result.push_str(&hex);
        } else {
            result.push(c);
        }
    }

    result
}
