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

/// Checks that a trivial program runs in the sandbox on this host.
///
/// On a merged-`/usr` host the new root holds `/usr/lib64` but not the
/// `/lib64` symlink that names the ELF interpreter, so every dynamically linked
/// program fails to start there. A host that forbids unprivileged user
/// namespaces (hosted Ubuntu images do) fails every sandboxed start with
/// "Operation not permitted". Only those two known limitations skip the cases
/// that launch a program: the probe must fail with the sandbox's "No such file"
/// start-up error *and* `/lib64` must be a symlink on this host, or with the
/// "Operation not permitted" start-up error. Any other probe failure is a
/// regression and fails the case.
fn require_exec() -> Result<()> {
    let program = resolve_binary(&["/bin/true", "/usr/bin/true"])?;
    let mut command = SandboxCommand::new(&program);
    command.stderr(Stdio::piped());
    let profile = SandboxProfile::new().allow_executable(&program);
    let output = spawn(profile, command)?
        .wait_with_output()
        .context("read the probe's output")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let merged_usr =
        stderr.contains("sandboxing failure: No such file or directory") && lib64_is_symlink()?;
    let namespaces_forbidden = stderr.contains("sandboxing failure: Operation not permitted");
    ensure!(
        merged_usr || namespaces_forbidden,
        "the probe failed for a reason other than a known host limitation: {stderr:?}"
    );
    let reason = format!("the host cannot exec in the sandbox: {}", stderr.trim());
    Err(Skipped(reason).into())
}

/// Whether `/lib64` is a symlink, as on a merged-`/usr` host.
fn lib64_is_symlink() -> Result<bool> {
    let root = Dir::open_ambient_dir("/", ambient_authority()).context("open the root")?;
    let metadata = root.symlink_metadata("lib64").context("inspect /lib64")?;
    Ok(metadata.is_symlink())
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

/// After a spawn `birdcage` has stripped the process environment; the
/// sandbox's `EnvGuard` must have put every variable back. The case snapshots
/// the whole environment before the spawn and requires the same set after it,
/// and the scenario variables must be among them. The child's own outcome is
/// irrelevant (the stripping happens in the parent before the fork), so this
/// case does not need the host to exec in the sandbox.
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
    spawn(profile, SandboxCommand::new(&program))?
        .wait_with_output()
        .context("wait for the child")?;
    let after = sorted_environment();
    ensure!(
        before == after,
        "the environment changed across the spawn: {before:?} became {after:?}"
    );
    Ok(())
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
