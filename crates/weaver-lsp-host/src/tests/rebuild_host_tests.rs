//! Regression coverage for [`TestWorld::rebuild_host`]'s torn-state guarantee.
//!
//! A registration failure partway through a rebuild must leave the previously
//! built host, handle map and overrides untouched, so this suite drives
//! duplicate-language failures at each position of a three-server config and
//! asserts nothing moved.

use std::collections::BTreeMap;

use rstest::rstest;
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::{
    capability::CapabilityKind,
    errors::LspHostError,
    language::Language,
    server::ServerCapabilitySet,
    tests::support::{CallKind, ResponseSet, TestServerConfig, TestWorld},
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

/// Builds a stub server config that always initializes successfully.
fn server_config(language: Language) -> TestServerConfig {
    TestServerConfig {
        language,
        capabilities: ServerCapabilitySet::new(true, true, true),
        responses: ResponseSet::default(),
        initialization_error: None,
    }
}

/// Builds a matrix carrying a single override for Rust.
fn rust_override(kind: CapabilityKind, value: CapabilityOverride) -> CapabilityMatrix {
    let mut matrix = CapabilityMatrix::default();
    matrix.set_override(Language::Rust.as_str(), kind.key(), value);
    matrix
}

/// Drives a failing rebuild and asserts the world is byte-for-byte as before.
///
/// The `languages` cases vary which slot of a three-server config carries the
/// repeated language, so the preservation invariant is checked at every
/// position where `register_language` can reject the rebuild: after one
/// successful registration, after two adjacent ones, and after two
/// registrations separated by an unrelated language.
#[rstest]
#[case::duplicate_in_first_and_second_slots(&[
    Language::Rust,
    Language::Rust,
    Language::Python,
])]
#[case::duplicate_in_second_and_third_slots(&[
    Language::Rust,
    Language::Python,
    Language::Python,
])]
#[case::duplicate_in_first_and_third_slots(&[
    Language::Rust,
    Language::Python,
    Language::Rust,
])]
fn rebuild_host_leaves_world_unchanged_when_registration_fails(#[case] languages: &[Language]) {
    let baseline_overrides_matrix =
        rust_override(CapabilityKind::References, CapabilityOverride::Force);
    let mut world = TestWorld::new(
        vec![server_config(Language::Rust)],
        baseline_overrides_matrix.clone(),
    )
    .expect("stub server should register");

    // Successful rebuild first, to capture a baseline that a subsequent
    // failing rebuild must leave untouched.
    world
        .rebuild_host(baseline_overrides_matrix)
        .expect("initial rebuild should succeed");

    // Drive a recorded operation so the baseline call history is non-empty;
    // otherwise the post-failure comparison would hold two empty vectors and
    // pass even if the rebuild discarded the recording server entirely.
    world.initialize(Language::Rust);
    assert!(
        world.last_error.is_none(),
        "baseline initialize should succeed, got {:?}",
        world.last_error
    );

    let baseline_overrides = capability_projection(&world.active_overrides());
    let baseline_calls = world.calls(Language::Rust);
    assert_eq!(
        baseline_calls,
        Some(vec![CallKind::Initialise]),
        "baseline call history should record the initialize"
    );
    // Read back from the real host, not just TestWorld's own bookkeeping.
    let baseline_capabilities = world.host.capabilities(Language::Rust);
    assert!(
        baseline_capabilities.is_some(),
        "an initialized Rust session should expose resolved capabilities"
    );

    // A config with a duplicate language forces `register_language` to fail
    // partway through the rebuild loop.
    world.set_configs(languages.iter().copied().map(server_config).collect());

    let attempted_overrides_matrix =
        rust_override(CapabilityKind::Diagnostics, CapabilityOverride::Deny);
    match world.rebuild_host(attempted_overrides_matrix) {
        Err(LspHostError::DuplicateLanguage { .. }) => {}
        other => panic!("expected duplicate language error, got {other:?}"),
    }

    // The world must be exactly as it was before the failing rebuild: same
    // overrides, the same recorded server with its history intact, and the
    // same live host session rather than a fresh, uninitialized one.
    assert_eq!(
        capability_projection(&world.active_overrides()),
        baseline_overrides
    );
    assert_eq!(world.calls(Language::Rust), baseline_calls);
    assert_eq!(
        world.host.capabilities(Language::Rust),
        baseline_capabilities
    );
}
