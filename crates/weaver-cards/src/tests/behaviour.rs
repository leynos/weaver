//! Behaviour-driven tests for `weaver-cards` schema contracts.

use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::{
    BranchInfo, CardLanguage, CardRefusal, CardSymbolKind, DepsInfo, DetailLevel, DocInfo,
    GetCardRequest, GetCardResponse, LocalInfo, LspInfo, MetricsInfo, ParamInfo, Provenance,
    RefusalReason, SignatureInfo, SourcePosition, SourceRange, StructureInfo, SymbolCard,
    SymbolIdentity, SymbolRef,
};

// ---------------------------------------------------------------------------
// QuotedString helper (same pattern as sempai_core::test_support)
// ---------------------------------------------------------------------------

/// Error returned when a string is not wrapped in balanced double-quotes.
#[derive(Debug, thiserror::Error)]
#[error("expected a double-quoted string, got: {0}")]
struct QuotedStringParseError(String);

/// A quoted string value from a Gherkin feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = QuotedStringParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| QuotedStringParseError(s.to_owned()))?;
        Ok(Self(value.to_owned()))
    }
}

impl QuotedString {
    fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TestWorld {
    card: Option<SymbolCard>,
    response: Option<GetCardResponse>,
    request: Option<GetCardRequest>,
    json_output: Option<String>,
}

#[fixture]
fn world() -> TestWorld {
    TestWorld::default()
}

// ---------------------------------------------------------------------------
// Fixture builders
// ---------------------------------------------------------------------------

fn sample_identity() -> SymbolIdentity {
    SymbolIdentity {
        symbol_id: String::from("sym_test"),
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

fn parse_detail_level(raw: &str) -> Result<DetailLevel, String> {
    raw.parse()
        .map_err(|e: crate::DetailLevelParseError| e.to_string())
}

fn parse_refusal_reason(raw: &str) -> Result<RefusalReason, String> {
    match raw {
        "no_symbol_at_position" => Ok(RefusalReason::NoSymbolAtPosition),
        "unsupported_language" => Ok(RefusalReason::UnsupportedLanguage),
        "not_yet_implemented" => Ok(RefusalReason::NotYetImplemented),
        "backend_unavailable" => Ok(RefusalReason::BackendUnavailable),
        other => Err(format!("unknown refusal reason: {other}")),
    }
}

fn build_card(detail: &str) -> Result<SymbolCard, String> {
    let level = parse_detail_level(detail)?;
    Ok(build_card_at_level(level))
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
        docstring: String::from("Processes a request."),
        summary: String::from("Processes a request."),
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
        branches: vec![BranchInfo {
            kind: String::from("if"),
            line: 18,
        }],
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

fn sample_metrics(fan_in: Option<u32>, fan_out: Option<u32>) -> MetricsInfo {
    MetricsInfo {
        lines: 33,
        cyclomatic: 5,
        fan_in,
        fan_out,
    }
}

fn sample_deps() -> DepsInfo {
    DepsInfo {
        calls: vec![String::from("sym_def456")],
        imports: vec![String::from("mod_std_io")],
        config: vec![],
    }
}

fn build_card_at_level(level: DetailLevel) -> SymbolCard {
    let base = || SymbolCard {
        card_version: 1,
        symbol: sample_identity(),
        signature: None,
        doc: None,
        attachments: None,
        structure: None,
        lsp: None,
        metrics: None,
        deps: None,
        interstitial: None,
        provenance: sample_provenance(),
        etag: None,
    };
    match level {
        DetailLevel::Minimal => base(),
        DetailLevel::Signature => SymbolCard {
            signature: Some(sample_signature()),
            ..base()
        },
        DetailLevel::Structure => SymbolCard {
            signature: Some(sample_signature()),
            doc: Some(sample_doc()),
            structure: Some(sample_structure()),
            metrics: Some(sample_metrics(None, None)),
            ..base()
        },
        DetailLevel::Semantic => SymbolCard {
            signature: Some(sample_signature()),
            doc: Some(sample_doc()),
            structure: Some(sample_structure()),
            lsp: Some(sample_lsp()),
            metrics: Some(sample_metrics(None, None)),
            ..base()
        },
        DetailLevel::Full => SymbolCard {
            signature: Some(sample_signature()),
            doc: Some(sample_doc()),
            structure: Some(sample_structure()),
            lsp: Some(sample_lsp()),
            metrics: Some(sample_metrics(Some(12), Some(3))),
            deps: Some(sample_deps()),
            ..base()
        },
    }
}

fn build_refusal_response(reason: RefusalReason, detail: DetailLevel) -> GetCardResponse {
    let message = match reason {
        RefusalReason::NotYetImplemented => {
            String::from("observe get-card: Tree-sitter card extraction is not yet implemented")
        }
        RefusalReason::NoSymbolAtPosition => {
            String::from("no symbol found at the requested position")
        }
        RefusalReason::UnsupportedLanguage => {
            String::from("the requested language is not supported")
        }
        RefusalReason::BackendUnavailable => String::from("the required backend is not available"),
    };
    GetCardResponse::Refusal {
        refusal: CardRefusal {
            reason,
            message,
            requested_detail: detail,
        },
    }
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a symbol card at {detail} detail level")]
fn given_card_at_detail(world: &mut TestWorld, detail: QuotedString) {
    world.card = Some(build_card(detail.as_str()).expect("valid detail level in feature file"));
}

#[given("a refusal response with reason {reason}")]
fn given_refusal_response(world: &mut TestWorld, reason: QuotedString) {
    let parsed_reason =
        parse_refusal_reason(reason.as_str()).expect("valid refusal reason in feature file");
    let detail = DetailLevel::Structure;
    world.response = Some(build_refusal_response(parsed_reason, detail));
}

#[given("a success response with a {detail} detail card")]
fn given_success_response(world: &mut TestWorld, detail: QuotedString) {
    let card = build_card(detail.as_str()).expect("valid detail level in feature file");
    world.response = Some(GetCardResponse::Success {
        card: Box::new(card),
    });
}

#[given("a get-card request with no detail flag")]
fn given_request_no_detail(world: &mut TestWorld) {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
    ];
    world.request = Some(GetCardRequest::parse(&args).expect("valid request"));
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the card is serialized to JSON")]
fn when_card_serialized(world: &mut TestWorld) {
    if let Some(c) = world.card.as_ref() {
        world.json_output = Some(serde_json::to_string(c).expect("serialize card"));
    } else {
        panic!("card must be set for this step");
    }
}

#[when("the response is serialized to JSON")]
fn when_response_serialized(world: &mut TestWorld) {
    let response = world.response.as_ref().expect("response should be set");
    world.json_output = Some(serde_json::to_string(response).expect("serialize response"));
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the JSON contains a {field} field")]
fn then_json_contains_field(world: &mut TestWorld, field: QuotedString) {
    let json = world.json_output.as_ref().expect("JSON should be set");
    let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
    let pointer = format!("/{}", field.as_str().replace('.', "/"));
    assert!(
        parsed.pointer(&pointer).is_some(),
        "expected JSON to contain field '{}', got: {json}",
        field.as_str()
    );
}

#[then("the JSON does not contain a {field} field")]
fn then_json_does_not_contain_field(world: &mut TestWorld, field: QuotedString) {
    let json = world.json_output.as_ref().expect("JSON should be set");
    let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
    let pointer = format!("/{}", field.as_str().replace('.', "/"));
    assert!(
        parsed.pointer(&pointer).is_none(),
        "expected JSON NOT to contain field '{}', got: {json}",
        field.as_str()
    );
}

#[then("the JSON field {key} has value {value}")]
fn then_json_field_has_value(world: &mut TestWorld, key: QuotedString, value: QuotedString) {
    let json = world.json_output.as_ref().expect("JSON should be set");
    let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
    let pointer = format!("/{}", key.as_str().replace('.', "/"));
    let actual = parsed
        .pointer(&pointer)
        .unwrap_or_else(|| panic!("expected JSON to contain key '{}'", key.as_str()));
    let expected = serde_json::Value::String(String::from(value.as_str()));
    assert_eq!(
        actual,
        &expected,
        "expected '{}' = {:?}, got {:?}",
        key.as_str(),
        expected,
        actual
    );
}

#[then("the detail level is {level}")]
fn then_detail_level_is(world: &mut TestWorld, level: QuotedString) {
    let request = world.request.as_ref().expect("request should be set");
    let expected = parse_detail_level(level.as_str()).expect("valid detail level in feature file");
    assert_eq!(request.detail, expected);
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/get_card_schema.feature")]
fn get_card_schema_behaviour(world: TestWorld) {
    drop(world);
}
