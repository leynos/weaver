//! Behaviour-driven tests for `sempai_core` types.

use anyhow::{Context, Result, bail};
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

fn parse_diagnostic_code(code: &str) -> Result<DiagnosticCode> {
    let serialized_code =
        serde_json::to_string(code).context("diagnostic code should serialize")?;
    serde_json::from_str(&serialized_code)
        .with_context(|| format!("unrecognised diagnostic code {code}"))
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

fn build_single_diagnostic_report(
    constructor: fn(DiagnosticCode, String, Option<SourceSpan>, Vec<String>) -> DiagnosticReport,
    code: &str,
    message: &str,
) -> Result<DiagnosticReport> {
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
) -> Result<()> {
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
) -> Result<()> {
    let (start_byte, end_byte) =
        parse_byte_range(byte_range.as_str()).context("byte range should be valid")?;
    let (start_lc, end_lc) =
        parse_line_range(line_range.as_str()).context("line range should be valid")?;
    world.span = Some(Span::new(start_byte, end_byte, start_lc, end_lc));
    Ok(())
}

#[given("language {name}")]
fn given_language(world: &mut TestWorld, name: QuotedString) -> Result<()> {
    let language = name
        .as_str()
        .parse()
        .context("language name should be valid")?;
    world.language = Some(language);
    Ok(())
}

#[given("a diagnostic with code {code} and message {message}")]
fn given_diagnostic(
    world: &mut TestWorld,
    code: QuotedString,
    message: QuotedString,
) -> Result<()> {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::single_error)
}

#[given("a parser diagnostic with code {code} and message {message}")]
fn given_parser_diagnostic(
    world: &mut TestWorld,
    code: QuotedString,
    message: QuotedString,
) -> Result<()> {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::parser_error)
}

#[given("a validator diagnostic with code {code} and message {message}")]
fn given_validator_diagnostic(
    world: &mut TestWorld,
    code: QuotedString,
    message: QuotedString,
) -> Result<()> {
    given_report_with_constructor(world, &code, &message, DiagnosticReport::validation_error)
}

#[given("a not-implemented report for feature {feature}")]
fn given_not_implemented_report(world: &mut TestWorld, feature: QuotedString) {
    world.report = Some(DiagnosticReport::not_implemented(feature.as_str()));
}

#[given("diagnostic code payload {code}")]
fn given_diagnostic_code_payload(world: &mut TestWorld, code: QuotedString) -> Result<()> {
    let serialized_code =
        serde_json::to_string(code.as_str()).context("diagnostic code payload should serialize")?;
    world.diagnostic_code_payload = Some(serialized_code);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the span is serialized to JSON")]
fn when_serialize_span(world: &mut TestWorld) -> Result<()> {
    let span = world.span.as_ref().context("span should be set")?;
    let serialized_span = serde_json::to_string(span).context("span should serialize")?;
    world.json_output = Some(serialized_span);
    Ok(())
}

#[when("the language is serialized and deserialized")]
fn when_language_round_trip(world: &mut TestWorld) -> Result<()> {
    let language = world.language.context("language should be set")?;
    let serialized_language =
        serde_json::to_string(&language).context("language should serialize")?;
    let deserialized_language: Language =
        serde_json::from_str(&serialized_language).context("language should deserialize")?;
    world.round_tripped_language = Some(deserialized_language);
    Ok(())
}

#[when("the diagnostic report is formatted")]
fn when_format_report(world: &mut TestWorld) -> Result<()> {
    let report = world.report.as_ref().context("report should be set")?;
    world.formatted_output = Some(format!("{report}"));
    Ok(())
}

#[when("the diagnostic report is serialized to JSON")]
fn when_serialize_diagnostic_report(world: &mut TestWorld) -> Result<()> {
    let report = world.report.as_ref().context("report should be set")?;
    let serialized_report = serde_json::to_string(report).context("report should serialize")?;
    world.json_output = Some(serialized_report);
    Ok(())
}

#[when("the diagnostic code payload is deserialized")]
fn when_deserialize_diagnostic_code_payload(world: &mut TestWorld) -> Result<()> {
    let payload = world
        .diagnostic_code_payload
        .as_ref()
        .context("diagnostic code payload should be set")?;
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
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let json = world.json_output.as_ref().context("JSON should be set")?;
    let parsed_json: serde_json::Value =
        serde_json::from_str(json).context("JSON output should be valid")?;
    let diagnostic_object = parsed_json
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .and_then(|diagnostics| diagnostics.first())
        .and_then(serde_json::Value::as_object)
        .context("first diagnostic object should exist")?;
    Ok(diagnostic_object.clone())
}

fn assert_str_contains(haystack: &str, needle: &str, label: &str) -> Result<()> {
    if !haystack.contains(needle) {
        bail!("expected {label} to contain '{needle}', got: {haystack}");
    }
    Ok(())
}

#[then("the JSON contains key {key} with value {value}")]
fn then_json_contains(world: &mut TestWorld, key: QuotedString, value: QuotedString) -> Result<()> {
    let json = world.json_output.as_ref().context("JSON should be set")?;
    let parsed_json: serde_json::Value =
        serde_json::from_str(json).context("JSON output should be valid")?;
    let Some(actual) = parsed_json.get(key.as_str()) else {
        bail!(
            "expected JSON to contain key '{}', got: {json}",
            key.as_str()
        );
    };
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(value.as_str().to_owned()));
    if actual != &expected {
        bail!(
            "expected JSON key '{}' to have value {expected:?}, got {actual:?}",
            key.as_str()
        );
    }
    Ok(())
}

