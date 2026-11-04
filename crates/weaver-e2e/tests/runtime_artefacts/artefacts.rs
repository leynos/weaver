//! Filesystem, process, and cleanup helpers shared by the scenario steps.
//!
//! Every helper reports failures as a description rather than panicking, so the
//! scenario steps can attach their own context before the test fails.

use std::{io::ErrorKind as IoErrorKind, path::Path, process::Output};

use camino::Utf8PathBuf;
use cap_std::fs::Dir;
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use weaver_config::{Config, RuntimePaths, SocketEndpoint};

/// Description of why a scenario step could not run to completion.
pub(super) type ScenarioResult<T> = Result<T, String>;

/// Name of the legacy PID file shared by pre-upgrade releases.
pub(super) const LEGACY_PID_FILE: &str = "weaverd.pid";

/// Name of the legacy health snapshot shared by pre-upgrade releases.
pub(super) const LEGACY_HEALTH_FILE: &str = "weaverd.health";

/// Name of the legacy lock file shared by pre-upgrade releases.
pub(super) const LEGACY_LOCK_FILE: &str = "weaverd.lock";

/// PID recorded by the legacy decoy files.
///
/// The value exceeds `i32::MAX` on purpose: a lifecycle command that trusted
/// the decoy would fail PID validation rather than signal a real process.
pub(super) const DECOY_PID: u32 = u32::MAX - 1;

/// Best-effort cleanup for daemons started by the scenario.
///
/// Registered PIDs receive `SIGTERM` when the guard drops, so a failed
/// assertion does not leave a daemon running for the rest of the test run.
#[derive(Debug, Default)]
pub(super) struct DaemonGuard {
    /// PIDs that have not yet been reported as cleanly stopped.
    live: Vec<u32>,
}

impl DaemonGuard {
    /// Records a daemon that must be terminated if the guard drops.
    pub(super) fn register(&mut self, pid: u32) { self.live.push(pid); }

    /// Forgets a daemon that has already been stopped.
    pub(super) fn mark_stopped(&mut self, pid: u32) {
        self.live.retain(|candidate| *candidate != pid);
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        for pid in self.live.drain(..) {
            if let Ok(raw) = i32::try_from(pid) {
                // Best effort: the daemon may already have exited, and `Drop`
                // cannot report a cleanup failure.
                _ = kill(Pid::from_raw(raw), Signal::SIGTERM);
            }
        }
    }
}

/// Builds a Unix endpoint for `name` inside `runtime_dir`.
pub(super) fn unix_endpoint(runtime_dir: &Path, name: &str) -> ScenarioResult<SocketEndpoint> {
    let path = runtime_dir.join(name);
    let utf8 = Utf8PathBuf::from_path_buf(path).map_err(|unrepresentable| {
        format!(
            "socket path {} is not valid UTF-8",
            unrepresentable.display()
        )
    })?;
    Ok(SocketEndpoint::unix(utf8))
}

/// Derives the runtime paths Weaver reports for `endpoint`.
pub(super) fn runtime_paths(endpoint: &SocketEndpoint) -> ScenarioResult<RuntimePaths> {
    let config = Config {
        daemon_socket: endpoint.clone(),
        ..Config::default()
    };
    RuntimePaths::from_config_readonly(&config)
        .map_err(|error| format!("derive runtime paths: {error}"))
}

/// Returns the stdout and stderr of a successful command.
pub(super) fn expect_success(output: &Output, action: &str) -> ScenarioResult<(String, String)> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        return Ok((stdout, stderr));
    }
    Err(format!(
        "`{action}` failed with {}: stdout={stdout:?} stderr={stderr:?}",
        output.status
    ))
}

/// Reads the PID recorded in `paths`' endpoint-scoped PID file.
pub(super) fn read_pid(dir: &Dir, paths: &RuntimePaths) -> ScenarioResult<u32> {
    let contents = read_file(dir, paths.pid_file_name())?;
    let parsed = contents.trim().parse::<u32>().ok();
    parsed.ok_or_else(|| {
        format!(
            "PID file {} should contain a process ID, got {contents:?}",
            paths.pid_path().display()
        )
    })
}

/// Reads a runtime file by name.
pub(super) fn read_file(dir: &Dir, name: &Path) -> ScenarioResult<String> {
    dir.read_to_string(name)
        .map_err(|source| format!("read {}: {source}", name.display()))
}

/// Writes a runtime file by name.
pub(super) fn write_file(dir: &Dir, name: &Path, contents: &str) -> ScenarioResult<()> {
    dir.write(name, contents)
        .map_err(|source| format!("write {}: {source}", name.display()))
}

/// Reports whether `name` exists within the runtime directory.
fn artefact_exists(dir: &Dir, name: &Path) -> ScenarioResult<bool> {
    match dir.metadata(name) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == IoErrorKind::NotFound => Ok(false),
        Err(source) => Err(format!("inspect {}: {source}", name.display())),
    }
}

/// Fails unless `name` exists within the runtime directory.
pub(super) fn require_present(dir: &Dir, name: &Path, description: &str) -> ScenarioResult<()> {
    if artefact_exists(dir, name)? {
        return Ok(());
    }
    Err(format!("{description} should exist at {}", name.display()))
}

/// Fails when `name` exists within the runtime directory.
pub(super) fn require_absent(dir: &Dir, name: &Path, description: &str) -> ScenarioResult<()> {
    if artefact_exists(dir, name)? {
        return Err(format!(
            "{description} should not exist at {}",
            name.display()
        ));
    }
    Ok(())
}

/// Fails unless `haystack` contains `needle`.
pub(super) fn require_text(haystack: &str, needle: &str, context: &str) -> ScenarioResult<()> {
    if haystack.contains(needle) {
        return Ok(());
    }
    Err(format!(
        "{context} should mention {needle:?}, got: {haystack:?}"
    ))
}

/// Reports whether a process with `pid` is still signalable.
pub(super) fn process_is_alive(pid: u32) -> bool {
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    kill(Pid::from_raw(raw), None).is_ok()
}
