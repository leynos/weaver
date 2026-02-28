//! Unit tests for plugin manifest types.

use std::path::PathBuf;

use rstest::rstest;

use super::*;
use crate::capability::CapabilityId;
use crate::error::PluginError;
use crate::manifest::PluginMetadata;

// ---------------------------------------------------------------------------
// PluginKind
// ---------------------------------------------------------------------------

#[rstest]
#[case::sensor(PluginKind::Sensor, "sensor")]
#[case::actuator(PluginKind::Actuator, "actuator")]
fn kind_as_str(#[case] kind: PluginKind, #[case] expected: &str) {
    assert_eq!(kind.as_str(), expected);
}

#[rstest]
#[case::sensor(PluginKind::Sensor, "sensor")]
#[case::actuator(PluginKind::Actuator, "actuator")]
fn kind_display(#[case] kind: PluginKind, #[case] expected: &str) {
    assert_eq!(kind.to_string(), expected);
}

#[rstest]
#[case::sensor("\"sensor\"", PluginKind::Sensor)]
#[case::actuator("\"actuator\"", PluginKind::Actuator)]
fn kind_serde_round_trip(#[case] json: &str, #[case] expected: PluginKind) {
    let parsed: PluginKind = serde_json::from_str(json).expect("deserialise");
    assert_eq!(parsed, expected);
    let back = serde_json::to_string(&parsed).expect("serialise");
    assert_eq!(back, json);
}

// ---------------------------------------------------------------------------
// PluginManifest construction
// ---------------------------------------------------------------------------

fn make_manifest() -> PluginManifest {
    let meta = PluginMetadata::new("rope", "1.0.0", PluginKind::Actuator);
    PluginManifest::new(
        meta,
        vec!["python".into()],
        PathBuf::from("/usr/bin/rope-plugin"),
    )
}

#[test]
fn new_manifest_has_defaults() {
    let m = make_manifest();
    assert_eq!(m.name(), "rope");
    assert_eq!(m.version(), "1.0.0");
    assert_eq!(m.kind(), PluginKind::Actuator);
    assert_eq!(m.languages(), &["python"]);
    assert_eq!(m.executable(), PathBuf::from("/usr/bin/rope-plugin"));
    assert!(m.args().is_empty());
    assert_eq!(m.timeout_secs(), 30);
    assert!(m.capabilities().is_empty());
}

#[test]
fn with_args_sets_arguments() {
    let m = make_manifest().with_args(vec!["--verbose".into(), "--strict".into()]);
    assert_eq!(m.args(), &["--verbose", "--strict"]);
}

#[test]
fn with_timeout_overrides_default() {
    let m = make_manifest().with_timeout_secs(60);
    assert_eq!(m.timeout_secs(), 60);
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[test]
fn validate_accepts_valid_manifest() {
    let m = make_manifest();
    assert!(m.validate().is_ok());
}

#[rstest]
#[case::empty_name("  ", "/usr/bin/jedi", "name")]
#[case::relative_executable("rope", "relative/path/rope", "absolute")]
fn validate_rejects_invalid_manifest(
    #[case] name: &str,
    #[case] executable: &str,
    #[case] error_substring: &str,
) {
    let meta = PluginMetadata::new(name, "1.0", PluginKind::Sensor);
    let manifest = PluginManifest::new(meta, vec!["python".into()], PathBuf::from(executable));
    let err = manifest
        .validate()
        .expect_err("should reject invalid manifest");
    assert!(matches!(err, PluginError::Manifest { .. }));
    assert!(
        err.to_string().contains(error_substring),
        "expected '{error_substring}' in: {err}"
    );
}

// ---------------------------------------------------------------------------
// Serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn manifest_serde_round_trip() {
    let m = make_manifest()
        .with_args(vec!["--flag".into()])
        .with_timeout_secs(10);
    let json = serde_json::to_string(&m).expect("serialise");
    let back: PluginManifest = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, m);
}

#[test]
fn manifest_deserialise_defaults_timeout() {
    let json = r#"{
        "name": "test",
        "version": "0.1",
        "kind": "sensor",
        "languages": ["rust"],
        "executable": "/bin/test"
    }"#;
    let m: PluginManifest = serde_json::from_str(json).expect("deserialise");
    assert_eq!(m.timeout_secs(), 30);
    assert!(m.args().is_empty());
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

#[test]
fn with_capabilities_sets_capabilities() {
    let m = make_manifest().with_capabilities(vec![CapabilityId::RenameSymbol]);
    assert_eq!(m.capabilities(), &[CapabilityId::RenameSymbol]);
}

#[test]
fn manifest_with_capabilities_serde_round_trip() {
    let m = make_manifest().with_capabilities(vec![CapabilityId::RenameSymbol]);
    let json = serde_json::to_string(&m).expect("serialise");
    let back: PluginManifest = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back.capabilities(), &[CapabilityId::RenameSymbol]);
    assert_eq!(back, m);
}

#[test]
fn manifest_without_capabilities_deserialises_to_empty() {
    let json = r#"{
        "name": "test",
        "version": "0.1",
        "kind": "actuator",
        "languages": ["rust"],
        "executable": "/bin/test"
    }"#;
    let m: PluginManifest = serde_json::from_str(json).expect("deserialise");
    assert!(m.capabilities().is_empty());
}

#[test]
fn validate_rejects_sensor_with_capabilities() {
    let meta = PluginMetadata::new("jedi", "1.0", PluginKind::Sensor);
    let manifest = PluginManifest::new(meta, vec!["python".into()], PathBuf::from("/usr/bin/jedi"))
        .with_capabilities(vec![CapabilityId::RenameSymbol]);
    let err = manifest.validate().expect_err("should reject");
    assert!(matches!(err, PluginError::Manifest { .. }));
    assert!(err.to_string().contains("sensor plugins must not declare"));
}

#[test]
fn validate_accepts_actuator_with_capabilities() {
    let m = make_manifest().with_capabilities(vec![CapabilityId::RenameSymbol]);
    assert!(m.validate().is_ok());
}
