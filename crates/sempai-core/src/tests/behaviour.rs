//! Behaviour-driven tests for `sempai_core` types.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::test_support::{QuotedString, parse_byte_range, parse_line_range};
use crate::{Diagnostic, DiagnosticCode, DiagnosticReport, Language, Span};

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
    let (start_byte, end_byte) = parse_byte_range(byte_range.as_str()).expect("valid byte range");
    let (start_lc, end_lc) = parse_line_range(line_range.as_str()).expect("valid line range");
    world.span = Some(Span::new(start_byte, end_byte, start_lc, end_lc));
}

#[given("language {name}")]
fn given_language(world: &mut TestWorld, name: QuotedString) {
    world.language = Some(name.as_str().parse().expect("valid language name"));
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
    let parsed: serde_json::Value =
        serde_json::from_str(json).expect("JSON output should be valid");
    let actual = parsed.get(key.as_str()).unwrap_or_else(|| {
        panic!(
            "expected JSON to contain key '{}', got: {json}",
            key.as_str()
        )
    });
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(value.as_str().to_owned()));
    assert_eq!(
        actual,
        &expected,
        "expected JSON key '{}' to have value {expected:?}, got {actual:?}",
        key.as_str()
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
