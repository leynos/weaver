//! Behaviour-driven tests for `sempai_core` types.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    DiagnosticCode,
    DiagnosticReport,
    Language,
    SourceSpan,
    Span,
    test_support::{QuotedString, parse_byte_range, parse_line_range},
};

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
    diagnostic_code_payload: Option<String>,
    deserialization_error: Option<String>,
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> TestWorld { TestWorld::default() }

fn parse_diagnostic_code(code: &str) -> DiagnosticCode {
    let json = serde_json::to_string(code).expect("serialise diagnostic code");
    serde_json::from_str(&json).expect(&format!("unrecognised diagnostic code: {code}"))
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

fn build_single_diagnostic_report(
    constructor: fn(DiagnosticCode, String, Option<SourceSpan>, Vec<String>) -> DiagnosticReport,
    code: &str,
    message: &str,
) -> DiagnosticReport {
    let diagnostic_code = parse_diagnostic_code(code);
    constructor(diagnostic_code, message.to_owned(), None, vec![])
}

fn given_report_with_constructor(
    world: &mut TestWorld,
    code: &QuotedString,
    message: &QuotedString,
    constructor: fn(DiagnosticCode, String, Option<SourceSpan>, Vec<String>) -> DiagnosticReport,
) {
    world.report = Some(build_single_diagnostic_report(
        constructor,
        code.as_str(),
        message.as_str(),
    ));
}

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
    given_report_with_constructor(world, &code, &message, DiagnosticReport::single_error);
}

#[given("a parser diagnostic with code {code} and message {message}")]
fn given_parser_diagnostic(world: &mut TestWorld, code: QuotedString, message: QuotedString) {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::parser_error);
}

#[given("a validator diagnostic with code {code} and message {message}")]
fn given_validator_diagnostic(world: &mut TestWorld, code: QuotedString, message: QuotedString) {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::validation_error);
}

#[given("a not-implemented report for feature {feature}")]
fn given_not_implemented_report(world: &mut TestWorld, feature: QuotedString) {
    world.report = Some(DiagnosticReport::not_implemented(feature.as_str()));
}

#[given("diagnostic code payload {code}")]
fn given_diagnostic_code_payload(world: &mut TestWorld, code: QuotedString) {
    world.diagnostic_code_payload =
        Some(serde_json::to_string(code.as_str()).expect("serialise diagnostic code payload"));
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

#[when("the diagnostic report is serialized to JSON")]
fn when_serialize_diagnostic_report(world: &mut TestWorld) {
    let report = world.report.as_ref().expect("report should be set");
    world.json_output = Some(serde_json::to_string(report).expect("serialize report"));
}

#[when("the diagnostic code payload is deserialized")]
fn when_deserialize_diagnostic_code_payload(world: &mut TestWorld) {
    let payload = world
        .diagnostic_code_payload
        .as_ref()
        .expect("diagnostic code payload should be set");
    world.deserialization_error = serde_json::from_str::<DiagnosticCode>(payload)
        .err()
        .map(|e| e.to_string());
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

fn first_diagnostic_object(world: &TestWorld) -> serde_json::Map<String, serde_json::Value> {
    let json = world.json_output.as_ref().expect("JSON should be set");
    let parsed: serde_json::Value =
        serde_json::from_str(json).expect("JSON output should be valid");
    parsed
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(serde_json::Value::as_object)
        .expect("first diagnostic object should exist")
        .clone()
}

fn assert_str_contains(haystack: &str, needle: &str, label: &str) {
    assert!(
        haystack.contains(needle),
        "expected {label} to contain '{needle}', got: {haystack}"
    );
}

#[then("the JSON contains key {key} with value {value}")]
fn then_json_contains(world: &mut TestWorld, key: QuotedString, value: QuotedString) {
    let json = world.json_output.as_ref().expect("JSON should be set");
    let parsed: serde_json::Value =
        serde_json::from_str(json).expect("JSON output should be valid");
    let actual = parsed
        .get(key.as_str())
        .expect(&format!("expected JSON to contain key '{}', got: {json}", key.as_str()));
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(value.as_str().to_owned()));
    assert_eq!(
        actual,
        &expected,
        "expected JSON key '{}' to have value {expected:?}, got {actual:?}",
        key.as_str()
    );
}

#[then("the first diagnostic JSON contains key {key}")]
fn then_first_diagnostic_contains_key(world: &mut TestWorld, key: QuotedString) {
    let first = first_diagnostic_object(world);
    assert!(
        first.contains_key(key.as_str()),
        "expected first diagnostic JSON to contain key '{}', got: {first:?}",
        key.as_str()
    );
}

#[then("the first diagnostic JSON does not contain key {key}")]
fn then_first_diagnostic_does_not_contain_key(world: &mut TestWorld, key: QuotedString) {
    let first = first_diagnostic_object(world);
    assert!(
        !first.contains_key(key.as_str()),
        "expected first diagnostic JSON to not contain key '{}', got: {first:?}",
        key.as_str()
    );
}

#[then("the first diagnostic JSON contains key {key} with value {value}")]
fn then_first_diagnostic_contains_key_with_value(
    world: &mut TestWorld,
    key: QuotedString,
    value: QuotedString,
) {
    let first = first_diagnostic_object(world);
    let actual = first
        .get(key.as_str())
        .expect(&format!("expected first diagnostic JSON to contain key '{}', got: {first:?}", key.as_str()));
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(value.as_str().to_owned()));
    assert_eq!(
        actual,
        &expected,
        "expected key '{}' to have value {expected:?}, got {actual:?}",
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
    assert_str_contains(output, snippet.as_str(), "formatted output");
}

#[then("deserialization fails with message containing {snippet}")]
fn then_deserialization_fails(world: &mut TestWorld, snippet: QuotedString) {
    let err = world
        .deserialization_error
        .as_ref()
        .expect("deserialization error should be set");
    assert_str_contains(err, snippet.as_str(), "deserialization error");
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_core.feature")]
fn sempai_core_behaviour(world: TestWorld) { let _ = world; }
