//! Insta snapshot tests that lock down the JSON shapes of symbol cards
//! and `get-card` responses.
//!
//! Each test serializes a fixture to pretty-printed JSON and asserts that
//! the output matches a stored snapshot file. These snapshots guarantee
//! byte-identical output for unchanged inputs across runs.

use insta::assert_snapshot;
use rstest::rstest;

use super::fixtures;
use crate::{BranchInfo, CardRefusal, DetailLevel, GetCardResponse, RefusalReason, SymbolCard};

// ---------------------------------------------------------------------------
// Fixture builders (snapshot-specific wrappers around shared fixtures)
// ---------------------------------------------------------------------------

fn minimal_identity() -> crate::SymbolIdentity {
    fixtures::sample_identity(None)
}

fn identity_with_container() -> crate::SymbolIdentity {
    fixtures::sample_identity(Some("handlers"))
}

fn sample_structure() -> crate::StructureInfo {
    fixtures::sample_structure(vec![
        BranchInfo {
            kind: String::from("if"),
            line: 18,
        },
        BranchInfo {
            kind: String::from("match"),
            line: 25,
        },
    ])
}

fn sample_deps() -> crate::DepsInfo {
    fixtures::sample_deps(
        vec![String::from("sym_def456"), String::from("sym_ghi789")],
        vec![String::from("cfg_max_retries")],
    )
}

// ---------------------------------------------------------------------------
// Card snapshots
// ---------------------------------------------------------------------------

#[test]
fn snapshot_minimal_card() {
    let card = SymbolCard {
        card_version: 1,
        symbol: minimal_identity(),
        signature: None,
        doc: None,
        attachments: None,
        structure: None,
        lsp: None,
        metrics: None,
        deps: None,
        interstitial: None,
        provenance: fixtures::sample_provenance(&[]),
        etag: None,
    };
    let json = serde_json::to_string_pretty(&card).expect("serialize");
    assert_snapshot!(json);
}

fn structure_card() -> SymbolCard {
    let docstring = "Processes an incoming request and returns a response.";
    SymbolCard {
        card_version: 1,
        symbol: identity_with_container(),
        signature: Some(fixtures::sample_signature()),
        doc: Some(fixtures::sample_doc(docstring)),
        attachments: None,
        structure: Some(sample_structure()),
        lsp: None,
        metrics: Some(fixtures::sample_metrics(None, None)),
        deps: None,
        interstitial: None,
        provenance: fixtures::sample_provenance(&[]),
        etag: None,
    }
}

#[test]
fn snapshot_structure_card() {
    let json = serde_json::to_string_pretty(&structure_card()).expect("serialize");
    assert_snapshot!(json);
}

#[test]
fn snapshot_full_card() {
    let card = SymbolCard {
        lsp: Some(fixtures::sample_lsp()),
        metrics: Some(fixtures::sample_metrics(Some(12), Some(3))),
        deps: Some(sample_deps()),
        provenance: fixtures::sample_provenance(&["lsp_hover"]),
        etag: Some(String::from("etag_abc123")),
        ..structure_card()
    };
    let json = serde_json::to_string_pretty(&card).expect("serialize");
    assert_snapshot!(json);
}

// ---------------------------------------------------------------------------
// Response snapshots
// ---------------------------------------------------------------------------

#[test]
fn snapshot_refusal_not_implemented() {
    let response = GetCardResponse::not_yet_implemented(DetailLevel::Structure);
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(json);
}

fn refusal_response(reason: RefusalReason, message: &str, detail: DetailLevel) -> GetCardResponse {
    GetCardResponse::Refusal {
        refusal: CardRefusal {
            reason,
            message: String::from(message),
            requested_detail: detail,
        },
    }
}

#[rstest]
#[case::no_symbol(
    "refusal_no_symbol",
    RefusalReason::NoSymbolAtPosition,
    "no symbol found at the requested position",
    DetailLevel::Structure
)]
#[case::unsupported_language(
    "refusal_unsupported_language",
    RefusalReason::UnsupportedLanguage,
    "the requested language is not supported",
    DetailLevel::Structure
)]
#[case::position_out_of_range(
    "refusal_position_out_of_range",
    RefusalReason::PositionOutOfRange,
    "the requested position is outside the file bounds",
    DetailLevel::Structure
)]
#[case::backend_unavailable(
    "refusal_backend_unavailable",
    RefusalReason::BackendUnavailable,
    "the required backend is not available",
    DetailLevel::Semantic
)]
fn snapshot_refusal_variants(
    #[case] snapshot_name: &str,
    #[case] reason: RefusalReason,
    #[case] message: &str,
    #[case] requested_detail: DetailLevel,
) {
    let response = refusal_response(reason, message, requested_detail);
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(snapshot_name, json);
}

#[test]
fn snapshot_success_response() {
    let response = GetCardResponse::Success {
        card: Box::new(structure_card()),
    };
    let json = serde_json::to_string_pretty(&response).expect("serialize");
    assert_snapshot!(json);
}
