//! Behaviour-driven tests for `weaver-cards` schema contracts.

use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use super::fixtures;
use crate::{CardRefusal, DetailLevel, GetCardRequest, GetCardResponse, RefusalReason, SymbolCard};

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
// Fixture builders (delegates to shared fixtures module)
// ---------------------------------------------------------------------------

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
    Ok(fixtures::build_card_at_level(level))
}

fn build_refusal_response(reason: RefusalReason, detail: DetailLevel) -> GetCardResponse {
    if reason == RefusalReason::NotYetImplemented {
        return GetCardResponse::not_yet_implemented(detail);
    }
    let message = match &reason {
        RefusalReason::NoSymbolAtPosition => {
            String::from("no symbol found at the requested position")
        }
        RefusalReason::UnsupportedLanguage => {
            String::from("the requested language is not supported")
        }
        RefusalReason::BackendUnavailable => String::from("the required backend is not available"),
        // NotYetImplemented is handled by the early return above; the
        // wildcard covers future #[non_exhaustive] variants.
        _ => String::from("card could not be produced"),
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

#[given("a get-card request with an unknown flag")]
fn given_request_unknown_flag(world: &mut TestWorld) {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
        String::from("--some-unknown"),
        String::from("value"),
    ];
    world.request = Some(GetCardRequest::parse(&args).expect("unknown flags should be skipped"));
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

fn parse_json_and_pointer(world: &TestWorld, field: &QuotedString) -> (serde_json::Value, String) {
    let json = world.json_output.as_ref().expect("JSON should be set");
    let parsed: serde_json::Value = serde_json::from_str(json).expect("valid JSON");
    let pointer = format!("/{}", field.as_str().replace('.', "/"));
    (parsed, pointer)
}

#[then("the JSON contains a {field} field")]
fn then_json_contains_field(world: &mut TestWorld, field: QuotedString) {
    let (parsed, pointer) = parse_json_and_pointer(world, &field);
    assert!(
        parsed.pointer(&pointer).is_some(),
        "expected JSON to contain field '{}', got: {parsed}",
        field.as_str()
    );
}

#[then("the JSON does not contain a {field} field")]
fn then_json_does_not_contain_field(world: &mut TestWorld, field: QuotedString) {
    let (parsed, pointer) = parse_json_and_pointer(world, &field);
    assert!(
        parsed.pointer(&pointer).is_none(),
        "expected JSON NOT to contain field '{}', got: {parsed}",
        field.as_str()
    );
}

#[then("the JSON field {key} has value {value}")]
fn then_json_field_has_value(world: &mut TestWorld, key: QuotedString, value: QuotedString) {
    let (parsed, pointer) = parse_json_and_pointer(world, &key);
    let actual = parsed
        .pointer(&pointer)
        .unwrap_or_else(|| panic!("expected JSON to contain key '{}'", key.as_str()));
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(String::from(value.as_str())));
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
