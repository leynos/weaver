//! Response types for the `observe graph-slice` operation.
//!
//! The response is either a successful [`GraphSlice`](GraphSliceResponse::Success)
//! containing cards, edges, and spillover metadata, or a structured refusal
//! in [`GraphSliceResponse::Refusal`] explaining why a slice could not be
//! produced.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{
    budget::SliceBudget,
    request::{SliceDirection, SliceEdgeType, SliceParseError, parse_variant},
};
use crate::{DetailLevel, SymbolCard};

/// Resolution scope for an edge, recording how the target was resolved.
///
/// This field tells consumers whether the match used the complete symbol
/// table, a partial subset, or was LSP-provided.
///
/// # Example
///
/// ```
/// use weaver_cards::graph_slice::ResolutionScope;
///
/// let scope = ResolutionScope::FullSymbolTable;
/// let json = serde_json::to_string(&scope).expect("serialization should succeed");
/// assert_eq!(json, "\"full_symbol_table\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ResolutionScope {
    /// Resolved against the complete symbol table.
    FullSymbolTable,
    /// Resolved against a partial symbol table (subset of files loaded).
    PartialSymbolTable,
    /// Resolved via an LSP-provided identifier.
    Lsp,
}

impl FromStr for ResolutionScope {
    type Err = SliceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_variant(
            s,
            &[
                ("full_symbol_table", Self::FullSymbolTable),
                ("partial_symbol_table", Self::PartialSymbolTable),
                ("lsp", Self::Lsp),
            ],
            "resolution scope",
            "full_symbol_table, partial_symbol_table, lsp",
        )
    }
}

/// Provenance metadata for a single edge.
///
/// Records the extraction source and optional call-site details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeProvenance {
    /// Extraction source (e.g. `lsp_call_hierarchy`,
    /// `tree_sitter_heuristic`).
    pub source: String,
    /// Optional call-site details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<EdgeProvenanceDetails>,
}

/// Call-site location within edge provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeProvenanceDetails {
    /// Call-site information.
    pub call_site: CallSiteInfo,
}

/// Location of a call site within a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSiteInfo {
    /// File URI of the call site.
    pub uri: String,
    /// Line number (1-indexed).
    pub line: u32,
    /// Column number (1-indexed).
    pub column: u32,
}

/// An external target reference for unresolved edges.
///
/// Used when the edge target cannot be resolved to a local symbol ID
/// (common in Tree-sitter-only modes or dynamic languages).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalTarget {
    /// Language of the external symbol.
    pub language: String,
    /// Qualified name of the external symbol.
    pub name: String,
}

/// The target of an edge in a graph slice.
///
/// Ensures exactly one target variant is present, preventing invalid states
/// where both or neither target is specified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EdgeTarget {
    /// A resolved local symbol ID.
    Local {
        /// Symbol ID of the target node.
        to: String,
    },
    /// An unresolved external symbol reference.
    External {
        /// External target details.
        to_external: ExternalTarget,
    },
}

/// A typed edge in a graph slice.
///
/// Edges carry their type, endpoints, confidence, direction, resolution
/// scope, and provenance. The target is always specified via the `target`
/// enum, which ensures exactly one of local or external is present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceEdge {
    /// Schema version for forward compatibility.
    pub edge_version: u32,
    /// Edge type (call, import, or config).
    #[serde(rename = "type")]
    pub edge_type: SliceEdgeType,
    /// Symbol ID of the source node.
    pub from: String,
    /// Target of this edge (local symbol or external reference).
    #[serde(flatten)]
    pub target: EdgeTarget,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Traversal direction in which this edge was discovered.
    pub direction: SliceDirection,
    /// How the target was resolved.
    pub resolution_scope: ResolutionScope,
    /// Extraction provenance.
    pub provenance: EdgeProvenance,
}

/// Entry symbol reference within a graph-slice response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceEntry {
    /// Symbol ID of the entry node.
    pub symbol_id: String,
}

