//! Unit tests for the `weaver` binary resolution rules.
//!
//! Every collaborator is stubbed, so the tests never read the process
//! environment, touch the filesystem, or spawn `cargo`.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use rstest::rstest;

use super::{BinaryCandidates, BuildOutcome, Probe, memoized_binary_path, resolve_binary_with};

/// Boxed environment lookup backed by [`Stubs`].
type EnvStub<'a> = Box<dyn Fn(&str) -> Option<OsString> + 'a>;
/// Boxed file-existence predicate backed by [`Stubs`].
type ExistsStub<'a> = Box<dyn Fn(&Path) -> bool + 'a>;
/// Boxed build runner backed by [`Stubs`].
type BuildStub<'a> = Box<dyn Fn() -> Result<BuildOutcome, String> + 'a>;
/// A [`Probe`] assembled entirely from stubbed collaborators.
type StubProbe<'a> = Probe<EnvStub<'a>, ExistsStub<'a>, BuildStub<'a>>;
/// Shared cache cell used by the concurrency cases.
type ResolutionCell = OnceLock<Result<PathBuf, String>>;

/// Path Cargo would report through `CARGO_BIN_EXE_weaver`.
const CARGO_BIN: &str = "/cargo/target/debug/weaver";
/// Path beside the running test executable.
const TARGET_DIR_BIN: &str = "/run/target/debug/weaver";
/// Path under the workspace's default debug target directory.
const TARGET_DEBUG_BIN: &str = "/workspace/target/debug/weaver";

/// Builds the pair of candidate locations shared by every case.
fn candidates() -> BinaryCandidates {
    BinaryCandidates {
        target_dir: PathBuf::from(TARGET_DIR_BIN),
        target_debug: PathBuf::from(TARGET_DEBUG_BIN),
    }
}

/// Recorded stub behaviour for one resolution attempt.
///
/// The existence predicate answers from `existing_before_build` until the build
/// runner has been called, then from `existing_after_build`, which is what lets
/// a single case describe "missing, built, now present".
struct Stubs {
    /// Value the environment lookup returns for `CARGO_BIN_EXE_weaver`.
    env_value: Option<OsString>,
    /// Paths that exist before the build runs.
    existing_before_build: Vec<PathBuf>,
    /// Paths that exist once the build has run.
    existing_after_build: Vec<PathBuf>,
    /// Result the build runner yields.
    outcome: Result<BuildOutcome, String>,
    /// Number of times the build runner has been invoked.
    builds: AtomicUsize,
}

impl Stubs {
    /// Creates stubs where nothing exists and the build succeeds without
    /// producing anything.
    fn new() -> Self {
        Self {
            env_value: None,
            existing_before_build: Vec::new(),
            existing_after_build: Vec::new(),
            outcome: Ok(BuildOutcome::Succeeded),
            builds: AtomicUsize::new(0),
        }
    }

    /// Sets the value reported for `CARGO_BIN_EXE_weaver`.
    fn with_env(mut self, value: &str) -> Self {
        self.env_value = Some(OsString::from(value));
        self
    }

    /// Marks the given paths as existing both before and after the build.
    fn with_existing(mut self, paths: &[&str]) -> Self {
        self.existing_before_build = paths.iter().map(PathBuf::from).collect();
        self.existing_after_build
            .clone_from(&self.existing_before_build);
        self
    }

    /// Marks the given paths as existing only once the build has run.
    fn with_build_output(mut self, paths: &[&str]) -> Self {
        self.existing_after_build = paths.iter().map(PathBuf::from).collect();
        self
    }

    /// Sets the result the build runner yields.
    fn with_outcome(mut self, outcome: Result<BuildOutcome, String>) -> Self {
        self.outcome = outcome;
        self
    }

    /// Number of build invocations recorded so far.
    fn build_count(&self) -> usize { self.builds.load(Ordering::SeqCst) }

    /// Borrows the stubs as a [`Probe`] for the resolution rules.
    fn probe(&self) -> StubProbe<'_> {
        Probe {
            env_var: Box::new(|name: &str| self.env_lookup(name)),
            exists: Box::new(|path: &Path| self.path_exists(path)),
            build: Box::new(|| self.run_build()),
        }
    }

    /// Answers the environment lookup, honouring only the Cargo variable the
    /// resolver consults.
    fn env_lookup(&self, name: &str) -> Option<OsString> {
        (name == "CARGO_BIN_EXE_weaver")
            .then(|| self.env_value.clone())
            .flatten()
    }

    /// Reports whether the path exists, switching to the post-build view once
    /// the build runner has been called.
    fn path_exists(&self, path: &Path) -> bool {
        let known = if self.build_count() == 0 {
            &self.existing_before_build
        } else {
            &self.existing_after_build
        };
        known.iter().any(|candidate| candidate == path)
    }

    /// Records a build invocation and yields the configured result.
    fn run_build(&self) -> Result<BuildOutcome, String> {
        self.builds.fetch_add(1, Ordering::SeqCst);
        self.outcome.clone()
    }
}

