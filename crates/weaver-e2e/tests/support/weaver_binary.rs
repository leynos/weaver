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
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use cap_std::{ambient_authority, fs::Dir};

#[path = "weaver_binary_paths.rs"]
mod paths;

use self::paths::{run_workspace_build, target_debug_binary_path, target_dir_binary_path};

#[cfg(test)]
#[path = "weaver_binary_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
#[path = "weaver_binary_tests.rs"]
mod tests;

/// Describes a workspace binary the resolver can locate or build.
///
/// Parameterising the binary keeps the candidate probing, memoization, and
/// build fallback shared between `weaver` and `weaverd`; only the name, Cargo
/// package, and `CARGO_BIN_EXE_*` variable differ.
struct WorkspaceBinary {
    /// File name stem of the built executable.
    name: &'static str,
    /// Cargo package that owns the binary target.
    package: &'static str,
    /// Environment variable Cargo sets for the binary's path.
    ///
    /// Cargo sets this only for the crate that owns the binary, so the lookup
    /// normally fails from `weaver-e2e`; it is honoured anyway for reuse.
    env_var: &'static str,
}

impl WorkspaceBinary {
    /// Describes one workspace binary.
    const fn new(name: &'static str, package: &'static str, env_var: &'static str) -> Self {
        Self {
            name,
            package,
            env_var,
        }
    }

    /// Returns the Cargo arguments that build this binary.
    const fn build_arguments(&self) -> [&'static str; 5] {
        ["build", "-p", self.package, "--bin", self.name]
    }
}

/// The `weaver` CLI binary resolved by [`resolve_or_build_weaver_binary_path`].
const WEAVER_BINARY: WorkspaceBinary =
    WorkspaceBinary::new("weaver", "weaver-cli", "CARGO_BIN_EXE_weaver");

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
    ///
    /// The callback is fallible so that an inaccessible path — a permissions
    /// failure, say — is reported as an error rather than being flattened into
    /// "absent", which would send the resolver off to run a pointless build.
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
    /// layout for the binary named `binary_name`.
    ///
    /// # Errors
    /// Returns a description if the current executable path or the workspace
    /// root cannot be determined.
    fn from_environment(binary_name: &str) -> Result<Self, String> {
        Ok(Self {
            target_dir: target_dir_binary_path(binary_name)?,
            target_debug: target_debug_binary_path(binary_name)?,
        })
    }

    /// Returns the first candidate the predicate accepts, or `None` if neither
    /// exists yet.
    ///
    /// # Errors
    /// Returns the predicate's error as soon as one candidate cannot be
    /// inspected; later candidates are then left unprobed.
    fn first_existing(
        &self,
        exists: &impl Fn(&Path) -> Result<bool, String>,
    ) -> Result<Option<PathBuf>, String> {
        for candidate in [&self.target_dir, &self.target_debug] {
            if exists(candidate.as_path())? {
                return Ok(Some(candidate.clone()));
            }
        }
        Ok(None)
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
    static WEAVER_BINARY_PATH: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    memoized_binary_path(&WEAVER_BINARY_PATH, || {
        resolve_workspace_binary(
            WEAVER_BINARY.name,
            WEAVER_BINARY.package,
            WEAVER_BINARY.env_var,
        )
    })
}

