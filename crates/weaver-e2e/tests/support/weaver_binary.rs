//! Resolves the workspace `weaver` binary for end-to-end tests.
//!
//! The resolution rules live in [`resolve_binary_with`], which performs no I/O
//! of its own: it consults a [`Probe`] holding an environment lookup, a
//! file-existence predicate, and a build runner. Production code supplies the
//! real collaborators; tests supply stubs, so every branch — including the
//! build fallback and its failure modes — is exercised without touching the
//! process environment or invoking `cargo`.

use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

#[cfg(test)]
#[path = "weaver_binary_tests.rs"]
mod tests;

/// Name of the environment variable Cargo sets for the `weaver` binary.
///
/// Cargo only sets this for the crate that owns the binary (`weaver-cli`), so
/// the lookup never succeeds from `weaver-e2e`; it is honoured anyway so the
/// resolver stays correct if the module is reused from `weaver-cli`.
const CARGO_BIN_EXE_VAR: &str = "CARGO_BIN_EXE_weaver";

/// Result of running the workspace build command to completion.
///
/// A non-zero exit is not an I/O error, so it is reported as data rather than
/// as `Err`; the exit status is captured as its rendered form because
/// `ExitStatus` cannot be constructed portably in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildOutcome {
    /// The build command exited successfully.
    Succeeded,
    /// The build command ran to completion but exited non-zero.
    Failed {
        /// The rendered exit status, as `ExitStatus` displays it.
        status: String,
    },
}

/// The effectful collaborators the resolution rules depend upon.
///
/// Bundling them keeps [`resolve_binary_with`] within the workspace argument
/// limit and makes the injection points explicit at every call site.
pub(crate) struct Probe<E, X, B> {
    /// Reads a process environment variable by name.
    pub(crate) env_var: E,
    /// Reports whether a path names an existing regular file.
    pub(crate) exists: X,
    /// Runs the workspace build that produces the `weaver` binary.
    pub(crate) build: B,
}

/// The locations searched for a prebuilt `weaver` binary, in probe order.
pub(crate) struct BinaryCandidates {
    /// The binary sitting beside the running test executable, that is, the
    /// active Cargo target directory with any trailing `deps` segment stripped.
    pub(crate) target_dir: PathBuf,
    /// The conventional `<workspace root>/target/debug/weaver` location, used
    /// when the tests run from a target directory that holds no CLI binary.
    pub(crate) target_debug: PathBuf,
}

impl BinaryCandidates {
    /// Derives both candidate paths from the running process and the workspace
    /// layout.
    ///
    /// # Errors
    /// Returns a description if the current executable path or the workspace
    /// root cannot be determined.
    fn from_environment() -> Result<Self, String> {
        Ok(Self {
            target_dir: target_dir_binary_path()?,
            target_debug: target_debug_binary_path()?,
        })
    }

    /// Returns the first candidate the predicate accepts, or `None` if neither
    /// exists yet.
    fn first_existing(&self, exists: &impl Fn(&Path) -> bool) -> Option<PathBuf> {
        [&self.target_dir, &self.target_debug]
            .into_iter()
            .find(|candidate| exists(candidate.as_path()))
            .cloned()
    }

    /// Describes both probed locations for inclusion in a failure message.
    fn describe(&self) -> String {
        format!(
            "checked {} and {}",
            self.target_dir.display(),
            self.target_debug.display()
        )
    }
}

/// Resolves the `weaver` binary used by e2e tests, building it if necessary.
///
/// This is not a pure lookup: when no prebuilt binary can be found, it shells
/// out to `cargo build -p weaver-cli --bin weaver`, writing build artefacts
/// into the workspace target directory. The build runs at most once per test
/// process because the outcome — success or failure — is cached, so every
/// caller sees the same answer and can decide how to report it.
///
/// The cache is per-process, not per-test: concurrent callers are safe because
/// `OnceLock` runs the resolver exactly once, which is what stops parallel test
/// threads racing to spawn competing `cargo build` invocations. The corollary
/// is that a failure is cached too, so one failed build poisons the remaining
/// tests in that binary; the next `cargo test` invocation starts afresh.
///
/// # Errors
/// Returns a description of why the binary could not be located or built.
pub(crate) fn resolve_or_build_weaver_binary_path() -> Result<&'static Path, &'static str> {
    static WEAVER_BINARY: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    memoized_binary_path(&WEAVER_BINARY, resolve_weaver_binary)
}