/// Resolves against the stubs and returns the outcome.
fn resolve(stubs: &Stubs) -> Result<PathBuf, String> {
    resolve_binary_with(&stubs.probe(), &candidates())
}

#[test]
fn env_variable_wins_when_it_names_an_existing_file() {
    let stubs = Stubs::new()
        .with_env(CARGO_BIN)
        .with_existing(&[CARGO_BIN, TARGET_DIR_BIN]);

    let resolved = resolve(&stubs).expect("the Cargo-provided binary should resolve");

    assert_eq!(resolved, PathBuf::from(CARGO_BIN));
    assert_eq!(stubs.build_count(), 0, "no build should be needed");
}

#[test]
fn env_variable_is_ignored_when_the_file_is_missing() {
    let stubs = Stubs::new()
        .with_env(CARGO_BIN)
        .with_existing(&[TARGET_DIR_BIN]);

    let resolved = resolve(&stubs).expect("the probed candidate should resolve");

    assert_eq!(resolved, PathBuf::from(TARGET_DIR_BIN));
    assert_eq!(stubs.build_count(), 0, "no build should be needed");
}

#[rstest]
#[case::target_dir(TARGET_DIR_BIN)]
#[case::target_debug(TARGET_DEBUG_BIN)]
fn probed_candidate_resolves_without_building(#[case] existing: &str) {
    let stubs = Stubs::new().with_existing(&[existing]);

    let resolved = resolve(&stubs).expect("the probed candidate should resolve");

    assert_eq!(resolved, PathBuf::from(existing));
    assert_eq!(stubs.build_count(), 0, "no build should be needed");
}

#[rstest]
#[case::target_dir(TARGET_DIR_BIN)]
#[case::target_debug(TARGET_DEBUG_BIN)]
fn build_runs_once_when_both_candidates_are_missing(#[case] produced: &str) {
    let stubs = Stubs::new().with_build_output(&[produced]);

    let resolved = resolve(&stubs).expect("the freshly built binary should resolve");

    assert_eq!(resolved, PathBuf::from(produced));
    assert_eq!(stubs.build_count(), 1, "exactly one build should run");
}

#[test]
fn successful_build_that_produces_nothing_reports_both_probed_paths() {
    let stubs = Stubs::new();

    let error = resolve(&stubs).expect_err("resolution should fail when nothing was produced");

    assert_eq!(
        error,
        format!(
            "failed to locate built weaver binary after cargo build: checked {TARGET_DIR_BIN} and \
             {TARGET_DEBUG_BIN}"
        )
    );
    assert_eq!(stubs.build_count(), 1, "exactly one build should run");
}

#[test]
fn non_zero_build_status_is_reported_verbatim() {
    let stubs = Stubs::new().with_outcome(Ok(BuildOutcome::Failed {
        status: String::from("exit status: 101"),
    }));

    let error = resolve(&stubs).expect_err("a failed build should fail resolution");

    assert_eq!(
        error,
        "building workspace weaver binary failed with status exit status: 101"
    );
}

#[test]
fn build_spawn_failure_propagates() {
    let stubs = Stubs::new().with_outcome(Err(String::from(
        "failed to build workspace weaver binary: no such file",
    )));

    let error = resolve(&stubs).expect_err("an unspawnable build should fail resolution");

    assert_eq!(
        error,
        "failed to build workspace weaver binary: no such file"
    );
}

/// Number of threads racing the memoizing shell.
const RACING_THREADS: usize = 8;

/// Resolves through `cell` from several threads at once and returns each
/// thread's view of the cached outcome alongside the resolver call count.
fn race_memoized(outcome: &Result<PathBuf, String>) -> (Vec<Result<PathBuf, String>>, usize) {
    let cell = ResolutionCell::new();
    let calls = AtomicUsize::new(0);

    let seen = thread::scope(|scope| {
        let handles: Vec<_> = (0..RACING_THREADS)
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

#[rstest]
#[case::success(Ok(PathBuf::from(TARGET_DIR_BIN)))]
#[case::failure(Err(String::from(
    "building workspace weaver binary failed with status exit status: 101"
)))]
fn concurrent_callers_share_one_cached_outcome(#[case] outcome: Result<PathBuf, String>) {
    let (seen, calls) = race_memoized(&outcome);

    assert_eq!(calls, 1, "the resolver should run exactly once");
    assert!(
        seen.iter().all(|observed| *observed == outcome),
        "every thread should observe the cached outcome, saw {seen:?}"
    );
}
