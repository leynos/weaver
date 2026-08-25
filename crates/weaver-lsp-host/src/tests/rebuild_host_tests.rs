//! Regression coverage for [`TestWorld::rebuild_host`]'s torn-state guarantee.
//!
//! A registration failure partway through a rebuild must leave the previously
//! built host, handle map and overrides untouched. Two suites assert this: an
//! explicit rstest matrix that drives duplicate-language failures at each
//! position of a three-server config, and a bounded proptest property that
//! generalizes the same invariant over short config sequences which repeat a
//! language by construction.

use std::collections::BTreeMap;

use proptest::prelude::*;
use rstest::rstest;
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::{
    capability::{CapabilityKind, CapabilitySummary},
    errors::LspHostError,
    language::Language,
    server::ServerCapabilitySet,
    tests::support::{CallKind, ResponseSet, TestServerConfig, TestWorld},
};

/// Languages the host supports, and therefore the alphabet the property test
/// draws generated server configurations from.
static SUPPORTED_LANGUAGES: [Language; 3] =
    [Language::Rust, Language::Python, Language::TypeScript];

/// Upper bound on the filler entries surrounding the deliberate duplicate,
/// keeping every generated rebuild sequence to at most four servers.
const MAX_FILLER: usize = 2;

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

/// Snapshot of the observable world state that a failed rebuild must preserve.
struct Baseline {
    /// Capability overrides applied to the host when the snapshot was taken.
    overrides: BTreeMap<String, BTreeMap<String, CapabilityOverride>>,
    /// Call history recorded by the Rust stub server.
    calls: Option<Vec<CallKind>>,
    /// Capabilities the real host resolved for the initialized Rust session.
    capabilities: Option<CapabilitySummary>,
}

/// Builds an initialized single-server world alongside the snapshot that a
/// subsequent failing rebuild must leave untouched.
///
/// The world is driven through a successful rebuild and then an `initialize`
/// so the recorded call history is non-empty; comparing two empty histories
/// would pass even if the rebuild discarded the recording server entirely.
fn baseline_world() -> Result<(TestWorld, Baseline), TestCaseError> {
    let baseline_overrides = rust_override(CapabilityKind::References, CapabilityOverride::Force);
    let mut world = TestWorld::new(
        vec![server_config(Language::Rust)],
        baseline_overrides.clone(),
    )
    .map_err(|error| TestCaseError::fail(format!("stub server should register: {error}")))?;

    // Rebuild once so the baseline is itself the product of a rebuild.
    world
        .rebuild_host(baseline_overrides)
        .map_err(|error| TestCaseError::fail(format!("initial rebuild should succeed: {error}")))?;

    world.initialize(Language::Rust);
    prop_assert!(
        world.last_error.is_none(),
        "baseline initialize should succeed, got {:?}",
        world.last_error
    );

    let calls = world.calls(Language::Rust);
    prop_assert_eq!(
        calls.clone(),
        Some(vec![CallKind::Initialise]),
        "baseline call history should record the initialize"
    );

    // Read back from the real host, not just TestWorld's own bookkeeping.
    let capabilities = world.host.capabilities(Language::Rust);
    prop_assert!(
        capabilities.is_some(),
        "an initialized Rust session should expose resolved capabilities"
    );

    let baseline = Baseline {
        overrides: capability_projection(&world.active_overrides()),
        calls,
        capabilities,
    };
    Ok((world, baseline))
}

/// Drives a rebuild over a config list that repeats a language and requires it
/// to fail with the duplicate-language error rather than any other outcome.
///
/// The attempted overrides differ from the baseline so a swap-on-failure bug
/// is observable through [`TestWorld::active_overrides`].
fn rebuild_must_fail_on_duplicate(
    world: &mut TestWorld,
    languages: &[Language],
) -> Result<(), TestCaseError> {
    world.set_configs(languages.iter().copied().map(server_config).collect());
    let attempted = rust_override(CapabilityKind::Diagnostics, CapabilityOverride::Deny);
    match world.rebuild_host(attempted) {
        Err(LspHostError::DuplicateLanguage { .. }) => Ok(()),
        other => Err(TestCaseError::fail(format!(
            "expected duplicate language error, got {other:?}"
        ))),
    }
}

/// Asserts the world still matches the snapshot taken before a failed rebuild.
///
/// All three facets are checked: the overrides, the recorded call history, and
/// the live [`crate::LspHost`] session rather than a fresh, uninitialized one.
fn assert_world_preserved(world: &TestWorld, baseline: &Baseline) -> Result<(), TestCaseError> {
    prop_assert_eq!(
        capability_projection(&world.active_overrides()),
        baseline.overrides.clone(),
        "a failed rebuild must not adopt the attempted overrides"
    );
    prop_assert_eq!(
        world.calls(Language::Rust),
        baseline.calls.clone(),
        "a failed rebuild must retain the original recording server and its history"
    );
    prop_assert_eq!(
        world.host.capabilities(Language::Rust),
        baseline.capabilities.clone(),
        "a failed rebuild must leave the live host session initialized"
    );
    Ok(())
}

/// Drives a failing rebuild and asserts the world is exactly as before.
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
fn rebuild_host_leaves_world_unchanged_when_registration_fails(
    #[case] languages: &[Language],
) -> Result<(), TestCaseError> {
    let (mut world, baseline) = baseline_world()?;
    rebuild_must_fail_on_duplicate(&mut world, languages)?;
    assert_world_preserved(&world, &baseline)
}

/// Generates a single language drawn from the supported set.
fn any_language() -> impl Strategy<Value = Language> {
    proptest::sample::select(SUPPORTED_LANGUAGES.as_slice())
}

/// Generates a bounded config sequence that repeats a language by construction.
///
/// One language is chosen and inserted twice, at independently generated
/// positions, into a run of at most [`MAX_FILLER`] filler languages; sequences
/// are therefore two to four entries long and always reach a duplicate
/// registration. Construction is preferred over `prop_filter` because a filter
/// would reject most draws, wasting the generation budget and risking
/// proptest's local rejection limit.
fn duplicate_language_sequence() -> impl Strategy<Value = Vec<Language>> {
    (
        any_language(),
        proptest::collection::vec(any_language(), 0..=MAX_FILLER),
    )
        .prop_flat_map(|(duplicate, filler)| {
            let len = filler.len();
            (Just(duplicate), Just(filler), 0..=len, 0..=len + 1)
        })
        .prop_map(|(duplicate, mut sequence, first, second)| {
            sequence.insert(first, duplicate);
            sequence.insert(second, duplicate);
            sequence
        })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 256,
        ..ProptestConfig::default()
    })]

    /// Every failing rebuild, at any duplicate position within a bounded
    /// config sequence, leaves the overrides, call history and live host
    /// session exactly as they were.
    #[test]
    fn prop_failed_rebuild_preserves_world(languages in duplicate_language_sequence()) {
        let (mut world, baseline) = baseline_world()?;
        rebuild_must_fail_on_duplicate(&mut world, &languages)?;
        assert_world_preserved(&world, &baseline)?;
    }
}
