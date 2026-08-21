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

fn parse_diagnostic_code(code: &str) -> Result<DiagnosticCode, String> {
    let json = serde_json::to_string(code).map_err(|error| error.to_string())?;
    serde_json::from_str(&json)
        .map_err(|error| format!("unrecognised diagnostic code {code}: {error}"))
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

fn build_single_diagnostic_report(
    constructor: fn(DiagnosticCode, String, Option<SourceSpan>, Vec<String>) -> DiagnosticReport,
    code: &str,
    message: &str,
) -> Result<DiagnosticReport, String> {
    let diagnostic_code = parse_diagnostic_code(code)?;
    Ok(constructor(
        diagnostic_code,
        message.to_owned(),
        None,
        vec![],
    ))
}

fn given_report_with_constructor(
    world: &mut TestWorld,
    code: &QuotedString,
    message: &QuotedString,
    constructor: fn(DiagnosticCode, String, Option<SourceSpan>, Vec<String>) -> DiagnosticReport,
) -> Result<(), String> {
    world.report = Some(build_single_diagnostic_report(
        constructor,
        code.as_str(),
        message.as_str(),
    )?);
    Ok(())
}

#[given("a span from bytes {byte_range} at lines {line_range}")]
fn given_span(
    world: &mut TestWorld,
    byte_range: QuotedString,
    line_range: QuotedString,
) -> Result<(), String> {
    let (start_byte, end_byte) =
        parse_byte_range(byte_range.as_str()).map_err(|error| error.to_string())?;
    let (start_lc, end_lc) =
        parse_line_range(line_range.as_str()).map_err(|error| error.to_string())?;
    world.span = Some(Span::new(start_byte, end_byte, start_lc, end_lc));
    Ok(())
}

#[given("language {name}")]
fn given_language(world: &mut TestWorld, name: QuotedString) -> Result<(), String> {
    world.language = Some(
        name.as_str()
            .parse::<Language>()
            .map_err(|error| error.to_string())?,
    );
    Ok(())
}

#[given("a diagnostic with code {code} and message {message}")]
fn given_diagnostic(
    world: &mut TestWorld,
    code: QuotedString,
    message: QuotedString,
) -> Result<(), String> {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::single_error)
}

#[given("a parser diagnostic with code {code} and message {message}")]
fn given_parser_diagnostic(
    world: &mut TestWorld,
    code: QuotedString,
    message: QuotedString,
) -> Result<(), String> {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::parser_error)
}

#[given("a validator diagnostic with code {code} and message {message}")]
fn given_validator_diagnostic(
    world: &mut TestWorld,
    code: QuotedString,
    message: QuotedString,
) -> Result<(), String> {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::validation_error)
}

#[given("a not-implemented report for feature {feature}")]
fn given_not_implemented_report(world: &mut TestWorld, feature: QuotedString) {
    world.report = Some(DiagnosticReport::not_implemented(feature.as_str()));
}

