//! Behaviour-driven tests for `sempai_core` types.

use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::{Diagnostic, DiagnosticCode, DiagnosticReport, Language, LineCol, Span};

// ---------------------------------------------------------------------------
// Typed wrappers for Gherkin step parameters
// ---------------------------------------------------------------------------

/// A quoted string value from a Gherkin feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim_matches('"').to_owned()))
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
    span: Option<Span>,
    language: Option<Language>,
    round_tripped_language: Option<Language>,
    report: Option<DiagnosticReport>,
    formatted_output: Option<String>,
    json_output: Option<String>,
}

#[fixture]
fn world() -> TestWorld {
    TestWorld::default()
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a span from bytes {byte_range} at lines {line_range}")]
fn given_span(world: &mut TestWorld, byte_range: QuotedString, line_range: QuotedString) {
    // Parse byte range: "10..42"
    let bytes: Vec<u32> = byte_range
        .as_str()
        .split("..")
        .map(|s| s.parse().expect("valid byte offset"))
        .collect();
    let start_byte = *bytes.first().expect("start byte");
    let end_byte = *bytes.get(1).expect("end byte");

    // Parse line range: "2:0..4:0"
    let positions: Vec<&str> = line_range.as_str().split("..").collect();
    let start_str = positions.first().expect("start position");
    let end_str = positions.get(1).expect("end position");
    let start: Vec<u32> = start_str
        .split(':')
        .map(|s| s.parse().expect("valid line:col"))
        .collect();
    let end: Vec<u32> = end_str
        .split(':')
        .map(|s| s.parse().expect("valid line:col"))
        .collect();

    world.span = Some(Span::new(
        start_byte,
        end_byte,
        LineCol::new(
            *start.first().expect("start line"),
            *start.get(1).expect("start col"),
        ),
        LineCol::new(
            *end.first().expect("end line"),
            *end.get(1).expect("end col"),
        ),
    ));
}

#[given("language {name}")]
fn given_language(world: &mut TestWorld, name: QuotedString) {
    let lang = match name.as_str() {
        "rust" => Language::Rust,
        "python" => Language::Python,
        "type_script" => Language::TypeScript,
        "go" => Language::Go,
        "hcl" => Language::Hcl,
        other => panic!("unknown language: {other}"),
    };
    world.language = Some(lang);
}

#[given("a diagnostic with code {code} and message {message}")]
fn given_diagnostic(world: &mut TestWorld, code: QuotedString, message: QuotedString) {
    let diag_code = match code.as_str() {
        "E_SEMPAI_YAML_PARSE" => DiagnosticCode::ESempaiYamlParse,
        "E_SEMPAI_DSL_PARSE" => DiagnosticCode::ESempaiDslParse,
        other => panic!("unsupported diagnostic code: {other}"),
    };
    let report = DiagnosticReport::new(vec![Diagnostic::new(
        diag_code,
        message.as_str().to_owned(),
        None,
        vec![],
    )]);
    world.report = Some(report);
}

#[given("a not-implemented report for feature {feature}")]
fn given_not_implemented_report(world: &mut TestWorld, feature: QuotedString) {
    world.report = Some(DiagnosticReport::not_implemented(feature.as_str()));
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the span is serialized to JSON")]
fn when_serialize_span(world: &mut TestWorld) {
    let span = world.span.as_ref().expect("span should be set");
    world.json_output = Some(serde_json::to_string(span).expect("serialize span"));
}

#[when("the language is serialized and deserialized")]
fn when_language_round_trip(world: &mut TestWorld) {
    let lang = world.language.expect("language should be set");
    let json = serde_json::to_string(&lang).expect("serialize language");
    let deserialized: Language = serde_json::from_str(&json).expect("deserialize language");
    world.round_tripped_language = Some(deserialized);
}

#[when("the diagnostic report is formatted")]
fn when_format_report(world: &mut TestWorld) {
    let report = world.report.as_ref().expect("report should be set");
    world.formatted_output = Some(format!("{report}"));
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the JSON contains key {key} with value {value}")]
fn then_json_contains(world: &mut TestWorld, key: QuotedString, value: QuotedString) {
    let json = world.json_output.as_ref().expect("JSON should be set");
    let expected = format!("\"{}\":{}", key.as_str(), value.as_str());
    assert!(
        json.contains(&expected),
        "expected JSON to contain '{expected}', got: {json}"
    );
}

#[then("the round-tripped language equals the original")]
fn then_language_round_trip_equals(world: &mut TestWorld) {
    let original = world.language.expect("original language should be set");
    let round_tripped = world
        .round_tripped_language
        .expect("round-tripped language should be set");
    assert_eq!(original, round_tripped);
}

#[then("the formatted output contains {snippet}")]
fn then_formatted_contains(world: &mut TestWorld, snippet: QuotedString) {
    let output = world
        .formatted_output
        .as_ref()
        .expect("formatted output should be set");
    assert!(
        output.contains(snippet.as_str()),
        "expected output to contain '{}', got: {}",
        snippet.as_str(),
        output
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_core.feature")]
fn sempai_core_behaviour(world: TestWorld) {
    let _ = world;
}
