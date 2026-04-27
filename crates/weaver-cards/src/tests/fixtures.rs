//! Shared fixture builders for `weaver-cards` test suites.
//!
//! These helpers provide canonical sample data for symbol card types.
//! Test modules that need richer or variant data can build on top of
//! these foundations.

use crate::{
    BranchInfo,
    CardLanguage,
    CardSymbolKind,
    DepsInfo,
    DetailLevel,
    DocInfo,
    LocalInfo,
    LspInfo,
    MetricsInfo,
    ParamInfo,
    Provenance,
    SignatureInfo,
    SourcePosition,
    SourceRange,
    StructureInfo,
    SymbolCard,
    SymbolIdentity,
    SymbolRef,
};

/// Identity block with an optional container name.
pub fn sample_identity(container: Option<&str>) -> SymbolIdentity {
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

/// Provenance block with optional extra source entries.
pub fn sample_provenance(extra_sources: &[&str]) -> Provenance {
    let mut sources = vec![String::from("tree_sitter")];
    sources.extend(extra_sources.iter().map(|s| String::from(*s)));
    Provenance {
        extracted_at: String::from("2026-03-03T12:34:56Z"),
        sources,
    }
}

/// Canonical function signature.
pub fn sample_signature() -> SignatureInfo {
    SignatureInfo {
        display: String::from("fn process_request(req: &Request) -> Response"),
        params: vec![ParamInfo {
            name: String::from("req"),
            type_annotation: String::from("&Request"),
        }],
        returns: String::from("Response"),
    }
}

/// Documentation block with the given docstring text.
pub fn sample_doc(docstring: &str) -> DocInfo {
    DocInfo {
        docstring: String::from(docstring),
        summary: String::from(docstring),
        source: String::from("tree_sitter"),
    }
}

/// Structure block with one local and the given branches.
pub fn sample_structure(branches: Vec<BranchInfo>) -> StructureInfo {
    StructureInfo {
        locals: vec![LocalInfo {
            name: String::from("result"),
            kind: String::from("variable"),
            decl_line: 15,
        }],
        branches,
    }
}

/// LSP semantic data block.
pub fn sample_lsp() -> LspInfo {
    LspInfo {
        hover: String::from("fn process_request(req: &Request) -> Response"),
        type_info: String::from("Callable[[Request], Response]"),
        deprecated: false,
        source: String::from("lsp_hover"),
    }
}

/// Metrics block with optional fan-in/fan-out.
pub fn sample_metrics(fan_in: Option<u32>, fan_out: Option<u32>) -> MetricsInfo {
    MetricsInfo {
        lines: 33,
        cyclomatic: 5,
        fan_in,
        fan_out,
    }
}

/// Dependency edges block.
pub fn sample_deps(calls: Vec<String>, config: Vec<String>) -> DepsInfo {
    DepsInfo {
        calls,
        imports: vec![String::from("mod_std_io")],
        config,
    }
}

/// Build a [`SymbolCard`] at the given detail level using shared fixture
/// data. Uses the `container` identity variant and the short docstring.
pub fn build_card_at_level(level: DetailLevel) -> SymbolCard {
    let base = || SymbolCard {
        card_version: 1,
        symbol: sample_identity(Some("handlers")),
        signature: None,
        doc: None,
        attachments: None,
        structure: None,
        lsp: None,
        metrics: None,
        deps: None,
        interstitial: None,
        provenance: sample_provenance(&[]),
        etag: None,
    };
    let branches_one = || {
        vec![BranchInfo {
            kind: String::from("if"),
            line: 18,
        }]
    };
    match level {
        DetailLevel::Minimal => base(),
        DetailLevel::Signature => SymbolCard {
            signature: Some(sample_signature()),
            ..base()
        },
        DetailLevel::Structure => SymbolCard {
            signature: Some(sample_signature()),
            doc: Some(sample_doc("Processes a request.")),
            structure: Some(sample_structure(branches_one())),
            metrics: Some(sample_metrics(None, None)),
            ..base()
        },
        DetailLevel::Semantic => SymbolCard {
            signature: Some(sample_signature()),
            doc: Some(sample_doc("Processes a request.")),
            structure: Some(sample_structure(branches_one())),
            lsp: Some(sample_lsp()),
            metrics: Some(sample_metrics(None, None)),
            ..base()
        },
        DetailLevel::Full => SymbolCard {
            signature: Some(sample_signature()),
            doc: Some(sample_doc("Processes a request.")),
            structure: Some(sample_structure(branches_one())),
            lsp: Some(sample_lsp()),
            metrics: Some(sample_metrics(Some(12), Some(3))),
            deps: Some(sample_deps(vec![String::from("sym_def456")], vec![])),
            ..base()
        },
    }
}
