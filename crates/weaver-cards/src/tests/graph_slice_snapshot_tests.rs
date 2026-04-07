//! Insta snapshot tests that lock down the JSON shapes of graph-slice
//! requests, responses, edges, and spillover metadata.
//!
//! Each test serializes a fixture to pretty-printed JSON and asserts
//! that the output matches a stored snapshot file.

use insta::assert_snapshot;
use rstest::rstest;

use super::graph_slice_fixtures;
use crate::graph_slice::{ResolutionScope, SliceRefusalReason};
use crate::{
    GraphSliceRequest, GraphSliceResponse, SliceBudget, SliceDirection, SliceEdgeType,
    SliceSpillover,
};

// -----------------------------------------------------------------------
// Request snapshots
// -----------------------------------------------------------------------

#[test]
fn snapshot_graph_slice_request_defaults() {
    let args: Vec<String> = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
    ];
    let request = GraphSliceRequest::parse(&args).expect("valid request");
    let json = serde_json::to_string_pretty(&request).expect("serialize");
    assert_snapshot!(json);
}

#[test]
fn snapshot_graph_slice_request_all_flags() {
    let args: Vec<String> = vec![
        String::from("--uri"),
        String::from("file:///src/lib.rs"),
        String::from("--position"),
        String::from("42:17"),
        String::from("--depth"),
        String::from("3"),
        String::from("--direction"),
        String::from("out"),
        String::from("--edge-types"),
        String::from("call,import"),
        String::from("--min-confidence"),
        String::from("0.8"),
        String::from("--max-cards"),
        String::from("10"),
        String::from("--max-edges"),
        String::from("50"),
        String::from("--max-estimated-tokens"),
        String::from("2000"),
        String::from("--entry-detail"),
        String::from("semantic"),
        String::from("--node-detail"),
        String::from("signature"),
    ];
    let request = GraphSliceRequest::parse(&args).expect("valid request");
    let json = serde_json::to_string_pretty(&request).expect("serialize");
    assert_snapshot!(json);
}

// -----------------------------------------------------------------------
// Response snapshots
// -----------------------------------------------------------------------

#[test]
fn snapshot_graph_slice_success_default_budget() {
    let response = graph_slice_fixtures::sample_success_response();
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(json);
}

#[test]
fn snapshot_graph_slice_truncated_with_spillover() {
    let response = graph_slice_fixtures::sample_truncated_response();
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(json);
}

#[test]
fn snapshot_graph_slice_multi_resolution_scopes() {
    let response = graph_slice_fixtures::sample_multi_resolution_response();
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(json);
}

#[test]
fn snapshot_graph_slice_refusal_not_implemented() {
    let response = GraphSliceResponse::not_yet_implemented();
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(json);
}

#[rstest]
#[case::no_symbol(
    "graph_slice_refusal_no_symbol",
    SliceRefusalReason::NoSymbolAtPosition
)]
#[case::unsupported_language(
    "graph_slice_refusal_unsupported_language",
    SliceRefusalReason::UnsupportedLanguage
)]
#[case::position_out_of_range(
    "graph_slice_refusal_position_out_of_range",
    SliceRefusalReason::PositionOutOfRange
)]
#[case::backend_unavailable(
    "graph_slice_refusal_backend_unavailable",
    SliceRefusalReason::BackendUnavailable
)]
fn snapshot_graph_slice_refusal_variants(
    #[case] snapshot_name: &str,
    #[case] reason: SliceRefusalReason,
) {
    let response = graph_slice_fixtures::sample_refusal(reason);
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(snapshot_name, json);
}

// -----------------------------------------------------------------------
// Edge snapshots
// -----------------------------------------------------------------------

#[test]
fn snapshot_resolved_call_edge() {
    let edge = graph_slice_fixtures::sample_resolved_call_edge();
    let json = serde_json::to_string_pretty(&edge).expect("serialize");
    assert_snapshot!(json);
}

#[test]
fn snapshot_external_target_edge() {
    let edge = graph_slice_fixtures::sample_external_edge();
    let json = serde_json::to_string_pretty(&edge).expect("serialize");
    assert_snapshot!(json);
}

// -----------------------------------------------------------------------
// Spillover snapshots
// -----------------------------------------------------------------------

#[test]
fn snapshot_spillover_empty() {
    let spillover = SliceSpillover::empty();
    let json = serde_json::to_string_pretty(&spillover).expect("serialize");
    assert_snapshot!(json);
}

#[test]
fn snapshot_spillover_with_frontier() {
    let spillover = SliceSpillover {
        truncated: true,
        frontier: vec![
            crate::graph_slice::SpilloverCandidate {
                symbol_id: String::from("sym_spill_001"),
                depth: 2,
            },
            crate::graph_slice::SpilloverCandidate {
                symbol_id: String::from("sym_spill_002"),
                depth: 3,
            },
        ],
    };
    let json = serde_json::to_string_pretty(&spillover).expect("serialize");
    assert_snapshot!(json);
}

// -----------------------------------------------------------------------
// Component unit tests
// -----------------------------------------------------------------------

#[test]
fn resolution_scope_serializes_as_snake_case() {
    let full = serde_json::to_string(&ResolutionScope::FullSymbolTable).expect("serialize");
    assert_eq!(full, "\"full_symbol_table\"");

    let partial = serde_json::to_string(&ResolutionScope::PartialSymbolTable).expect("serialize");
    assert_eq!(partial, "\"partial_symbol_table\"");

    let lsp = serde_json::to_string(&ResolutionScope::Lsp).expect("serialize");
    assert_eq!(lsp, "\"lsp\"");
}

#[test]
fn slice_direction_serializes_as_snake_case() {
    assert_eq!(
        serde_json::to_string(&SliceDirection::In).expect("serialize"),
        "\"in\""
    );
    assert_eq!(
        serde_json::to_string(&SliceDirection::Out).expect("serialize"),
        "\"out\""
    );
    assert_eq!(
        serde_json::to_string(&SliceDirection::Both).expect("serialize"),
        "\"both\""
    );
}

#[test]
fn slice_edge_type_serializes_as_snake_case() {
    assert_eq!(
        serde_json::to_string(&SliceEdgeType::Call).expect("serialize"),
        "\"call\""
    );
    assert_eq!(
        serde_json::to_string(&SliceEdgeType::Import).expect("serialize"),
        "\"import\""
    );
    assert_eq!(
        serde_json::to_string(&SliceEdgeType::Config).expect("serialize"),
        "\"config\""
    );
}

#[test]
fn budget_default_values() {
    let budget = SliceBudget::default();
    assert_eq!(budget.max_cards(), 30);
    assert_eq!(budget.max_edges(), 200);
    assert_eq!(budget.max_estimated_tokens(), 4000);
}

#[test]
fn spillover_empty_is_not_truncated() {
    let spillover = SliceSpillover::empty();
    assert!(!spillover.truncated);
    assert!(spillover.frontier.is_empty());
}
