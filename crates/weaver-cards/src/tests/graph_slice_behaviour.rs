//! Behaviour-driven tests for `graph-slice` schema contracts.

use anyhow::{Context, Result, bail, ensure};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::{graph_slice_fixtures, test_utils::QuotedString};
use crate::{
    GraphSliceRequest,
    GraphSliceResponse,
    SliceEdgeType,
    graph_slice::{ResolutionScope, SliceRefusalReason},
};

// ---------------------------------------------------------------------------
// Test world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TestWorld {
    response: Option<GraphSliceResponse>,
    request: Option<GraphSliceRequest>,
    request_error: Option<String>,
    json_output: Option<String>,
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> TestWorld { TestWorld::default() }

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a graph-slice success response with default budget")]
fn given_default_success(world: &mut TestWorld) {
    world.response = Some(graph_slice_fixtures::sample_success_response());
}

#[given("a graph-slice truncated response with spillover")]
fn given_truncated_response(world: &mut TestWorld) {
    world.response = Some(graph_slice_fixtures::sample_truncated_response());
}

#[given("a graph-slice refusal with reason {reason}")]
fn given_refusal(world: &mut TestWorld, reason: QuotedString) -> Result<()> {
    let parsed = reason
        .as_str()
        .parse::<SliceRefusalReason>()
        .with_context(|| {
            format!(
                "feature file contains an invalid refusal reason: {}",
                reason.as_str()
            )
        })?;
    world.response = Some(graph_slice_fixtures::sample_refusal(parsed));
    Ok(())
}

#[given("a graph-slice request with no optional flags")]
fn given_default_request(world: &mut TestWorld) -> Result<()> {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
    ];
    world.request = Some(GraphSliceRequest::parse(&args).context("default request is valid")?);
    Ok(())
}

#[given("a graph-slice request with edge types {types}")]
fn given_request_with_edge_types(world: &mut TestWorld, types: QuotedString) {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
        String::from("--edge-types"),
        String::from(types.as_str()),
    ];
    match GraphSliceRequest::parse(&args) {
        Ok(request) => world.request = Some(request),
        Err(error) => world.request_error = Some(error.to_string()),
    }
}

#[given("a graph-slice request with depth {depth}")]
fn given_request_with_depth(world: &mut TestWorld, depth: QuotedString) {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
        String::from("--depth"),
        String::from(depth.as_str()),
    ];
    match GraphSliceRequest::parse(&args) {
        Ok(request) => world.request = Some(request),
        Err(error) => world.request_error = Some(error.to_string()),
    }
}