/// Runs `resolve` at most once per cell and borrows the cached outcome.
///
/// Both success and failure are cached deliberately: a failed build means the
/// workspace does not compile, which will not right itself mid-run, and
/// retrying would let every parallel test thread spawn its own `cargo build`.
///
/// # Errors
/// Returns the cached failure description when `resolve` failed.
pub(crate) fn memoized_binary_path(
    cell: &OnceLock<Result<PathBuf, String>>,
    resolve: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<&Path, &str> {
    match cell.get_or_init(resolve) {
        Ok(path) => Ok(path.as_path()),
        Err(error) => Err(error.as_str()),
    }
}

/// Applies the resolution rules with the real filesystem, environment, and
/// `cargo` collaborators.
///
/// # Errors
/// Returns a description of why the binary could not be located or built.
fn resolve_weaver_binary() -> Result<PathBuf, String> {
    let candidates = BinaryCandidates::from_environment()?;
    let probe = Probe {
        env_var: |name: &str| env::var_os(name),
        exists: |path: &Path| path.is_file(),
        build: run_workspace_build,
    };
    resolve_binary_with(&probe, &candidates)
}

/// Applies the resolution rules against the supplied collaborators.
///
/// The Cargo-provided path wins when it names an existing file; otherwise the
/// candidates are probed, the build is run once, and the candidates are probed
/// again.
///
/// # Errors
/// Returns a description when the build fails or when neither candidate exists
/// after a successful build.
pub(crate) fn resolve_binary_with<E, X, B>(
    probe: &Probe<E, X, B>,
    candidates: &BinaryCandidates,
) -> Result<PathBuf, String>
where
    E: Fn(&str) -> Option<OsString>,
    X: Fn(&Path) -> bool,
    B: Fn() -> Result<BuildOutcome, String>,
{
    let from_cargo = (probe.env_var)(CARGO_BIN_EXE_VAR)
        .map(PathBuf::from)
        .filter(|path| (probe.exists)(path.as_path()));
    if let Some(path) = from_cargo {
        return Ok(path);
    }

    if let Some(found) = candidates.first_existing(&probe.exists) {
        return Ok(found);
    }

    if let BuildOutcome::Failed { status } = (probe.build)()? {
        return Err(format!(
            "building workspace weaver binary failed with status {status}"
        ));
    }

    candidates.first_existing(&probe.exists).ok_or_else(|| {
        format!(
            "failed to locate built weaver binary after cargo build: {}",
            candidates.describe()
        )
    })
}

/// Locates `weaver` beside the running test executable.
///
/// # Errors
/// Returns a description if the current executable path is unavailable.
fn target_dir_binary_path() -> Result<PathBuf, String> {
    let mut target_dir =
        env::current_exe().map_err(|error| format!("current executable path: {error}"))?;
    target_dir.pop();
    if target_dir.ends_with("deps") {
        target_dir.pop();
    }

    Ok(target_dir.join(weaver_file_name()))
}

/// Locates `weaver` in the workspace's default debug target directory.
///
/// # Errors
/// Returns a description if the workspace root cannot be determined.
fn target_debug_binary_path() -> Result<PathBuf, String> {
    Ok(workspace_root()?
        .join("target")
        .join("debug")
        .join(weaver_file_name()))
}

/// Returns the platform-specific file name of the `weaver` executable.
fn weaver_file_name() -> String { format!("weaver{}", env::consts::EXE_SUFFIX) }

/// Returns the workspace root, derived from this crate's manifest directory.
///
/// # Errors
/// Returns a description if the manifest directory has no grandparent.
fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| String::from("workspace root should exist for e2e tests"))
}

/// Builds the workspace `weaver` binary with `cargo`.
///
/// # Errors
/// Returns a description if `cargo` could not be spawned; a non-zero exit is
/// reported as [`BuildOutcome::Failed`] rather than as an error.
fn run_workspace_build() -> Result<BuildOutcome, String> {
    let status = Command::new("cargo")
        .current_dir(workspace_root()?)
        .args(["build", "-p", "weaver-cli", "--bin", "weaver"])
        .status()
        .map_err(|error| format!("failed to build workspace weaver binary: {error}"))?;

    if status.success() {
        Ok(BuildOutcome::Succeeded)
    } else {
        Ok(BuildOutcome::Failed {
            status: status.to_string(),
        })
    }
}