/// Resolves any workspace binary, building it when no prebuilt copy exists.
///
/// This shares the candidate probing, build fallback, and failure reporting of
/// the `weaver` resolver with other workspace binaries such as `weaverd`.
///
/// # Errors
/// Returns a description of why the binary could not be located or built.
pub(crate) fn resolve_workspace_binary(
    name: &'static str,
    package: &'static str,
    env_var: &'static str,
) -> Result<PathBuf, String> {
    resolve_binary(&WorkspaceBinary::new(name, package, env_var))
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
/// `cargo` collaborators for `binary`.
///
/// # Errors
/// Returns a description of why the binary could not be located or built.
fn resolve_binary(binary: &WorkspaceBinary) -> Result<PathBuf, String> {
    let candidates = BinaryCandidates::from_environment(binary.name)?;
    let probe = Probe {
        env_var: |name: &str| env::var_os(name),
        exists: |path: &Path| is_regular_file(path, binary.name),
        build: || run_workspace_build(binary),
    };
    resolve_binary_with_named(&probe, &candidates, binary)
}

/// Reports whether `path` names an existing regular file.
///
/// This deliberately does not use [`Path::is_file`], which collapses every
/// metadata failure into `false`. Only [`ErrorKind::NotFound`] means "absent";
/// anything else — a permissions failure on the parent directory, for instance
/// — is a genuine filesystem error and is reported as one, so the resolver
/// never mistakes an unreadable binary for a missing one and never runs a
/// build that could not possibly fix the problem.
///
/// The stat goes through a [`Dir`] capability opened on the parent rather than
/// through `std::fs`, per the workspace's capability-based filesystem policy.
/// Opening the parent is itself the first half of the distinction: a parent
/// that does not exist means the candidate is absent, whereas a parent that
/// exists but cannot be opened is an error.
///
/// # Errors
/// Returns a description when the path has no parent or no file name, or when
/// the parent directory or the candidate's metadata cannot be read for any
/// reason other than not existing.
fn is_regular_file(path: &Path, binary_name: &str) -> Result<bool, String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "candidate {binary_name} binary has no parent: {}",
            path.display()
        )
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        format!(
            "candidate {binary_name} binary has no file name: {}",
            path.display()
        )
    })?;

    let dir = match Dir::open_ambient_dir(parent, ambient_authority()) {
        Ok(dir) => dir,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to open directory holding candidate {binary_name} binary {}: {error}",
                parent.display()
            ));
        }
    };

    match dir.metadata(file_name) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "failed to inspect candidate {binary_name} binary {}: {error}",
            path.display()
        )),
    }
}

/// Applies the resolution rules against the supplied collaborators.
///
/// The Cargo-provided path wins when it names an existing file; otherwise the
/// candidates are probed, the build is run once, and the candidates are probed
/// again.
///
/// # Errors
/// Returns a description when a candidate cannot be inspected, when the build
/// fails, or when neither candidate exists after a successful build. An
/// existence-probe failure short-circuits before the build runs.
pub(crate) fn resolve_binary_with<E, X, B>(
    probe: &Probe<E, X, B>,
    candidates: &BinaryCandidates,
) -> Result<PathBuf, String>
where
    E: Fn(&str) -> Option<OsString>,
    X: Fn(&Path) -> Result<bool, String>,
    B: Fn() -> Result<BuildOutcome, String>,
{
    resolve_binary_with_named(probe, candidates, &WEAVER_BINARY)
}

/// Applies the resolution rules for a named workspace binary.
///
/// # Errors
/// Returns a description when a candidate cannot be inspected, when the build
/// fails, or when neither candidate exists after a successful build.
fn resolve_binary_with_named<E, X, B>(
    probe: &Probe<E, X, B>,
    candidates: &BinaryCandidates,
    binary: &WorkspaceBinary,
) -> Result<PathBuf, String>
where
    E: Fn(&str) -> Option<OsString>,
    X: Fn(&Path) -> Result<bool, String>,
    B: Fn() -> Result<BuildOutcome, String>,
{
    let from_cargo = (probe.env_var)(binary.env_var).map(PathBuf::from);
    if let Some(path) = from_cargo
        && (probe.exists)(path.as_path())?
    {
        return Ok(path);
    }

    if let Some(found) = candidates.first_existing(&probe.exists)? {
        return Ok(found);
    }

    if let BuildOutcome::Failed { status } = (probe.build)()? {
        return Err(format!(
            "building workspace {} binary failed with status {status}",
            binary.name
        ));
    }

    candidates.first_existing(&probe.exists)?.ok_or_else(|| {
        format!(
            "failed to locate built {} binary after cargo build: {}",
            binary.name,
            candidates.describe()
        )
    })
}
