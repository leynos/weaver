//! Behaviour-driven tests for capability contract validation.

use std::{collections::HashMap, path::PathBuf, str::FromStr};

use anyhow::{Context as _, Result, ensure};
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

/// Parses a Gherkin-quoted token as a JSON string literal of type `T`.
///
/// Capability ids and reason codes are both `serde` string enums, so one
/// helper covers each without duplicating the quoting dance.
fn parse_quoted_enum<T: serde::de::DeserializeOwned>(
    token: &QuotedString,
) -> serde_json::Result<T> {
    serde_json::from_str(&format!("\"{}\"", token.as_str()))
}

fn manifest_with_cap(
    cap: &QuotedString,
    name: &str,
    kind: PluginKind,
) -> serde_json::Result<PluginManifest> {
    let capability_id: CapabilityId = parse_quoted_enum(cap)?;
    let metadata = PluginMetadata::new(name, "1.0", kind);
    Ok(PluginManifest::new(
        metadata,
        vec!["python".into()],
        PathBuf::from("/usr/bin/test"),
    )
    .with_capabilities(vec![capability_id]))
}

/// Borrows the pair of contract versions recorded by the given steps.
fn recorded_versions(world: &CapabilityWorld) -> Result<(&ContractVersion, &ContractVersion)> {
    let version_a = world.version_a.as_ref().context("version_a must be set")?;
    let version_b = world.version_b.as_ref().context("version_b must be set")?;
    Ok((version_a, version_b))
}

fn ensure_versions_compatible(world: &CapabilityWorld, expected: bool) -> Result<()> {
    let (version_a, version_b) = recorded_versions(world)?;
    let expected_compatibility = if expected {
        "compatible"
    } else {
        "incompatible"
    };
    ensure!(
        version_a.is_compatible_with(version_b) == expected,
        "expected {version_a} to be {expected_compatibility} with {version_b}"
    );
    Ok(())
}

/// Borrows the validation outcome recorded by the when steps.
fn recorded_validation(world: &CapabilityWorld) -> Result<&Result<(), PluginError>> {
    world
        .validation_result
        .as_ref()
        .context("validation must have been run")
}

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
fn given_failure_with_reason(world: &mut CapabilityWorld, code: QuotedString) -> Result<()> {
    let reason: ReasonCode = parse_quoted_enum(&code).context("parsing reason code")?;
    let diag = PluginDiagnostic::new(DiagnosticSeverity::Error, "symbol not found")
        .with_reason_code(reason);
    world.response = Some(PluginResponse::failure(vec![diag]));
    Ok(())
}

#[given("an actuator manifest with capability {cap}")]
fn given_actuator_manifest_with_cap(world: &mut CapabilityWorld, cap: QuotedString) -> Result<()> {
    world.manifest = Some(
        manifest_with_cap(&cap, "test-plugin", PluginKind::Actuator)
            .context("parsing actuator capability id")?,
    );
    Ok(())
}

#[given("a sensor manifest with capability {cap}")]
fn given_sensor_manifest_with_cap(world: &mut CapabilityWorld, cap: QuotedString) -> Result<()> {
    world.manifest = Some(
        manifest_with_cap(&cap, "test-sensor", PluginKind::Sensor)
            .context("parsing sensor capability id")?,
    );
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
fn when_validate_request(world: &mut CapabilityWorld) -> Result<()> {
    let contract = world.contract.as_ref().context("contract must be set")?;
    let request = world.request.as_ref().context("request must be set")?;
    world.validation_result = Some(contract.validate_request(request));
    Ok(())
}

#[when("the response is validated")]
fn when_validate_response(world: &mut CapabilityWorld) -> Result<()> {
    let contract = world.contract.as_ref().context("contract must be set")?;
    let response = world.response.as_ref().context("response must be set")?;
    world.validation_result = Some(contract.validate_response(response));
    Ok(())
}

#[when("the manifest is validated")]
fn when_validate_manifest(world: &mut CapabilityWorld) -> Result<()> {
    let manifest = world.manifest.as_ref().context("manifest must be set")?;
    world.validation_result = Some(manifest.validate());
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("validation succeeds")]
fn then_validation_succeeds(world: &mut CapabilityWorld) -> Result<()> {
    let result = recorded_validation(world)?;
    ensure!(
        result.is_ok(),
        "expected validation to succeed but got: {result:?}"
    );
    Ok(())
}

#[then("validation fails with {substring}")]
fn then_validation_fails_with(world: &mut CapabilityWorld, substring: QuotedString) -> Result<()> {
    let Err(error) = recorded_validation(world)? else {
        anyhow::bail!("expected validation to fail but it succeeded");
    };
    let message = error.to_string();
    ensure!(
        message
            .to_ascii_lowercase()
            .contains(&substring.as_str().to_ascii_lowercase()),
        "expected error to contain '{}' but got: {message}",
        substring.as_str()
    );
    Ok(())
}

#[then("the versions are compatible")]
fn then_versions_compatible(world: &mut CapabilityWorld) -> Result<()> {
    ensure_versions_compatible(world, true)
}

#[then("the versions are incompatible")]
fn then_versions_incompatible(world: &mut CapabilityWorld) -> Result<()> {
    ensure_versions_compatible(world, false)
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