#[then("the first diagnostic JSON contains key {key}")]
fn then_first_diagnostic_contains_key(world: &mut TestWorld, key: QuotedString) -> Result<()> {
    let first = first_diagnostic_object(world)?;
    if !first.contains_key(key.as_str()) {
        bail!(
            "expected first diagnostic JSON to contain key '{}', got: {first:?}",
            key.as_str()
        );
    }
    Ok(())
}

#[then("the first diagnostic JSON does not contain key {key}")]
fn then_first_diagnostic_does_not_contain_key(
    world: &mut TestWorld,
    key: QuotedString,
) -> Result<()> {
    let first = first_diagnostic_object(world)?;
    if first.contains_key(key.as_str()) {
        bail!(
            "expected first diagnostic JSON to not contain key '{}', got: {first:?}",
            key.as_str()
        );
    }
    Ok(())
}

#[then("the first diagnostic JSON contains key {key} with value {value}")]
fn then_first_diagnostic_contains_key_with_value(
    world: &mut TestWorld,
    key: QuotedString,
    value: QuotedString,
) -> Result<()> {
    let first = first_diagnostic_object(world)?;
    let Some(actual) = first.get(key.as_str()) else {
        bail!(
            "expected first diagnostic JSON to contain key '{}', got: {first:?}",
            key.as_str()
        );
    };
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(value.as_str().to_owned()));
    if actual != &expected {
        bail!(
            "expected key '{}' to have value {expected:?}, got {actual:?}",
            key.as_str()
        );
    }
    Ok(())
}

#[then("the round-tripped language equals the original")]
fn then_language_round_trip_equals(world: &mut TestWorld) -> Result<()> {
    let original = world.language.context("original language should be set")?;
    let round_tripped = world
        .round_tripped_language
        .context("round-tripped language should be set")?;
    if original != round_tripped {
        bail!("expected {original:?} to equal round-tripped {round_tripped:?}");
    }
    Ok(())
}

#[then("the formatted output contains {snippet}")]
fn then_formatted_contains(world: &mut TestWorld, snippet: QuotedString) -> Result<()> {
    let output = world
        .formatted_output
        .as_ref()
        .context("formatted output should be set")?;
    assert_str_contains(output, snippet.as_str(), "formatted output")
}

#[then("deserialization fails with message containing {snippet}")]
fn then_deserialization_fails(world: &mut TestWorld, snippet: QuotedString) -> Result<()> {
    let error = world
        .deserialization_error
        .as_ref()
        .context("deserialization error should be set")?;
    assert_str_contains(error, snippet.as_str(), "deserialization error")
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/sempai_core.feature")]
fn sempai_core_behaviour(world: TestWorld) { let _ = world; }
