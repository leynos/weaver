//! Behaviour-driven tests for `weaver-cards` schema contracts.

use anyhow::{Context, Result, bail, ensure};
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

fn parse_detail_level(raw: &str) -> Result<DetailLevel> {
    raw.parse()
        .map_err(|error: crate::DetailLevelParseError| anyhow::Error::msg(error.to_string()))
}

fn parse_refusal_reason(raw: &str) -> Result<RefusalReason> {
    match raw {
        "no_symbol_at_position" => Ok(RefusalReason::NoSymbolAtPosition),
        "position_out_of_range" => Ok(RefusalReason::PositionOutOfRange),
        "unsupported_language" => Ok(RefusalReason::UnsupportedLanguage),
        "not_yet_implemented" => Ok(RefusalReason::NotYetImplemented),
        "backend_unavailable" => Ok(RefusalReason::BackendUnavailable),
        other => bail!("unknown refusal reason: {other}"),
    }
}

fn build_card(detail: &str) -> Result<SymbolCard> {
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

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a symbol card at {detail} detail level")]
fn given_card_at_detail(world: &mut TestWorld, detail: QuotedString) -> Result<()> {
    world.card =
        Some(build_card(detail.as_str()).context("feature file contains a valid detail level")?);
    Ok(())
}

#[given("a refusal response with reason {reason}")]
fn given_refusal_response(world: &mut TestWorld, reason: QuotedString) -> Result<()> {
    let parsed_reason = parse_refusal_reason(reason.as_str())
        .context("feature file contains a valid refusal reason")?;
    let detail = DetailLevel::Structure;
    world.response = Some(build_refusal_response(parsed_reason, detail));
    Ok(())
}

#[given("a success response with a {detail} detail card")]
fn given_success_response(world: &mut TestWorld, detail: QuotedString) -> Result<()> {
    let card = build_card(detail.as_str()).context("feature file contains a valid detail level")?;
    world.response = Some(GetCardResponse::Success {
        card: Box::new(card),
    });
    Ok(())
}

#[given("a get-card request with no detail flag")]
fn given_request_no_detail(world: &mut TestWorld) -> Result<()> {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
    ];
    world.request = Some(GetCardRequest::parse(&args).context("default request is valid")?);
    Ok(())
}

#[given("a get-card request with an unknown flag")]
fn given_request_unknown_flag(world: &mut TestWorld) -> Result<()> {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
        String::from("--some-unknown"),
        String::from("value"),
    ];
    world.request =
        Some(GetCardRequest::parse(&args).context("unknown request flags are skipped")?);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the card is serialized to JSON")]
fn when_card_serialized(world: &mut TestWorld) -> Result<()> {
    let card = world.card.as_ref().context("card should be set")?;
    world.json_output = Some(serde_json::to_string(card).context("serialize card")?);
    Ok(())
}

#[when("the response is serialized to JSON")]
fn when_response_serialized(world: &mut TestWorld) -> Result<()> {
    let response = world.response.as_ref().context("response should be set")?;
    world.json_output = Some(serde_json::to_string(response).context("serialize response")?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

fn parse_json_and_pointer(
    world: &TestWorld,
    field: &QuotedString,
) -> Result<(serde_json::Value, String)> {
    let json = world.json_output.as_ref().context("JSON should be set")?;
    let parsed = serde_json::from_str(json).context("serialized output contains valid JSON")?;
    let pointer = format!("/{}", field.as_str().replace('.', "/"));
    Ok((parsed, pointer))
}

#[then("the JSON contains a {field} field")]
fn then_json_contains_field(world: &mut TestWorld, field: QuotedString) -> Result<()> {
    let (parsed, pointer) = parse_json_and_pointer(world, &field)?;
    ensure!(
        parsed.pointer(&pointer).is_some(),
        "expected JSON to contain field '{}', got: {parsed}",
        field.as_str()
    );
    Ok(())
}

#[then("the JSON does not contain a {field} field")]
fn then_json_does_not_contain_field(world: &mut TestWorld, field: QuotedString) -> Result<()> {
    let (parsed, pointer) = parse_json_and_pointer(world, &field)?;
    ensure!(
        parsed.pointer(&pointer).is_none(),
        "expected JSON NOT to contain field '{}', got: {parsed}",
        field.as_str()
    );
    Ok(())
}

#[then("the JSON field {key} has value {value}")]
fn then_json_field_has_value(
    world: &mut TestWorld,
    key: QuotedString,
    value: QuotedString,
) -> Result<()> {
    let (parsed, pointer) = parse_json_and_pointer(world, &key)?;
    let Some(actual) = parsed.pointer(&pointer) else {
        bail!("expected JSON to contain key '{}': {parsed}", key.as_str());
    };
    let expected: serde_json::Value = serde_json::from_str(value.as_str())
        .unwrap_or_else(|_| serde_json::Value::String(String::from(value.as_str())));
    ensure!(
        actual == &expected,
        "expected '{}' = {:?}, got {:?}",
        key.as_str(),
        expected,
        actual
    );
    Ok(())
}

#[then("the detail level is {level}")]
fn then_detail_level_is(world: &mut TestWorld, level: QuotedString) -> Result<()> {
    let request = world.request.as_ref().context("request should be set")?;
    let expected =
        parse_detail_level(level.as_str()).context("feature file contains a valid detail level")?;
    ensure!(request.detail == expected, "unexpected detail level");
    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/get_card_schema.feature")]
fn get_card_schema_behaviour(world: TestWorld) { drop(world); }
