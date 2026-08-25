//! Property-based checks on the resolver's memoization contract.
//!
//! [`super::memoized_binary_path`] promises two things to concurrent callers:
//! the wrapped resolver runs exactly once, and every caller borrows that one
//! outcome. The explicit cases in `weaver_binary_tests.rs` pin those promises
//! at a fixed thread count and two handwritten outcomes; the property here
//! sweeps a generated domain of successes, failures and caller counts so a
//! regression cannot hide in the gap between the handwritten cases.
//!
//! The racing harness lives here rather than beside the explicit tests because
//! both suites drive it; the explicit suite supplies its own thread count.

use std::{
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use proptest::{prelude::*, test_runner::FileFailurePersistence};

use super::memoized_binary_path;

/// Where shrunk counterexamples for this module are persisted.
///
/// Proptest's default policy hunts upwards for `lib.rs` or `main.rs` to anchor
/// a `proptest-regressions` directory. This module lives under `tests/`,
/// outside any crate source root, so that search fails noisily and falls back
/// to a sidecar file. Naming the sidecar outright yields the same location
/// without the warning. The path is relative to the crate root, which is the
/// working directory Cargo gives a test binary.
const REGRESSION_FILE: &str = "tests/support/weaver_binary_cache_tests.proptest-regressions";

/// Shared cache cell used by the concurrency cases.
type ResolutionCell = OnceLock<Result<PathBuf, String>>;

/// Fewest callers that can still race one another.
const MIN_CALLERS: usize = 2;
/// Most callers a generated case may spawn.
///
/// Each case spawns this many operating-system threads, so the ceiling stays
/// in single digits: the property is about the memoization contract, not about
/// scheduler saturation, and two threads already exercise the race.
const MAX_CALLERS: usize = 8;

/// Resolves through a fresh cell from `callers` threads at once.
///
/// Returns each thread's view of the cached outcome alongside the number of
/// times the resolver itself ran, which is the pair every memoization
/// assertion is phrased against.
///
/// # Panics
/// Panics if a racing thread panics, which would mean the memoizing shell
/// itself faulted.
pub(super) fn race_memoized(
    callers: usize,
    outcome: &Result<PathBuf, String>,
) -> (Vec<Result<PathBuf, String>>, usize) {
    let cell = ResolutionCell::new();
    let calls = AtomicUsize::new(0);

    let seen = thread::scope(|scope| {
        let handles: Vec<_> = (0..callers)
            .map(|_| scope.spawn(|| observe_cached(&cell, &calls, outcome)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("racing thread should not panic"))
            .collect::<Vec<_>>()
    });

    (seen, calls.load(Ordering::SeqCst))
}

/// Reads the cached outcome through the memoizing shell, counting how often the
/// resolver itself runs, and returns an owned copy of what this caller saw.
fn observe_cached(
    cell: &ResolutionCell,
    calls: &AtomicUsize,
    outcome: &Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    memoized_binary_path(cell, || {
        calls.fetch_add(1, Ordering::SeqCst);
        outcome.clone()
    })
    .map(Path::to_path_buf)
    .map_err(str::to_owned)
}

/// Generates an absolute path of one to three lower-case segments.
///
/// The segments are constructed rather than filtered: every draw from the
/// regular expression is already a legal path component, so the generation
/// budget is never spent on rejected candidates.
fn any_resolved_path() -> impl Strategy<Value = PathBuf> {
    proptest::collection::vec("[a-z][a-z0-9_]{0,7}", 1..=3).prop_map(|segments| {
        let mut path = PathBuf::from("/");
        path.extend(segments);
        path
    })
}

/// Generates a non-empty resolution failure description.
///
/// The wording mirrors the messages [`super::resolve_binary_with`] produces so
/// the generated failures stay representative of what the cache really holds.
fn any_failure_message() -> impl Strategy<Value = String> {
    "[a-z][a-z ]{0,31}".prop_map(|detail| format!("resolving weaver binary failed: {detail}"))
}

/// Generates either resolution outcome the cache is required to hold.
///
/// Failures are generated alongside successes because the cache deliberately
/// retains them: a failed build means the workspace does not compile, and
/// retrying would let every caller spawn a competing `cargo build`.
fn any_cached_outcome() -> impl Strategy<Value = Result<PathBuf, String>> {
    prop_oneof![
        any_resolved_path().prop_map(Ok::<PathBuf, String>),
        any_failure_message().prop_map(Err::<PathBuf, String>),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 256,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_FILE))),
        ..ProptestConfig::default()
    })]

    /// Whatever the resolver yields, and however many callers race for it, the
    /// resolver runs once and every caller borrows that same outcome.
    #[test]
    fn prop_racing_callers_share_one_cached_outcome(
        outcome in any_cached_outcome(),
        callers in MIN_CALLERS..=MAX_CALLERS,
    ) {
        let (seen, calls) = race_memoized(callers, &outcome);

        prop_assert_eq!(calls, 1, "the resolver should run exactly once");
        prop_assert_eq!(seen.len(), callers, "every caller should report a view");
        for observed in &seen {
            prop_assert_eq!(observed, &outcome, "callers disagreed, saw {:?}", seen);
        }
    }
}
