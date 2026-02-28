//! Tests for [`EngineConfig`].

use crate::EngineConfig;

#[test]
fn default_config_has_expected_values() {
    let config = EngineConfig::default();
    assert_eq!(config.max_matches_per_rule(), 10_000);
    assert_eq!(config.max_capture_text_bytes(), 1_048_576);
    assert_eq!(config.max_deep_search_nodes(), 100_000);
    assert!(!config.enable_hcl());
}

#[test]
fn custom_config_construction() {
    let config = EngineConfig::new(500, 4096, 1000, true);
    assert_eq!(config.max_matches_per_rule(), 500);
    assert_eq!(config.max_capture_text_bytes(), 4096);
    assert_eq!(config.max_deep_search_nodes(), 1000);
    assert!(config.enable_hcl());
}

#[test]
fn config_equality() {
    let a = EngineConfig::default();
    let b = EngineConfig::default();
    assert_eq!(a, b);
}

#[test]
fn config_clone() {
    let original = EngineConfig::new(100, 200, 300, true);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}