#[given("diagnostic code payload {code}")]
fn given_diagnostic_code_payload(world: &mut TestWorld, code: QuotedString) -> Result<(), String> {
    world.diagnostic_code_payload =
        Some(serde_json::to_string(code.as_str()).map_err(|error| error.to_string())?);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the span is serialized to JSON")]
fn when_serialize_span(world: &mut TestWorld) -> Result<(), String> {
    let span = world.span.as_ref().ok_or("span should be set")?;
    world.json_output = Some(serde_json::to_string(span).map_err(|error| error.to_string())?);
    Ok(())
}

#[when("the language is serialized and deserialized")]
fn when_language_round_trip(world: &mut TestWorld) -> Result<(), String> {
    let lang = world.language.ok_or("language should be set")?;
    let json = serde_json::to_string(&lang).map_err(|error| error.to_string())?;
    let deserialized: Language = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    world.round_tripped_language = Some(deserialized);
    Ok(())
}

#[when("the diagnostic report is formatted")]
fn when_format_report(world: &mut TestWorld) -> Result<(), String> {
    let report = world.report.as_ref().ok_or("report should be set")?;
    world.formatted_output = Some(format!("{report}"));
    Ok(())
}

#[when("the diagnostic report is serialized to JSON")]
fn when_serialize_diagnostic_report(world: &mut TestWorld) -> Result<(), String> {
    let report = world.report.as_ref().ok_or("report should be set")?;
    world.json_output = Some(serde_json::to_string(report).map_err(|error| error.to_string())?);
    Ok(())
}

#[when("the diagnostic code payload is deserialized")]
fn when_deserialize_diagnostic_code_payload(world: &mut TestWorld) -> Result<(), String> {
    let payload = world
        .diagnostic_code_payload
        .as_ref()
        .ok_or("diagnostic code payload should be set")?;
    world.deserialization_error = serde_json::from_str::<DiagnosticCode>(payload)
        .err()
        .map(|e| e.to_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

fn first_diagnostic_object(
    world: &TestWorld,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let json = world.json_output.as_ref().ok_or("JSON should be set")?;
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    parsed
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(serde_json::Value::as_object)
        .cloned()
        .ok_or_else(|| String::from("first diagnostic object should exist"))
}

fn assert_str_contains(haystack: &str, needle: &str, label: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!(
            "expected {label} to contain '{needle}', got: {haystack}"
        ))
    }
}

#[then("the JSON contains key {key} with value {value}")]
fn then_json_contains(
    world: &mut TestWorld,
    key: QuotedString,
    value: QuotedString,
) -> Result<(), String> {
    let json = world.json_output.as_ref().ok_or("JSON should be set")?;
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    let actual = parsed.get(key.as_str()).ok_or_else(|| {
        format!(
            "expected JSON to contain key '{}', got: {json}",
            key.as_str()
        )
    })?;
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(value.as_str().to_owned()));
    if actual == &expected {
        Ok(())
    } else {
        Err(format!(
            "expected JSON key '{}' to have value {expected:?}, got {actual:?}",
            key.as_str()
        ))
    }
}

#[then("the first diagnostic JSON contains key {key}")]
fn then_first_diagnostic_contains_key(
    world: &mut TestWorld,
    key: QuotedString,
) -> Result<(), String> {
    let first = first_diagnostic_object(world)?;
    if first.contains_key(key.as_str()) {
        Ok(())
    } else {
        Err(format!(
            "expected first diagnostic JSON to contain key '{}', got: {first:?}",
            key.as_str()
        ))
    }
}

#[then("the first diagnostic JSON does not contain key {key}")]
fn then_first_diagnostic_does_not_contain_key(
    world: &mut TestWorld,
    key: QuotedString,
) -> Result<(), String> {
    let first = first_diagnostic_object(world)?;
    if first.contains_key(key.as_str()) {
        Err(format!(
            "expected first diagnostic JSON to not contain key '{}', got: {first:?}",
            key.as_str()
        ))
    } else {
        Ok(())
    }
}

#[then("the first diagnostic JSON contains key {key} with value {value}")]
fn then_first_diagnostic_contains_key_with_value(
    world: &mut TestWorld,
    key: QuotedString,
    value: QuotedString,
) -> Result<(), String> {
    let first = first_diagnostic_object(world)?;
    let actual = first.get(key.as_str()).ok_or_else(|| {
        format!(
            "expected first diagnostic JSON to contain key '{}', got: {first:?}",
            key.as_str()
        )
    })?;
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(value.as_str().to_owned()));
    if actual == &expected {
        Ok(())
    } else {
        Err(format!(
            "expected key '{}' to have value {expected:?}, got {actual:?}",
            key.as_str()
        ))
    }
}

#[then("the round-tripped language equals the original")]
fn then_language_round_trip_equals(world: &mut TestWorld) -> Result<(), String> {
    let original = world.language.ok_or("original language should be set")?;
    let round_tripped = world
        .round_tripped_language
        .ok_or("round-tripped language should be set")?;
    if original == round_tripped {
        Ok(())
    } else {
        Err(format!(
            "expected round-tripped language {original:?}, got {round_tripped:?}"
        ))
    }
}

#[then("the formatted output contains {snippet}")]
fn then_formatted_contains(world: &mut TestWorld, snippet: QuotedString) -> Result<(), String> {
    let output = world
        .formatted_output
        .as_ref()
        .ok_or("formatted output should be set")?;
    assert_str_contains(output, snippet.as_str(), "formatted output")
}

#[then("deserialization fails with message containing {snippet}")]
fn then_deserialization_fails(world: &mut TestWorld, snippet: QuotedString) -> Result<(), String> {
    let err = world
        .deserialization_error
        .as_ref()
        .ok_or("deserialization error should be set")?;
    assert_str_contains(err, snippet.as_str(), "deserialization error")
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_core.feature")]
fn sempai_core_behaviour(world: TestWorld) { let _ = world; }
