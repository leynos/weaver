//! Behaviour-driven tests for `weaver-cards` schema contracts.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::{fixtures, test_utils::QuotedString};
use crate::{CardRefusal, DetailLevel, GetCardRequest, GetCardResponse, RefusalReason, SymbolCard};

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

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> TestWorld { TestWorld::default() }

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
        "position_out_of_range" => Ok(RefusalReason::PositionOutOfRange),
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
        RefusalReason::PositionOutOfRange => {
            String::from("the requested position is outside the file bounds")
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

/// Parses arguments into the request state required by subsequent BDD steps.
fn set_request(world: &mut TestWorld, arguments: &[&str]) -> Result<(), String> {
    let request_arguments: Vec<String> = arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect();
    world.request =
        Some(GetCardRequest::parse(&request_arguments).map_err(|error| error.to_string())?);
    Ok(())
}

/// Asserts whether a JSON field is present using the common dotted-field syntax.
fn assert_json_field_presence(
    world: &TestWorld,
    field: &QuotedString,
    should_be_present: bool,
) -> Result<(), String> {
    let (parsed, pointer) = parse_json_and_pointer(world, field)?;
    let is_present = parsed.pointer(&pointer).is_some();
    if is_present == should_be_present {
        return Ok(());
    }
    let expectation = if should_be_present {
        "contain"
    } else {
        "NOT contain"
    };
    Err(format!(
        "expected JSON to {expectation} field '{}', got: {parsed}",
        field.as_str()
    ))
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a symbol card at {detail} detail level")]
fn given_card_at_detail(world: &mut TestWorld, detail: QuotedString) -> Result<(), String> {
    world.card = Some(build_card(detail.as_str())?);
    Ok(())
}

#[given("a refusal response with reason {reason}")]
fn given_refusal_response(world: &mut TestWorld, reason: QuotedString) -> Result<(), String> {
    let parsed_reason = parse_refusal_reason(reason.as_str())?;
    let detail = DetailLevel::Structure;
    world.response = Some(build_refusal_response(parsed_reason, detail));
    Ok(())
}

#[given("a success response with a {detail} detail card")]
fn given_success_response(world: &mut TestWorld, detail: QuotedString) -> Result<(), String> {
    let card = build_card(detail.as_str())?;
    world.response = Some(GetCardResponse::Success {
        card: Box::new(card),
    });
    Ok(())
}

#[given("a get-card request with no detail flag")]
fn given_request_no_detail(world: &mut TestWorld) -> Result<(), String> {
    set_request(
        world,
        &["--uri", "file:///src/main.rs", "--position", "10:5"],
    )
}

#[given("a get-card request with an unknown flag")]
fn given_request_unknown_flag(world: &mut TestWorld) -> Result<(), String> {
    set_request(
        world,
        &[
            "--uri",
            "file:///src/main.rs",
            "--position",
            "10:5",
            "--some-unknown",
            "value",
        ],
    )
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the card is serialized to JSON")]
fn when_card_serialized(world: &mut TestWorld) -> Result<(), String> {
    let card = world
        .card
        .as_ref()
        .ok_or("card must be set for this step")?;
    world.json_output = Some(serde_json::to_string(card).map_err(|error| error.to_string())?);
    Ok(())
}

#[when("the response is serialized to JSON")]
fn when_response_serialized(world: &mut TestWorld) -> Result<(), String> {
    let response = world.response.as_ref().ok_or("response should be set")?;
    world.json_output = Some(serde_json::to_string(response).map_err(|error| error.to_string())?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

fn parse_json_and_pointer(
    world: &TestWorld,
    field: &QuotedString,
) -> Result<(serde_json::Value, String), String> {
    let json = world.json_output.as_ref().ok_or("JSON should be set")?;
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    let pointer = format!("/{}", field.as_str().replace('.', "/"));
    Ok((parsed, pointer))
}

#[then("the JSON contains a {field} field")]
fn then_json_contains_field(world: &mut TestWorld, field: QuotedString) -> Result<(), String> {
    assert_json_field_presence(world, &field, true)
}

#[then("the JSON does not contain a {field} field")]
fn then_json_does_not_contain_field(
    world: &mut TestWorld,
    field: QuotedString,
) -> Result<(), String> {
    assert_json_field_presence(world, &field, false)
}

#[then("the JSON field {key} has value {value}")]
fn then_json_field_has_value(
    world: &mut TestWorld,
    key: QuotedString,
    value: QuotedString,
) -> Result<(), String> {
    let (parsed, pointer) = parse_json_and_pointer(world, &key)?;
    let missing_key_message = format!("expected JSON to contain key '{}': {parsed}", key.as_str());
    let actual = parsed.pointer(&pointer).ok_or(missing_key_message)?;
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
    Ok(())
}

#[then("the detail level is {level}")]
fn then_detail_level_is(world: &mut TestWorld, level: QuotedString) -> Result<(), String> {
    let request = world.request.as_ref().ok_or("request should be set")?;
    let expected = parse_detail_level(level.as_str())?;
    assert_eq!(request.detail, expected);
    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/get_card_schema.feature")]
fn get_card_schema_behaviour(world: TestWorld) { drop(world); }
