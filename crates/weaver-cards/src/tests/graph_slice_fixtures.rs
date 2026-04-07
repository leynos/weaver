//! Shared fixture builders for graph-slice test suites.
//!
//! These helpers provide canonical sample data for graph-slice schema
//! types. Test modules that need richer or variant data can build on
//! top of these foundations.

use crate::graph_slice::{
    CallSiteInfo, EdgeProvenance, EdgeProvenanceDetails, ExternalTarget, GraphSliceResponse,
    ResolutionScope, SliceConstraints, SliceEdge, SliceEntry, SliceRefusal, SliceRefusalReason,
    SliceSpillover, SpilloverCandidate,
};
use crate::{DetailLevel, SliceBudget, SliceDirection, SliceEdgeType};

use super::fixtures;

/// Builds a default-budget success response with one card and one edge.
pub fn sample_success_response() -> GraphSliceResponse {
    let entry_card = fixtures::build_card_at_level(DetailLevel::Structure);
    let edge = sample_resolved_call_edge();

    GraphSliceResponse::Success {
        slice_version: 1,
        entry: SliceEntry {
            symbol_id: String::from("sym_abc123"),
        },
        constraints: sample_constraints(),
        cards: vec![entry_card],
        edges: vec![edge],
        spillover: SliceSpillover::empty(),
    }
}

/// Builds a truncated success response with spillover metadata.
pub fn sample_truncated_response() -> GraphSliceResponse {
    let entry_card = fixtures::build_card_at_level(DetailLevel::Structure);
    let edge = sample_resolved_call_edge();

    GraphSliceResponse::Success {
        slice_version: 1,
        entry: SliceEntry {
            symbol_id: String::from("sym_abc123"),
        },
        constraints: sample_constraints(),
        cards: vec![entry_card],
        edges: vec![edge],
        spillover: SliceSpillover {
            truncated: true,
            frontier: vec![
                SpilloverCandidate {
                    symbol_id: String::from("sym_spill_001"),
                    depth: 2,
                },
                SpilloverCandidate {
                    symbol_id: String::from("sym_spill_002"),
                    depth: 3,
                },
            ],
        },
    }
}

/// Builds a success response with edges covering all three resolution
/// scope values.
pub fn sample_multi_resolution_response() -> GraphSliceResponse {
    let entry_card = fixtures::build_card_at_level(DetailLevel::Structure);
    let node_card = fixtures::build_card_at_level(DetailLevel::Minimal);

    let full_edge = SliceEdge {
        edge_version: 1,
        edge_type: SliceEdgeType::Call,
        from: String::from("sym_abc123"),
        to: Some(String::from("sym_def456")),
        to_external: None,
        confidence: 0.92,
        direction: SliceDirection::Out,
        resolution_scope: ResolutionScope::FullSymbolTable,
        provenance: EdgeProvenance {
            source: String::from("lsp_call_hierarchy"),
            details: Some(EdgeProvenanceDetails {
                call_site: CallSiteInfo {
                    uri: String::from("file:///src/main.rs"),
                    line: 15,
                    column: 8,
                },
            }),
        },
    };

    let partial_edge = SliceEdge {
        edge_version: 1,
        edge_type: SliceEdgeType::Import,
        from: String::from("sym_abc123"),
        to: Some(String::from("sym_ghi789")),
        to_external: None,
        confidence: 0.78,
        direction: SliceDirection::Out,
        resolution_scope: ResolutionScope::PartialSymbolTable,
        provenance: EdgeProvenance {
            source: String::from("tree_sitter_import_pass"),
            details: None,
        },
    };

    let lsp_edge = SliceEdge {
        edge_version: 1,
        edge_type: SliceEdgeType::Config,
        from: String::from("sym_abc123"),
        to_external: Some(ExternalTarget {
            language: String::from("python"),
            name: String::from("settings.MAX_RETRIES"),
        }),
        to: None,
        confidence: 0.55,
        direction: SliceDirection::Out,
        resolution_scope: ResolutionScope::Lsp,
        provenance: EdgeProvenance {
            source: String::from("tree_sitter_config_pass"),
            details: None,
        },
    };

    GraphSliceResponse::Success {
        slice_version: 1,
        entry: SliceEntry {
            symbol_id: String::from("sym_abc123"),
        },
        constraints: sample_constraints(),
        cards: vec![entry_card, node_card],
        edges: vec![full_edge, partial_edge, lsp_edge],
        spillover: SliceSpillover::empty(),
    }
}

/// Builds a refusal response for the given reason.
pub fn sample_refusal(reason: SliceRefusalReason) -> GraphSliceResponse {
    let message = match &reason {
        SliceRefusalReason::NoSymbolAtPosition => {
            String::from("no symbol found at the requested position")
        }
        SliceRefusalReason::PositionOutOfRange => {
            String::from("position is outside the file bounds")
        }
        SliceRefusalReason::UnsupportedLanguage => {
            String::from("the requested language is not supported")
        }
        SliceRefusalReason::NotYetImplemented => {
            String::from("observe graph-slice: graph-slice traversal is not yet implemented")
        }
        SliceRefusalReason::BackendUnavailable => {
            String::from("the required backend is not available")
        }
    };
    GraphSliceResponse::Refusal {
        refusal: SliceRefusal { reason, message },
    }
}

/// Canonical constraints echoed in a default-budget response.
pub fn sample_constraints() -> SliceConstraints {
    SliceConstraints {
        depth: 2,
        direction: SliceDirection::Both,
        edge_types: SliceEdgeType::all().to_vec(),
        min_confidence: 0.5,
        budget: SliceBudget::default(),
        entry_detail: DetailLevel::Structure,
        node_detail: DetailLevel::Minimal,
    }
}

/// A resolved call edge from the entry symbol to a known target.
pub fn sample_resolved_call_edge() -> SliceEdge {
    SliceEdge {
        edge_version: 1,
        edge_type: SliceEdgeType::Call,
        from: String::from("sym_abc123"),
        to: Some(String::from("sym_def456")),
        to_external: None,
        confidence: 0.92,
        direction: SliceDirection::Out,
        resolution_scope: ResolutionScope::FullSymbolTable,
        provenance: EdgeProvenance {
            source: String::from("lsp_call_hierarchy"),
            details: Some(EdgeProvenanceDetails {
                call_site: CallSiteInfo {
                    uri: String::from("file:///src/main.rs"),
                    line: 15,
                    column: 8,
                },
            }),
        },
    }
}

/// An unresolved call edge with an external target.
pub fn sample_external_edge() -> SliceEdge {
    SliceEdge {
        edge_version: 1,
        edge_type: SliceEdgeType::Call,
        from: String::from("sym_abc123"),
        to: None,
        to_external: Some(ExternalTarget {
            language: String::from("python"),
            name: String::from("requests.get"),
        }),
        confidence: 0.35,
        direction: SliceDirection::Out,
        resolution_scope: ResolutionScope::PartialSymbolTable,
        provenance: EdgeProvenance {
            source: String::from("tree_sitter_heuristic"),
            details: None,
        },
    }
}
