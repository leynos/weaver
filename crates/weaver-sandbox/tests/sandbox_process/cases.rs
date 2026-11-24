//! The cases behind `sandbox_process`.

use std::{
    env,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use anyhow::{Context as _, Result, bail, ensure};
use cap_std::{ambient_authority, fs::Dir};
use tempfile::TempDir;
use weaver_sandbox::{
    Sandbox,
    SandboxCommand,
    SandboxError,
    SandboxOutput,
    SandboxProfile,
    process::Stdio,
};

/// Scenario variables the parent hands to every child.
pub const SCENARIO_ENV: [(&str, &str); 2] = [("KEEP_ME", "present"), ("DROP_ME", "remove-me")];

/// One named case of the target.
pub type Case = (&'static str, fn() -> Result<()>);

/// Every case, in listing order.
pub const CASES: &[Case] = &[
    ("allowed_file_is_readable", allowed_file_is_readable),
    (
        "disallowed_file_access_is_blocked",
        disallowed_file_access_is_blocked,
    ),
    (
        "environment_allowlist_is_inherited",
        environment_allowlist_is_inherited,
    ),
    (
        "environment_is_isolated_by_default",
        environment_is_isolated_by_default,
    ),
    (
        "full_environment_can_be_inherited",
        full_environment_can_be_inherited,
    ),
    (
        "parent_environment_is_restored_after_spawn",
        parent_environment_is_restored_after_spawn,
    ),
    ("relative_program_is_rejected", relative_program_is_rejected),
    ("missing_program_is_rejected", missing_program_is_rejected),
    ("unlisted_program_is_rejected", unlisted_program_is_rejected),
    (
        "multithreaded_process_is_rejected",
        multithreaded_process_is_rejected,
    ),
];

/// Fixture files the sandboxed `cat` reads.
struct Fixture {
    /// Keeps the directory alive for the case's lifetime.
    _dir: TempDir,
    /// A file the sandbox profile may whitelist.
    allowed: PathBuf,
    /// A file the sandbox profile never mentions.
    forbidden: PathBuf,
}

/// Writes the allowed and forbidden fixture files.
fn fixture() -> Result<Fixture> {
    let dir = TempDir::new().context("allocate a temporary directory")?;
    let allowed = dir.path().join("allowed.txt");
    let forbidden = dir.path().join("forbidden.txt");
    let root = Dir::open_ambient_dir(dir.path(), ambient_authority())
        .context("open the temporary directory")?;
    root.write("allowed.txt", "allowed file content")
        .context("write the allowed file")?;
    root.write("forbidden.txt", "forbidden file content")
        .context("write the forbidden file")?;
    Ok(Fixture {
        _dir: dir,
        allowed,
        forbidden,
    })
}

/// Returns the canonical path of the first candidate binary on the host.
///
/// `Sandbox::spawn` whitelists the canonical path but executes the path it was
/// given, and a merged-`/usr` host's `/bin` does not exist inside the new root,
/// so the commands below must name the canonical path.
fn resolve_binary(candidates: &[&str]) -> Result<PathBuf> {
    let found = candidates
        .iter()
        .map(Path::new)
        .find(|path| path.exists())
        .with_context(|| format!("no candidate binary found in {candidates:?}"))?;
    let root = Dir::open_ambient_dir("/", ambient_authority()).context("open the root")?;
    let relative = found.strip_prefix("/").unwrap_or(found);
    let resolved = root
        .canonicalize(relative)
        .with_context(|| format!("canonicalise {}", found.display()))?;
    Ok(Path::new("/").join(resolved))
}

/// Raised when the host cannot run a dynamically linked program in the
/// sandbox, so the case has nothing to observe.
#[derive(Debug)]
pub struct Skipped(String);

impl std::fmt::Display for Skipped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}

impl std::error::Error for Skipped {}

/// Spawns `command` under `profile`, naming the rejection in the error.
fn spawn(profile: SandboxProfile, command: SandboxCommand) -> Result<weaver_sandbox::SandboxChild> {
    Sandbox::new(profile)
        .spawn(command)
        .map_err(|err| anyhow::anyhow!("the sandbox rejected the spawn: {err}"))
}

/// Names the variable that opts the cases that run a program into the run.
const EXEC_OPT_IN: &str = "WEAVER_SANDBOX_EXEC_CASES";

/// Opts the cases that launch a dynamically linked program in or out.
///
/// They need a host whose sandbox can start one, and neither a merged-`/usr`
/// host (the new root lacks the `/lib64` interpreter symlink, so `execve`
/// fails with ENOENT) nor a host that forbids unprivileged user namespaces
/// (hosted Ubuntu images; every start fails with EPERM) is one. `birdcage`
/// reports those failures and unrelated ones with the same message, so the
/// cases do not guess from it: they run only when `WEAVER_SANDBOX_EXEC_CASES=1`
/// is set, and then every failure is a failure. Unset, they skip, loudly.
fn require_exec() -> Result<()> {
    if env::var_os(EXEC_OPT_IN).is_some_and(|value| value == "1") {
        return Ok(());
    }
    let reason = format!("set {EXEC_OPT_IN}=1 on a host whose sandbox can exec a dynamic binary");
    Err(Skipped(reason).into())
}

/// Spawns `command` under `profile` with piped output and waits for it.
fn launch(profile: SandboxProfile, command: SandboxCommand) -> Result<SandboxOutput> {
    require_exec()?;
    spawn(profile, command)?
        .wait_with_output()
        .context("read the child's output")
}

/// Runs `cat target` in a sandbox that may read `readable`, if any.
fn run_cat(target: &Path, readable: Option<&Path>) -> Result<SandboxOutput> {
    let program = resolve_binary(&["/bin/cat", "/usr/bin/cat"])?;
    let mut command = SandboxCommand::new(&program);
    command.arg(target);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut profile = SandboxProfile::new().allow_executable(&program);
    if let Some(path) = readable {
        profile = profile.allow_read_path(path);
    }
    launch(profile, command)
}

/// Runs `env` in a sandbox with `profile`, returning its standard output.
fn run_env(profile: SandboxProfile) -> Result<String> {
    let program = resolve_binary(&["/usr/bin/env", "/bin/env"])?;
    let mut command = SandboxCommand::new(&program);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let output = launch(profile.allow_executable(&program), command)?;
    ensure!(
        output.status.success(),
        "env failed: {:?} {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A whitelisted file is readable inside the sandbox.
fn allowed_file_is_readable() -> Result<()> {
    let files = fixture()?;
    let output = run_cat(&files.allowed, Some(&files.allowed))?;
    ensure!(output.status.success(), "cat failed: {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        stdout.contains("allowed file content"),
        "stdout was {stdout:?}"
    );
    Ok(())
}

/// A file outside the profile cannot be read, and not because the sandbox
/// itself failed to start.
fn disallowed_file_access_is_blocked() -> Result<()> {
    let files = fixture()?;
    let output = run_cat(&files.forbidden, None)?;
    ensure!(!output.status.success(), "the forbidden file was readable");
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        !stderr.contains("sandboxing failure") && stderr.contains("forbidden.txt"),
        "expected cat's own refusal, got {stderr:?}"
    );
    Ok(())
}

/// Only the allow-listed variable reaches the sandboxed process.
fn environment_allowlist_is_inherited() -> Result<()> {
    let stdout = run_env(SandboxProfile::new().allow_environment_variable("KEEP_ME"))?;
    ensure!(stdout.contains("KEEP_ME=present"), "stdout was {stdout:?}");
    ensure!(!stdout.contains("DROP_ME"), "stdout was {stdout:?}");
    Ok(())
}

/// The default profile passes no variable through.
fn environment_is_isolated_by_default() -> Result<()> {
    let stdout = run_env(SandboxProfile::new())?;
    ensure!(!stdout.contains("KEEP_ME"), "stdout was {stdout:?}");
    ensure!(!stdout.contains("DROP_ME"), "stdout was {stdout:?}");
    Ok(())
}

/// Full inheritance passes every variable through.
fn full_environment_can_be_inherited() -> Result<()> {
    let stdout = run_env(SandboxProfile::new().allow_full_environment())?;
    ensure!(stdout.contains("KEEP_ME=present"), "stdout was {stdout:?}");
    ensure!(
        stdout.contains("DROP_ME=remove-me"),
        "stdout was {stdout:?}"
    );
    Ok(())
}

/// The process environment as sorted pairs, so two snapshots compare equal
/// whatever order the platform lists them in.
fn sorted_environment() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let mut vars: Vec<_> = env::vars_os().collect();
    vars.sort();
    vars
}

/// After a spawn attempt `birdcage` may have stripped the process environment;
/// the sandbox's `EnvGuard` must have put every variable back. The case
/// snapshots the whole environment before the attempt and requires the same set
/// after it, and the scenario variables must be among them.
///
/// The environment comparison comes first, whatever the spawn did. A spawn
/// that succeeds passes the case (the child's own outcome is irrelevant, so the
/// host need not exec in the sandbox). A typed preflight error fails it, since
/// the sandbox was never reached. An activation error (a host that forbids the
/// sandbox's namespaces refuses the spawn this way, but so can a failed
/// exception) cannot show that the environment was stripped and restored, so it
/// fails the case as "not verified" instead of passing it.
///
/// Not ported from the deleted `EnvGuard` tests: a variable created while the
/// guard is live and removal on drop, which need the `pub(crate)` guard itself.
fn parent_environment_is_restored_after_spawn() -> Result<()> {
    let before = sorted_environment();
    for (key, value) in SCENARIO_ENV {
        ensure!(
            env::var(key).as_deref() == Ok(value),
            "{key} was not set before the spawn"
        );
    }
    let program = resolve_binary(&["/bin/true", "/usr/bin/true"])?;
    let profile = SandboxProfile::new().allow_executable(&program);
    let spawned = Sandbox::new(profile).spawn(SandboxCommand::new(&program));
    let after = sorted_environment();
    ensure!(
        before == after,
        "the environment changed across the spawn: {before:?} became {after:?}"
    );
    match spawned {
        Ok(child) => {
            child.wait_with_output().context("wait for the child")?;
            Ok(())
        }
        // `birdcage` refused to activate (a denied namespace, say). Activation
        // can fail before or after it strips the environment, so a run that
        // ends here has not verified restoration: fail rather than pass, since
        // the runner cannot report a third outcome.
        Err(SandboxError::Activation(err)) => bail!(
            "restoration was not verified: birdcage did not activate ({err}), which can happen \
             before or after it strips the environment"
        ),
        Err(other) => bail!("the spawn failed before the sandbox was reached: {other}"),
    }
}

/// Spawns `program` under an empty profile and returns the rejection.
fn spawn_rejection(program: &Path) -> Result<SandboxError> {
    let mut command = SandboxCommand::new(program);
    command.arg("hello");
    match Sandbox::new(SandboxProfile::new()).spawn(command) {
        Ok(_) => bail!(
            "the spawn of {} should have been rejected",
            program.display()
        ),
        Err(err) => Ok(err),
    }
}

/// A relative program path is refused.
fn relative_program_is_rejected() -> Result<()> {
    match spawn_rejection(Path::new("relative/bin"))? {
        SandboxError::ProgramNotAbsolute(path) if path == Path::new("relative/bin") => Ok(()),
        other => bail!("unexpected error: {other:?}"),
    }
}

/// A program that does not exist is refused.
fn missing_program_is_rejected() -> Result<()> {
    let missing = Path::new("/definitely/missing/tool");
    match spawn_rejection(missing)? {
        SandboxError::MissingPath { path } if path == missing => Ok(()),
        other => bail!("unexpected error: {other:?}"),
    }
}

/// A program the profile does not list is refused.
fn unlisted_program_is_rejected() -> Result<()> {
    let program = resolve_binary(&["/bin/echo", "/usr/bin/echo"])?;
    match spawn_rejection(&program)? {
        SandboxError::ExecutableNotAuthorised { program: refused } if refused == program => Ok(()),
        other => bail!("unexpected error: {other:?}"),
    }
}

/// A second live thread must be refused before `birdcage` can assert.
fn multithreaded_process_is_rejected() -> Result<()> {
    let (release, parked) = mpsc::channel::<()>();
    let (ready_tx, ready) = mpsc::channel::<()>();
    let helper = thread::spawn(move || {
        ready_tx.send(()).ok();
        parked.recv().ok();
    });
    ready.recv().context("wait for the helper thread")?;
    let program = resolve_binary(&["/bin/true", "/usr/bin/true"])?;
    let rejection = spawn_rejection(&program);
    release.send(()).ok();
    helper.join().ok();
    match rejection? {
        SandboxError::MultiThreaded { thread_count } if thread_count > 1 => Ok(()),
        other => bail!("expected MultiThreaded, got {other:?}"),
    }
}
