//! Behaviour-driven tests for capability contract validation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::capability::{
    CapabilityContract, CapabilityId, ContractVersion, ReasonCode, RenameSymbolContract,
};
use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};
use crate::protocol::{
    DiagnosticSeverity, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse,
};

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
struct CapabilityWorld {
    contract: Option<RenameSymbolContract>,
    request: Option<PluginRequest>,
    response: Option<PluginResponse>,
    manifest: Option<PluginManifest>,
    validation_result: Option<Result<(), PluginError>>,
    version_a: Option<ContractVersion>,
    version_b: Option<ContractVersion>,
}

#[fixture]
fn world() -> CapabilityWorld {
    CapabilityWorld::default()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parses key-value pairs from a space-separated string like
/// `key1="val1" key2="val2"`.
///
/// Each token is split on the first `=`; the value portion has surrounding
/// double-quotes stripped. This is intentionally simple because the inputs
/// are controlled test fixtures.
fn parse_kv_pairs(input: &str) -> HashMap<String, serde_json::Value> {
    let mut map = HashMap::new();
    for token in input.split_whitespace() {
        if let Some((key, raw_value)) = token.split_once('=') {
            let value = raw_value.trim_matches('"');
            map.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a rename-symbol contract")]
fn given_contract(world: &mut CapabilityWorld) {
    world.contract = Some(RenameSymbolContract);
}

#[given("a plugin request with operation {operation} and arguments {args}")]
fn given_request_with_args(world: &mut CapabilityWorld, operation: QuotedString, args: String) {
    let arguments = parse_kv_pairs(&args);
    world.request = Some(PluginRequest::with_arguments(
        operation.as_str(),
        vec![],
        arguments,
    ));
}

#[given("a successful diff response")]
fn given_diff_response(world: &mut CapabilityWorld) {
    world.response = Some(PluginResponse::success(PluginOutput::Diff {
        content: "--- a/f\n+++ b/f\n".into(),
    }));
}

#[given("a successful analysis response")]
fn given_analysis_response(world: &mut CapabilityWorld) {
    world.response = Some(PluginResponse::success(PluginOutput::Analysis {
        data: serde_json::json!({"symbols": []}),
    }));
}

#[given("a failure response with reason code {code}")]
fn given_failure_with_reason(world: &mut CapabilityWorld, code: QuotedString) {
    let reason: ReasonCode =
        serde_json::from_str(&format!("\"{}\"", code.as_str())).expect("valid reason code");
    let diag = PluginDiagnostic::new(DiagnosticSeverity::Error, "symbol not found")
        .with_reason_code(reason);
    world.response = Some(PluginResponse::failure(vec![diag]));
}

#[given("an actuator manifest with capability {cap}")]
fn given_actuator_manifest_with_cap(world: &mut CapabilityWorld, cap: QuotedString) {
    let cap_id: CapabilityId =
        serde_json::from_str(&format!("\"{}\"", cap.as_str())).expect("valid capability id");
    let meta = PluginMetadata::new("test-plugin", "1.0", PluginKind::Actuator);
    world.manifest = Some(
        PluginManifest::new(meta, vec!["python".into()], PathBuf::from("/usr/bin/test"))
            .with_capabilities(vec![cap_id]),
    );
}

#[given("a sensor manifest with capability {cap}")]
fn given_sensor_manifest_with_cap(world: &mut CapabilityWorld, cap: QuotedString) {
    let cap_id: CapabilityId =
        serde_json::from_str(&format!("\"{}\"", cap.as_str())).expect("valid capability id");
    let meta = PluginMetadata::new("test-sensor", "1.0", PluginKind::Sensor);
    world.manifest = Some(
        PluginManifest::new(meta, vec!["python".into()], PathBuf::from("/usr/bin/test"))
            .with_capabilities(vec![cap_id]),
    );
}

#[given("contract version {major}.{minor}")]
fn given_version_a(world: &mut CapabilityWorld, major: u16, minor: u16) {
    world.version_a = Some(ContractVersion::new(major, minor));
}

#[given("a peer version {major}.{minor}")]
fn given_version_b(world: &mut CapabilityWorld, major: u16, minor: u16) {
    world.version_b = Some(ContractVersion::new(major, minor));
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the request is validated")]
fn when_validate_request(world: &mut CapabilityWorld) {
    let contract = world.contract.as_ref().expect("contract must be set");
    let request = world.request.as_ref().expect("request must be set");
    world.validation_result = Some(contract.validate_request(request));
}

#[when("the response is validated")]
fn when_validate_response(world: &mut CapabilityWorld) {
    let contract = world.contract.as_ref().expect("contract must be set");
    let response = world.response.as_ref().expect("response must be set");
    world.validation_result = Some(contract.validate_response(response));
}

#[when("the manifest is validated")]
fn when_validate_manifest(world: &mut CapabilityWorld) {
    let manifest = world.manifest.as_ref().expect("manifest must be set");
    world.validation_result = Some(manifest.validate());
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("validation succeeds")]
fn then_validation_succeeds(world: &mut CapabilityWorld) {
    let result = world
        .validation_result
        .as_ref()
        .expect("validation must have been run");
    assert!(
        result.is_ok(),
        "expected validation to succeed but got: {}",
        result.as_ref().expect_err("unreachable")
    );
}

#[then("validation fails with {substring}")]
fn then_validation_fails_with(world: &mut CapabilityWorld, substring: QuotedString) {
    let result = world
        .validation_result
        .as_ref()
        .expect("validation must have been run");
    let err = result
        .as_ref()
        .expect_err("expected validation to fail but it succeeded");
    let msg = err.to_string();
    assert!(
        msg.to_ascii_lowercase()
            .contains(&substring.as_str().to_ascii_lowercase()),
        "expected error to contain '{}' but got: {msg}",
        substring.as_str()
    );
}

#[then("the versions are compatible")]
fn then_versions_compatible(world: &mut CapabilityWorld) {
    let a = world.version_a.as_ref().expect("version_a must be set");
    let b = world.version_b.as_ref().expect("version_b must be set");
    assert!(
        a.is_compatible_with(b),
        "expected {a} to be compatible with {b}"
    );
}

#[then("the versions are incompatible")]
fn then_versions_incompatible(world: &mut CapabilityWorld) {
    let a = world.version_a.as_ref().expect("version_a must be set");
    let b = world.version_b.as_ref().expect("version_b must be set");
    assert!(
        !a.is_compatible_with(b),
        "expected {a} to be incompatible with {b}"
    );
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/capability_contract.feature")]
fn capability_contract_behaviour(world: CapabilityWorld) {
    let _ = world;
}
