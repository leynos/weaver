//! Behaviour-driven tests for capability contract validation.

use std::{collections::HashMap, path::PathBuf, str::FromStr};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    capability::{
        CapabilityContract,
        CapabilityId,
        ContractVersion,
        ReasonCode,
        RenameSymbolContract,
    },
    error::PluginError,
    manifest::{PluginKind, PluginManifest, PluginMetadata},
    protocol::{DiagnosticSeverity, PluginDiagnostic, PluginOutput, PluginRequest, PluginResponse},
};

// ---------------------------------------------------------------------------
// Typed wrappers for Gherkin step parameters
// ---------------------------------------------------------------------------

/// A quoted string value from a Gherkin feature file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotedString(String);

impl FromStr for QuotedString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self(s.trim_matches('"').to_owned())) }
}

impl QuotedString {
    fn as_str(&self) -> &str { &self.0 }
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

type StepResult = Result<(), String>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> CapabilityWorld { CapabilityWorld::default() }

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

/// Builds a manifest whose single capability is supplied by a Gherkin step.
fn manifest_with_capability(
    name: &str,
    kind: PluginKind,
    capability: &QuotedString,
) -> Result<PluginManifest, String> {
    let capability_id: CapabilityId = serde_json::from_str(&format!("\"{}\"", capability.as_str()))
        .map_err(|error| format!("valid capability id: {error}"))?;
    let metadata = PluginMetadata::new(name, "1.0", kind);
    Ok(PluginManifest::new(
        metadata,
        vec!["python".into()],
        PathBuf::from("/usr/bin/test"),
    )
    .with_capabilities(vec![capability_id]))
}

/// Validates a contract target and stores the result for a later assertion step.
fn validate_contract_target<T>(
    world: &mut CapabilityWorld,
    get_target: impl FnOnce(&CapabilityWorld) -> Option<&T>,
    validate: impl FnOnce(&RenameSymbolContract, &T) -> Result<(), PluginError>,
    target_name: &str,
) -> StepResult {
    let result = {
        let contract = world
            .contract
            .as_ref()
            .ok_or_else(|| String::from("contract must be set"))?;
        let target_value = get_target(world).ok_or_else(|| format!("{target_name} must be set"))?;
        validate(contract, target_value)
    };
    world.validation_result = Some(result);
    Ok(())
}

/// Retrieves the two contract versions used by compatibility assertions.
fn version_pair(world: &CapabilityWorld) -> Result<(&ContractVersion, &ContractVersion), String> {
    let first = world
        .version_a
        .as_ref()
        .ok_or_else(|| String::from("version_a must be set"))?;
    let second = world
        .version_b
        .as_ref()
        .ok_or_else(|| String::from("version_b must be set"))?;
    Ok((first, second))
}

/// Checks whether the configured versions have the expected compatibility.
fn assert_version_compatibility(world: &CapabilityWorld, should_be_compatible: bool) -> StepResult {
    let (first, second) = version_pair(world)?;
    let are_compatible = first.is_compatible_with(second);
    if are_compatible == should_be_compatible {
        return Ok(());
    }
    let expectation = if should_be_compatible {
        "compatible"
    } else {
        "incompatible"
    };
    Err(format!(
        "expected {first} to be {expectation} with {second}"
    ))
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("a rename-symbol contract")]
fn given_contract(world: &mut CapabilityWorld) { world.contract = Some(RenameSymbolContract); }

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
fn given_failure_with_reason(world: &mut CapabilityWorld, code: QuotedString) -> StepResult {
    let reason: ReasonCode = serde_json::from_str(&format!("\"{}\"", code.as_str()))
        .map_err(|error| format!("valid reason code: {error}"))?;
    let diag = PluginDiagnostic::new(DiagnosticSeverity::Error, "symbol not found")
        .with_reason_code(reason);
    world.response = Some(PluginResponse::failure(vec![diag]));
    Ok(())
}

#[given("an actuator manifest with capability {cap}")]
fn given_actuator_manifest_with_cap(world: &mut CapabilityWorld, cap: QuotedString) -> StepResult {
    world.manifest = Some(manifest_with_capability(
        "test-plugin",
        PluginKind::Actuator,
        &cap,
    )?);
    Ok(())
}

#[given("a sensor manifest with capability {cap}")]
fn given_sensor_manifest_with_cap(world: &mut CapabilityWorld, cap: QuotedString) -> StepResult {
    world.manifest = Some(manifest_with_capability(
        "test-sensor",
        PluginKind::Sensor,
        &cap,
    )?);
    Ok(())
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
fn when_validate_request(world: &mut CapabilityWorld) -> StepResult {
    validate_contract_target(
        world,
        |state| state.request.as_ref(),
        RenameSymbolContract::validate_request,
        "request",
    )
}

#[when("the response is validated")]
fn when_validate_response(world: &mut CapabilityWorld) -> StepResult {
    validate_contract_target(
        world,
        |state| state.response.as_ref(),
        RenameSymbolContract::validate_response,
        "response",
    )
}

#[when("the manifest is validated")]
fn when_validate_manifest(world: &mut CapabilityWorld) -> StepResult {
    let manifest = world
        .manifest
        .as_ref()
        .ok_or_else(|| String::from("manifest must be set"))?;
    world.validation_result = Some(manifest.validate());
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("validation succeeds")]
fn then_validation_succeeds(world: &mut CapabilityWorld) -> StepResult {
    let result = world
        .validation_result
        .as_ref()
        .ok_or_else(|| String::from("validation must have been run"))?;
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(format!("expected validation to succeed but got: {error}")),
    }
}

#[then("validation fails with {substring}")]
fn then_validation_fails_with(world: &mut CapabilityWorld, substring: QuotedString) -> StepResult {
    let result = world
        .validation_result
        .as_ref()
        .ok_or_else(|| String::from("validation must have been run"))?;
    let err = result
        .as_ref()
        .err()
        .ok_or_else(|| String::from("expected validation to fail but it succeeded"))?;
    let msg = err.to_string();
    let contains_substring = msg
        .to_ascii_lowercase()
        .contains(&substring.as_str().to_ascii_lowercase());
    if !contains_substring {
        return Err(format!(
            "expected error to contain '{}': {}",
            substring.as_str(),
            msg
        ));
    }
    Ok(())
}

#[then("the versions are compatible")]
fn then_versions_compatible(world: &mut CapabilityWorld) -> StepResult {
    assert_version_compatibility(world, true)
}

#[then("the versions are incompatible")]
fn then_versions_incompatible(world: &mut CapabilityWorld) -> StepResult {
    assert_version_compatibility(world, false)
}

// ---------------------------------------------------------------------------
// parse_kv_pairs unit tests
// ---------------------------------------------------------------------------

#[test]
fn parse_kv_pairs_empty_input() {
    let result = parse_kv_pairs("");
    assert!(result.is_empty());
}

#[test]
fn parse_kv_pairs_multiple_pairs() {
    let result = parse_kv_pairs(r#"uri="file:///a.rs" position="1:0" new_name="bar""#);
    assert_eq!(result.len(), 3);
    assert_eq!(result.get("uri").expect("uri"), "file:///a.rs");
    assert_eq!(result.get("position").expect("position"), "1:0");
    assert_eq!(result.get("new_name").expect("new_name"), "bar");
}

#[test]
fn parse_kv_pairs_ignores_tokens_without_equals() {
    let result = parse_kv_pairs(r#"stray uri="ok""#);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get("uri").expect("uri"), "ok");
}

// ---------------------------------------------------------------------------
// Scenario registration
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/capability_contract.feature")]
fn capability_contract_behaviour(world: CapabilityWorld) { let _ = world; }
