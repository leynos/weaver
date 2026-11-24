//! The cases behind `sandbox_process`.

use std::{
    env,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use anyhow::{Context as _, Result, bail, ensure};
use cap_std::{ambient_authority, fs::Dir};
use weaver_sandbox::{Sandbox, SandboxCommand, SandboxError, SandboxProfile};

/// Scenario variables the parent hands to every child.
pub const SCENARIO_ENV: [(&str, &str); 2] = [("KEEP_ME", "present"), ("DROP_ME", "remove-me")];

/// One named case of the target.
pub type Case = (&'static str, fn() -> Result<()>);

/// Every case, in listing order.
pub const CASES: &[Case] = &[
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
