//! Insta snapshot tests that lock down the JSON shapes of symbol cards
//! and `get-card` responses.
//!
//! Each test serialises a fixture to pretty-printed JSON and asserts that
//! the output matches a stored snapshot file. These snapshots guarantee
//! byte-identical output for unchanged inputs across runs.

use insta::assert_snapshot;

use crate::{
    BranchInfo, CardLanguage, CardRefusal, CardSymbolKind, DepsInfo, DetailLevel, DocInfo,
    GetCardResponse, LocalInfo, LspInfo, MetricsInfo, ParamInfo, Provenance, RefusalReason,
    SignatureInfo, SourcePosition, SourceRange, StructureInfo, SymbolCard, SymbolIdentity,
    SymbolRef,
};

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn minimal_identity() -> SymbolIdentity {
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
            container: None,
        },
    }
}

fn identity_with_container() -> SymbolIdentity {
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
            container: Some(String::from("handlers")),
        },
    }
}

fn sample_provenance() -> Provenance {
    Provenance {
        extracted_at: String::from("2026-03-03T12:34:56Z"),
        sources: vec![String::from("tree_sitter")],
    }
}

fn full_provenance() -> Provenance {
    Provenance {
        extracted_at: String::from("2026-03-03T12:34:56Z"),
        sources: vec![String::from("tree_sitter"), String::from("lsp_hover")],
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

fn sample_metrics_structure() -> MetricsInfo {
    MetricsInfo {
        lines: 33,
        cyclomatic: 5,
        fan_in: None,
        fan_out: None,
    }
}

fn sample_metrics_full() -> MetricsInfo {
    MetricsInfo {
        lines: 33,
        cyclomatic: 5,
        fan_in: Some(12),
        fan_out: Some(3),
    }
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
        provenance: sample_provenance(),
        etag: None,
    };
    let json = serde_json::to_string_pretty(&card).expect("serialise");
    assert_snapshot!(json);
}

#[test]
fn snapshot_structure_card() {
    let card = SymbolCard {
        card_version: 1,
        symbol: identity_with_container(),
        signature: Some(sample_signature()),
        doc: Some(sample_doc()),
        structure: Some(sample_structure()),
        lsp: None,
        metrics: Some(sample_metrics_structure()),
        deps: None,
        provenance: sample_provenance(),
        etag: None,
    };
    let json = serde_json::to_string_pretty(&card).expect("serialise");
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
        provenance: full_provenance(),
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

#[test]
fn snapshot_refusal_no_symbol() {
    let response = GetCardResponse::Refusal {
        refusal: CardRefusal {
            reason: RefusalReason::NoSymbolAtPosition,
            message: String::from("no symbol found at the requested position"),
            requested_detail: DetailLevel::Structure,
        },
    };
    let json = serde_json::to_string_pretty(&response).expect("serialise");
    assert_snapshot!(json);
}

#[test]
fn snapshot_success_response() {
    let card = SymbolCard {
        card_version: 1,
        symbol: identity_with_container(),
        signature: Some(sample_signature()),
        doc: Some(sample_doc()),
        structure: Some(sample_structure()),
        lsp: None,
        metrics: Some(sample_metrics_structure()),
        deps: None,
        provenance: sample_provenance(),
        etag: None,
    };
    let response = GetCardResponse::Success {
        card: Box::new(card),
    };
    let json = serde_json::to_string_pretty(&response).expect("serialise");
    assert_snapshot!(json);
}
