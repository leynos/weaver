//! Regression coverage for [`TestWorld::rebuild_host`]'s torn-state guarantee.
//!
//! A registration failure partway through a rebuild must leave the previously
//! built host, handle map and overrides untouched, so this suite drives a
//! duplicate-language failure and asserts nothing moved.

use std::collections::BTreeMap;

use rstest::rstest;
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::{
    capability::CapabilityKind,
    errors::LspHostError,
    language::Language,
    server::ServerCapabilitySet,
    tests::support::{ResponseSet, TestServerConfig, TestWorld},
};

/// Projects a [`CapabilityMatrix`] into a comparable map, since the type
/// itself does not implement `PartialEq` (its `LanguageCapabilities` values
/// do not derive it).
fn capability_projection(
    matrix: &CapabilityMatrix,
) -> BTreeMap<String, BTreeMap<String, CapabilityOverride>> {
    matrix
        .languages
        .iter()
        .map(|(language, caps)| (language.clone(), caps.overrides.clone()))
        .collect()
}

#[rstest]
fn rebuild_host_leaves_world_unchanged_when_registration_fails() {
    let config = vec![TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, true, true),
        responses: ResponseSet::default(),
        initialization_error: None,
    }];
    let mut baseline_overrides_matrix = CapabilityMatrix::default();
    baseline_overrides_matrix.set_override(
        Language::Rust.as_str(),
        CapabilityKind::References.key(),
        CapabilityOverride::Force,
    );
    let mut world = TestWorld::new(config, baseline_overrides_matrix.clone())
        .expect("stub server should register");

    // Successful rebuild first, to capture a baseline that a subsequent
    // failing rebuild must leave untouched.
    world
        .rebuild_host(baseline_overrides_matrix)
        .expect("initial rebuild should succeed");
    let baseline_overrides = capability_projection(&world.active_overrides());
    let baseline_calls = world.calls(Language::Rust);
    assert!(baseline_calls.is_some(), "expected a recorded Rust server");

    // A config with a duplicate language forces `register_language` to fail
    // partway through the rebuild loop.
    world.set_configs(vec![
        TestServerConfig {
            language: Language::Rust,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses: ResponseSet::default(),
            initialization_error: None,
        },
        TestServerConfig {
            language: Language::Rust,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses: ResponseSet::default(),
            initialization_error: None,
        },
    ]);

    let mut attempted_overrides_matrix = CapabilityMatrix::default();
    attempted_overrides_matrix.set_override(
        Language::Rust.as_str(),
        CapabilityKind::Diagnostics.key(),
        CapabilityOverride::Deny,
    );
    match world.rebuild_host(attempted_overrides_matrix) {
        Err(LspHostError::DuplicateLanguage { .. }) => {}
        other => panic!("expected duplicate language error, got {other:?}"),
    }

    // The world must be exactly as it was before the failing rebuild: same
    // overrides and the same recorded server, not a fresh, empty one.
    assert_eq!(
        capability_projection(&world.active_overrides()),
        baseline_overrides
    );
    assert_eq!(world.calls(Language::Rust), baseline_calls);
}
