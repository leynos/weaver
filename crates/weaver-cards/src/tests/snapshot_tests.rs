//! Insta snapshot tests that lock down the JSON shapes of symbol cards
//! and `get-card` responses.
//!
//! Each test serializes a fixture to pretty-printed JSON and asserts that
//! the output matches a stored snapshot file. These snapshots guarantee
//! byte-identical output for unchanged inputs across runs.

use insta::assert_snapshot;
use rstest::rstest;

use crate::{
    BranchInfo, CardLanguage, CardRefusal, CardSymbolKind, DepsInfo, DetailLevel, DocInfo,
    GetCardResponse, LocalInfo, LspInfo, MetricsInfo, ParamInfo, Provenance, RefusalReason,
    SignatureInfo, SourcePosition, SourceRange, StructureInfo, SymbolCard, SymbolIdentity,
    SymbolRef,
};

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn sample_identity(container: Option<&str>) -> SymbolIdentity {
    SymbolIdentity {
        symbol_id: String::from("sym_abc123"),
        symbol_ref: SymbolRef {
            uri: String::from("file:///src/main.rs"),
            range: SourceRange {
                start: SourcePosition {
                    line: 10,
                    column: 0,
                },
                end: SourcePosition {
                    line: 42,
                    column: 1,
                },
            },
            language: CardLanguage::Rust,
            kind: CardSymbolKind::Function,
            name: String::from("process_request"),
            container: container.map(String::from),
        },
    }
}

fn minimal_identity() -> SymbolIdentity {
    sample_identity(None)
}

fn identity_with_container() -> SymbolIdentity {
    sample_identity(Some("handlers"))
}

fn sample_provenance(extra_sources: &[&str]) -> Provenance {
    let mut sources = vec![String::from("tree_sitter")];
    sources.extend(extra_sources.iter().map(|s| String::from(*s)));
    Provenance {
        extracted_at: String::from("2026-03-03T12:34:56Z"),
        sources,
    }
}

fn sample_signature() -> SignatureInfo {
    SignatureInfo {
        display: String::from("fn process_request(req: &Request) -> Response"),
        params: vec![ParamInfo {
            name: String::from("req"),
            type_annotation: String::from("&Request"),
        }],
        returns: String::from("Response"),
    }
}

fn sample_doc() -> DocInfo {
    DocInfo {
        docstring: String::from("Processes an incoming request and returns a response."),
        summary: String::from("Processes an incoming request and returns a response."),
        source: String::from("tree_sitter"),
    }
}

fn sample_structure() -> StructureInfo {
    StructureInfo {
        locals: vec![LocalInfo {
            name: String::from("result"),
            kind: String::from("variable"),
            decl_line: 15,
        }],
        branches: vec![
            BranchInfo {
                kind: String::from("if"),
                line: 18,
            },
            BranchInfo {
                kind: String::from("match"),
                line: 25,
            },
        ],
    }
}

fn sample_metrics(fan_in: Option<u32>, fan_out: Option<u32>) -> MetricsInfo {
    MetricsInfo {
        lines: 33,
        cyclomatic: 5,
        fan_in,
        fan_out,
    }
}

fn sample_metrics_structure() -> MetricsInfo {
    sample_metrics(None, None)
}

fn sample_metrics_full() -> MetricsInfo {
    sample_metrics(Some(12), Some(3))
}

fn sample_lsp() -> LspInfo {
    LspInfo {
        hover: String::from("fn process_request(req: &Request) -> Response"),
        type_info: String::from("Callable[[Request], Response]"),
        deprecated: false,
        source: String::from("lsp_hover"),
    }
}

fn sample_deps() -> DepsInfo {
    DepsInfo {
        calls: vec![String::from("sym_def456"), String::from("sym_ghi789")],
        imports: vec![String::from("mod_std_io")],
        config: vec![String::from("cfg_max_retries")],
    }
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
        structure: None,
        lsp: None,
        metrics: None,
        deps: None,
        provenance: sample_provenance(&[]),
        etag: None,
    };
    let json = serde_json::to_string_pretty(&card).expect("serialise");
    assert_snapshot!(json);
}

fn structure_card() -> SymbolCard {
    SymbolCard {
        card_version: 1,
        symbol: identity_with_container(),
        signature: Some(sample_signature()),
        doc: Some(sample_doc()),
        structure: Some(sample_structure()),
        lsp: None,
        metrics: Some(sample_metrics_structure()),
        deps: None,
        provenance: sample_provenance(&[]),
        etag: None,
    }
}

#[test]
fn snapshot_structure_card() {
    let json = serde_json::to_string_pretty(&structure_card()).expect("serialise");
    assert_snapshot!(json);
}

#[test]
fn snapshot_full_card() {
    let card = SymbolCard {
        card_version: 1,
        symbol: identity_with_container(),
        signature: Some(sample_signature()),
        doc: Some(sample_doc()),
        structure: Some(sample_structure()),
        lsp: Some(sample_lsp()),
        metrics: Some(sample_metrics_full()),
        deps: Some(sample_deps()),
        provenance: sample_provenance(&["lsp_hover"]),
        etag: Some(String::from("etag_abc123")),
    };
    let json = serde_json::to_string_pretty(&card).expect("serialise");
    assert_snapshot!(json);
}

// ---------------------------------------------------------------------------
// Response snapshots
// ---------------------------------------------------------------------------

#[test]
fn snapshot_refusal_not_implemented() {
    let response = GetCardResponse::not_yet_implemented(DetailLevel::Structure);
    let json = serde_json::to_string_pretty(&response).expect("serialise");
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
    let json = serde_json::to_string_pretty(&response).expect("serialise");
    assert_snapshot!(snapshot_name, json);
}

#[test]
fn snapshot_success_response() {
    let response = GetCardResponse::Success {
        card: Box::new(structure_card()),
    };
    let json = serde_json::to_string_pretty(&response).expect("serialise");
    assert_snapshot!(json);
}
