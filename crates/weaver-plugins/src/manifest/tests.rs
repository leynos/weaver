//! Unit tests for plugin manifest types.

use std::path::PathBuf;

use rstest::rstest;

use super::*;
use crate::error::PluginError;

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
    PluginManifest::new(
        "rope",
        "1.0.0",
        PluginKind::Actuator,
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

#[test]
fn validate_rejects_empty_name() {
    let m = PluginManifest::new(
        "  ",
        "1.0",
        PluginKind::Sensor,
        vec!["python".into()],
        PathBuf::from("/usr/bin/jedi"),
    );
    let err = m.validate().expect_err("should reject empty name");
    assert!(matches!(err, PluginError::Manifest { .. }));
    assert!(err.to_string().contains("name"));
}

#[test]
fn validate_rejects_relative_executable() {
    let m = PluginManifest::new(
        "rope",
        "1.0",
        PluginKind::Actuator,
        vec!["python".into()],
        PathBuf::from("relative/path/rope"),
    );
    let err = m.validate().expect_err("should reject relative path");
    assert!(matches!(err, PluginError::Manifest { .. }));
    assert!(err.to_string().contains("absolute"));
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