#[given("a graph-slice response with all resolution scopes")]
fn given_multi_resolution(world: &mut TestWorld) {
    world.response = Some(graph_slice_fixtures::sample_multi_resolution_response());
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the slice response is serialized to JSON")]
fn when_response_serialized(world: &mut TestWorld) -> Result<()> {
    let response = world.response.as_ref().context("response should be set")?;
    world.json_output = Some(serde_json::to_string(response).context("serialize response")?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

fn parse_json(world: &TestWorld) -> Result<serde_json::Value> {
    let json = world.json_output.as_ref().context("JSON should be set")?;
    serde_json::from_str(json).context("serialized response contains valid JSON")
}

fn json_pointer(field: &str) -> String { format!("/{}", field.replace('.', "/")) }

#[then("the slice JSON contains a {field} field")]
fn then_json_contains(world: &mut TestWorld, field: QuotedString) -> Result<()> {
    let parsed = parse_json(world)?;
    let pointer = json_pointer(field.as_str());
    ensure!(
        parsed.pointer(&pointer).is_some(),
        "expected JSON to contain field '{}', got: {parsed}",
        field.as_str()
    );
    Ok(())
}

#[then("the slice JSON field {key} has value {value}")]
fn then_json_field_value(
    world: &mut TestWorld,
    key: QuotedString,
    value: QuotedString,
) -> Result<()> {
    let parsed = parse_json(world)?;
    let pointer = json_pointer(key.as_str());
    let Some(actual) = parsed.pointer(&pointer) else {
        bail!("expected JSON to contain key '{}'", key.as_str());
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

#[then("the slice JSON field {key} is empty")]
fn then_json_field_is_empty(world: &mut TestWorld, key: QuotedString) -> Result<()> {
    let parsed = parse_json(world)?;
    let pointer = json_pointer(key.as_str());
    let Some(actual) = parsed.pointer(&pointer) else {
        bail!("expected JSON to contain key '{}'", key.as_str());
    };
    match actual {
        serde_json::Value::Array(arr) if arr.is_empty() => {}
        serde_json::Value::Object(obj) if obj.is_empty() => {}
        serde_json::Value::String(s) if s.is_empty() => {}
        serde_json::Value::Null => {}
        other => bail!("expected '{}' to be empty, got {other:?}", key.as_str()),
    }
    Ok(())
}

#[then("the depth is {depth}")]
fn then_depth_is(world: &mut TestWorld, depth: QuotedString) -> Result<()> {
    let request = world.request.as_ref().context("request should be set")?;
    let expected = depth
        .as_str()
        .parse::<u32>()
        .context("feature file contains a valid u32 depth")?;
    ensure!(request.depth() == expected, "expected depth {expected}");
    Ok(())
}

#[then("the direction is {direction}")]
fn then_direction_is(world: &mut TestWorld, direction: QuotedString) -> Result<()> {
    let request = world.request.as_ref().context("request should be set")?;
    let expected = direction
        .as_str()
        .parse::<crate::SliceDirection>()
        .context("feature file contains a valid direction")?;
    ensure!(
        request.direction() == expected,
        "expected direction {expected:?}"
    );
    Ok(())
}

#[then("the edge types include {edge_type}")]
fn then_edge_types_include(world: &mut TestWorld, edge_type: QuotedString) -> Result<()> {
    let request = world.request.as_ref().context("request should be set")?;
    let expected = edge_type
        .as_str()
        .parse::<SliceEdgeType>()
        .context("feature file contains a valid edge type")?;
    ensure!(
        request.edge_types().contains(&expected),
        "expected edge types to include {:?}, got: {:?}",
        expected,
        request.edge_types()
    );
    Ok(())
}

#[then("the edge types are {types}")]
fn then_edge_types_are(world: &mut TestWorld, types: QuotedString) -> Result<()> {
    let request = world.request.as_ref().context("request should be set")?;
    let expected: Vec<SliceEdgeType> = types
        .as_str()
        .split(',')
        .map(|edge_type| edge_type.trim().parse::<SliceEdgeType>())
        .collect::<std::result::Result<_, _>>()
        .context("feature file contains valid edge types")?;
    ensure!(request.edge_types() == expected, "unexpected edge types");
    Ok(())
}

#[then("the request is rejected")]
fn then_request_rejected(world: &mut TestWorld) {
    assert!(
        world.request_error.is_some(),
        "expected request to be rejected, but it succeeded"
    );
}

#[then("the response contains edge with resolution_scope {scope}")]
fn then_response_contains_resolution_scope(
    world: &mut TestWorld,
    scope: QuotedString,
) -> Result<()> {
    let parsed = parse_json(world)?;
    let edges = parsed
        .get("edges")
        .and_then(|v| v.as_array())
        .context("response contains an edges array")?;
    let expected_scope = scope
        .as_str()
        .parse::<ResolutionScope>()
        .context("feature file contains a valid resolution scope")?;
    let serialized =
        serde_json::to_string(&expected_scope).context("serialize resolution scope")?;
    let expected_str = serialized.trim_matches('"');
    let found = edges
        .iter()
        .any(|edge| edge.get("resolution_scope").and_then(|v| v.as_str()) == Some(expected_str));
    ensure!(
        found,
        "expected to find edge with resolution_scope '{expected_str}'"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/graph_slice_schema.feature")]
fn graph_slice_schema_behaviour(world: TestWorld) { drop(world); }
