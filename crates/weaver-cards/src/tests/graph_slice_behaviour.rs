//! Behaviour-driven tests for `graph-slice` schema contracts.

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
fn given_refusal(world: &mut TestWorld, reason: QuotedString) {
    let parsed = reason
        .as_str()
        .parse::<SliceRefusalReason>()
        .expect("valid refusal reason");
    world.response = Some(graph_slice_fixtures::sample_refusal(parsed));
}

#[given("a graph-slice request with no optional flags")]
fn given_default_request(world: &mut TestWorld) {
    let args = vec![
        String::from("--uri"),
        String::from("file:///src/main.rs"),
        String::from("--position"),
        String::from("10:5"),
    ];
    world.request = Some(GraphSliceRequest::parse(&args).expect("valid request"));
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
fn when_response_serialized(world: &mut TestWorld) {
    let response = world.response.as_ref().expect("response should be set");
    world.json_output = Some(serde_json::to_string(response).expect("serialize"));
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

fn parse_json(world: &TestWorld) -> serde_json::Value {
    let json = world.json_output.as_ref().expect("JSON should be set");
    serde_json::from_str(json).expect("valid JSON")
}

fn json_pointer(field: &str) -> String { format!("/{}", field.replace('.', "/")) }

#[then("the slice JSON contains a {field} field")]
fn then_json_contains(world: &mut TestWorld, field: QuotedString) {
    let parsed = parse_json(world);
    let pointer = json_pointer(field.as_str());
    assert!(
        parsed.pointer(&pointer).is_some(),
        "expected JSON to contain field '{}', got: {parsed}",
        field.as_str()
    );
}

#[then("the slice JSON field {key} has value {value}")]
fn then_json_field_value(world: &mut TestWorld, key: QuotedString, value: QuotedString) {
    let parsed = parse_json(world);
    let pointer = json_pointer(key.as_str());
    let Some(actual) = parsed.pointer(&pointer) else {
        panic!("expected JSON to contain key '{}'", key.as_str());
    };
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

#[then("the slice JSON field {key} is empty")]
fn then_json_field_is_empty(world: &mut TestWorld, key: QuotedString) {
    let parsed = parse_json(world);
    let pointer = json_pointer(key.as_str());
    let Some(actual) = parsed.pointer(&pointer) else {
        panic!("expected JSON to contain key '{}'", key.as_str());
    };
    match actual {
        serde_json::Value::Array(arr) if arr.is_empty() => {}
        serde_json::Value::Object(obj) if obj.is_empty() => {}
        serde_json::Value::String(s) if s.is_empty() => {}
        serde_json::Value::Null => {}
        other => panic!("expected '{}' to be empty, got {:?}", key.as_str(), other),
    }
}

#[then("the depth is {depth}")]
fn then_depth_is(world: &mut TestWorld, depth: QuotedString) {
    let request = world.request.as_ref().expect("request should be set");
    let expected: u32 = depth.as_str().parse().expect("valid u32 in feature file");
    assert_eq!(request.depth(), expected);
}

#[then("the direction is {direction}")]
fn then_direction_is(world: &mut TestWorld, direction: QuotedString) {
    let request = world.request.as_ref().expect("request should be set");
    let expected: crate::SliceDirection = direction
        .as_str()
        .parse()
        .expect("valid direction in feature file");
    assert_eq!(request.direction(), expected);
}

#[then("the edge types include {edge_type}")]
fn then_edge_types_include(world: &mut TestWorld, edge_type: QuotedString) {
    let request = world.request.as_ref().expect("request should be set");
    let expected: SliceEdgeType = edge_type
        .as_str()
        .parse()
        .expect("valid edge type in feature file");
    assert!(
        request.edge_types().contains(&expected),
        "expected edge types to include {:?}, got: {:?}",
        expected,
        request.edge_types()
    );
}

#[then("the edge types are {types}")]
fn then_edge_types_are(world: &mut TestWorld, types: QuotedString) {
    let request = world.request.as_ref().expect("request should be set");
    let expected: Vec<SliceEdgeType> = types
        .as_str()
        .split(',')
        .map(|s| s.trim().parse().expect("valid edge type"))
        .collect();
    assert_eq!(request.edge_types(), &expected);
}

#[then("the request is rejected")]
fn then_request_rejected(world: &mut TestWorld) {
    assert!(
        world.request_error.is_some(),
        "expected request to be rejected, but it succeeded"
    );
}

#[then("the response contains edge with resolution_scope {scope}")]
fn then_response_contains_resolution_scope(world: &mut TestWorld, scope: QuotedString) {
    let parsed = parse_json(world);
    let edges = parsed
        .get("edges")
        .and_then(|v| v.as_array())
        .expect("edges array");
    let expected_scope = scope
        .as_str()
        .parse::<ResolutionScope>()
        .expect("valid resolution scope");
    let serialized = serde_json::to_string(&expected_scope).expect("serialize");
    let expected_str = serialized.trim_matches('"');
    let found = edges
        .iter()
        .any(|edge| edge.get("resolution_scope").and_then(|v| v.as_str()) == Some(expected_str));
    assert!(
        found,
        "expected to find edge with resolution_scope '{expected_str}'"
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/graph_slice_schema.feature")]
fn graph_slice_schema_behaviour(world: TestWorld) { drop(world); }