/// Normalized constraints echoed back in the response.
///
/// Always present in a success response so callers can observe which
/// defaults were applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceConstraints {
    /// Applied traversal depth.
    pub depth: u32,
    /// Applied traversal direction.
    pub direction: SliceDirection,
    /// Applied edge type filter (canonical order).
    pub edge_types: Vec<SliceEdgeType>,
    /// Applied minimum confidence threshold.
    pub min_confidence: f64,
    /// Applied budget constraints.
    pub budget: SliceBudget,
    /// Applied entry card detail level.
    pub entry_detail: DetailLevel,
    /// Applied non-entry node detail level.
    pub node_detail: DetailLevel,
}

/// A candidate node that was excluded due to budget truncation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpilloverCandidate {
    /// Symbol ID of the excluded candidate.
    pub symbol_id: String,
    /// Depth at which the candidate was discovered.
    pub depth: u32,
}

/// Spillover metadata for a graph-slice response.
///
/// Always present in a success response. When `truncated` is `false` the
/// `frontier` is empty. When `truncated` is `true` the frontier contains
/// candidate nodes that were excluded by budget constraints.
///
/// # Example
///
/// ```
/// use weaver_cards::SliceSpillover;
///
/// let spillover = SliceSpillover::empty();
/// assert!(!spillover.truncated);
/// assert!(spillover.frontier.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceSpillover {
    /// Whether the traversal was truncated by budget constraints.
    pub truncated: bool,
    /// Candidate nodes excluded by truncation.
    pub frontier: Vec<SpilloverCandidate>,
}

impl SliceSpillover {
    /// Creates an empty spillover indicating no truncation occurred.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            truncated: false,
            frontier: Vec::new(),
        }
    }
}

/// Reason why a graph slice could not be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SliceRefusalReason {
    /// No symbol found at the requested position.
    NoSymbolAtPosition,
    /// The requested position is outside the file bounds.
    PositionOutOfRange,
    /// The requested language is not supported.
    UnsupportedLanguage,
    /// The operation is not yet fully implemented.
    NotYetImplemented,
    /// The requested detail level requires a backend that is unavailable.
    BackendUnavailable,
}

impl FromStr for SliceRefusalReason {
    type Err = SliceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_variant(
            s,
            &[
                ("no_symbol_at_position", Self::NoSymbolAtPosition),
                ("position_out_of_range", Self::PositionOutOfRange),
                ("unsupported_language", Self::UnsupportedLanguage),
                ("not_yet_implemented", Self::NotYetImplemented),
                ("backend_unavailable", Self::BackendUnavailable),
            ],
            "refusal reason",
            "no_symbol_at_position, position_out_of_range, unsupported_language, \
             not_yet_implemented, backend_unavailable",
        )
    }
}

/// Structured refusal payload returned when a slice cannot be produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceRefusal {
    /// Machine-readable reason code.
    pub reason: SliceRefusalReason,
    /// Human-readable explanation.
    pub message: String,
}

/// Response from the `observe graph-slice` operation.
///
/// Either a successful slice or a structured refusal explaining why
/// the slice could not be produced.
///
/// # Example
///
/// ```
/// use weaver_cards::GraphSliceResponse;
///
/// let response = GraphSliceResponse::not_yet_implemented();
/// assert!(matches!(response, GraphSliceResponse::Refusal { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum GraphSliceResponse {
    /// A graph slice was successfully constructed.
    Success {
        /// Schema version for forward compatibility.
        slice_version: u32,
        /// Entry symbol reference.
        entry: SliceEntry,
        /// Normalized constraints that were applied.
        constraints: SliceConstraints,
        /// Symbol cards included in the slice.
        cards: Vec<SymbolCard>,
        /// Typed edges included in the slice.
        edges: Vec<SliceEdge>,
        /// Spillover metadata (always present).
        spillover: SliceSpillover,
    },
    /// A graph slice could not be constructed.
    Refusal {
        /// Structured refusal with reason and message.
        refusal: SliceRefusal,
    },
}

impl GraphSliceResponse {
    /// Creates a refusal response indicating that graph-slice traversal
    /// is not yet implemented.
    #[must_use]
    pub fn not_yet_implemented() -> Self {
        Self::Refusal {
            refusal: SliceRefusal {
                reason: SliceRefusalReason::NotYetImplemented,
                message: String::from(concat!(
                    "observe graph-slice: ",
                    "graph-slice traversal is not yet implemented"
                )),
            },
        }
    }
}
