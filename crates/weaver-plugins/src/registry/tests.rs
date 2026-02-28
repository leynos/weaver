//! Unit tests for the plugin registry.

use std::path::PathBuf;

use rstest::{fixture, rstest};

use super::*;
use crate::error::PluginError;
use crate::manifest::{PluginKind, PluginManifest, PluginMetadata};

fn make_actuator(name: &str, lang: &str) -> PluginManifest {
    let meta = PluginMetadata::new(name, "1.0", PluginKind::Actuator);
    PluginManifest::new(
        meta,
        vec![lang.into()],
        PathBuf::from(format!("/usr/bin/{name}")),
    )
}

fn make_sensor(name: &str, lang: &str) -> PluginManifest {
    let meta = PluginMetadata::new(name, "1.0", PluginKind::Sensor);
    PluginManifest::new(
        meta,
        vec![lang.into()],
        PathBuf::from(format!("/usr/bin/{name}")),
    )
}

#[fixture]
fn populated_registry() -> PluginRegistry {
    let mut r = PluginRegistry::new();
    r.register(make_actuator("rope", "python"))
        .expect("register rope");
    r.register(make_sensor("jedi", "python"))
        .expect("register jedi");
    r.register(make_actuator("srgn", "rust"))
        .expect("register srgn");
    r
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_registry_is_empty() {
    let r = PluginRegistry::new();
    assert!(r.is_empty());
    assert_eq!(r.len(), 0);
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[test]
fn register_and_get() {
    let mut r = PluginRegistry::new();
    r.register(make_actuator("rope", "python"))
        .expect("register");
    assert_eq!(r.len(), 1);
    let m = r.get("rope").expect("get rope");
    assert_eq!(m.name(), "rope");
}

#[test]
fn register_rejects_duplicate() {
    let mut r = PluginRegistry::new();
    r.register(make_actuator("rope", "python"))
        .expect("first register");
    let err = r
        .register(make_actuator("rope", "python"))
        .expect_err("duplicate should fail");
    assert!(matches!(err, PluginError::Manifest { .. }));
    assert!(err.to_string().contains("already registered"));
}

#[test]
fn register_rejects_invalid_manifest() {
    let mut r = PluginRegistry::new();
    let meta = PluginMetadata::new("  ", "1.0", PluginKind::Sensor);
    let bad = PluginManifest::new(meta, vec!["python".into()], PathBuf::from("/usr/bin/jedi"));
    let err = r.register(bad).expect_err("should reject invalid manifest");
    assert!(matches!(err, PluginError::Manifest { .. }));
}

// ---------------------------------------------------------------------------
// Lookup
// ---------------------------------------------------------------------------

#[rstest]
fn get_returns_none_for_missing(populated_registry: PluginRegistry) {
    assert!(populated_registry.get("nonexistent").is_none());
}

#[rstest]
fn find_by_kind_actuators(populated_registry: PluginRegistry) {
    let actuators = populated_registry.find_by_kind(PluginKind::Actuator);
    assert_eq!(actuators.len(), 2);
    let names: Vec<&str> = actuators.iter().map(|m| m.name()).collect();
    assert!(names.contains(&"rope"));
    assert!(names.contains(&"srgn"));
}

#[rstest]
fn find_by_kind_sensors(populated_registry: PluginRegistry) {
    let sensors = populated_registry.find_by_kind(PluginKind::Sensor);
    assert_eq!(sensors.len(), 1);
    assert_eq!(sensors.first().expect("one sensor").name(), "jedi");
}

#[rstest]
#[case::lowercase("python")]
#[case::capitalised("Python")]
fn find_for_language_is_case_insensitive(populated_registry: PluginRegistry, #[case] query: &str) {
    let results = populated_registry.find_for_language(query);
    assert_eq!(results.len(), 2, "expected 2 plugins for '{query}'");
}

#[rstest]
fn find_actuator_for_language(populated_registry: PluginRegistry) {
    let actuators = populated_registry.find_actuator_for_language("python");
    assert_eq!(actuators.len(), 1);
    assert_eq!(actuators.first().expect("one actuator").name(), "rope");
}

#[rstest]
fn find_for_language_returns_empty_for_unknown(populated_registry: PluginRegistry) {
    let results = populated_registry.find_for_language("haskell");
    assert!(results.is_empty());
}

#[rstest]
fn len_reflects_registration_count(populated_registry: PluginRegistry) {
    assert_eq!(populated_registry.len(), 3);
    assert!(!populated_registry.is_empty());
}

// ---------------------------------------------------------------------------
// Capability-based lookup
// ---------------------------------------------------------------------------

fn make_actuator_with_capabilities(
    name: &str,
    lang: &str,
    caps: Vec<CapabilityId>,
) -> PluginManifest {
    let meta = PluginMetadata::new(name, "1.0", PluginKind::Actuator);
    PluginManifest::new(
        meta,
        vec![lang.into()],
        PathBuf::from(format!("/usr/bin/{name}")),
    )
    .with_capabilities(caps)
}

#[fixture]
fn capability_registry() -> PluginRegistry {
    let mut r = PluginRegistry::new();
    r.register(make_actuator_with_capabilities(
        "rope",
        "python",
        vec![CapabilityId::RenameSymbol],
    ))
    .expect("register rope");
    r.register(make_actuator_with_capabilities(
        "ra",
        "rust",
        vec![CapabilityId::RenameSymbol, CapabilityId::ExtricateSymbol],
    ))
    .expect("register ra");
    r.register(make_sensor("jedi", "python"))
        .expect("register jedi");
    r
}

#[rstest]
fn find_for_capability_returns_matching(capability_registry: PluginRegistry) {
    let results = capability_registry.find_for_capability(CapabilityId::RenameSymbol);
    assert_eq!(results.len(), 2);
    let names: Vec<&str> = results.iter().map(|m| m.name()).collect();
    assert!(names.contains(&"rope"));
    assert!(names.contains(&"ra"));
}

#[rstest]
fn find_for_capability_excludes_non_matching(capability_registry: PluginRegistry) {
    let results = capability_registry.find_for_capability(CapabilityId::ExtricateSymbol);
    assert_eq!(results.len(), 1);
    assert_eq!(results.first().expect("one plugin").name(), "ra");
}

#[rstest]
fn find_for_capability_returns_empty_for_unused(capability_registry: PluginRegistry) {
    let results = capability_registry.find_for_capability(CapabilityId::ExtractMethod);
    assert!(results.is_empty());
}

#[rstest]
fn find_for_language_and_capability_intersects(capability_registry: PluginRegistry) {
    let results =
        capability_registry.find_for_language_and_capability("python", CapabilityId::RenameSymbol);
    assert_eq!(results.len(), 1);
    assert_eq!(results.first().expect("one plugin").name(), "rope");
}

#[rstest]
fn find_for_language_and_capability_returns_empty_on_mismatch(capability_registry: PluginRegistry) {
    let results = capability_registry
        .find_for_language_and_capability("python", CapabilityId::ExtricateSymbol);
    assert!(results.is_empty());
}

#[rstest]
fn find_for_language_and_capability_is_case_insensitive(capability_registry: PluginRegistry) {
    let results =
        capability_registry.find_for_language_and_capability("Rust", CapabilityId::RenameSymbol);
    assert_eq!(results.len(), 1);
    assert_eq!(results.first().expect("one plugin").name(), "ra");
}
