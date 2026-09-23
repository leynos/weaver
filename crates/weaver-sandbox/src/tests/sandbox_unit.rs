//! Unit tests covering sandbox spawn preflight errors.

use std::{io, path::PathBuf};

use crate::{
    SandboxError,
    SandboxProfile,
    sandbox::{Sandbox, SandboxCommand},
};

fn sandbox_with_forced_thread_count<F>(profile: SandboxProfile, counter: F) -> Sandbox
where
    F: Fn() -> Result<usize, io::Error> + Send + Sync + 'static,
{
    Sandbox::with_thread_counter_for_tests(profile, Box::new(counter))
}

fn spawn_expect_error(
    program: &PathBuf,
    profile: SandboxProfile,
) -> Result<SandboxError, &'static str> {
    let mut command = SandboxCommand::new(program);
    command.arg("hello");

    spawn_error(
        &sandbox_with_forced_thread_count(profile, || Ok(1)),
        command,
    )
}

fn spawn_error(sandbox: &Sandbox, command: SandboxCommand) -> Result<SandboxError, &'static str> {
    match sandbox.spawn(command) {
        Err(error) => Ok(error),
        Ok(_) => Err("spawn unexpectedly succeeded"),
    }
}

#[test]
fn rejects_relative_program_paths() {
    let sandbox = sandbox_with_forced_thread_count(SandboxProfile::new(), || Ok(1));
    let command = SandboxCommand::new("relative/bin");

    let err = spawn_error(&sandbox, command).expect("spawn should fail");
    match err {
        SandboxError::ProgramNotAbsolute(path) => {
            assert_eq!(path, PathBuf::from("relative/bin"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn rejects_missing_program_paths() {
    let missing = PathBuf::from("/definitely/missing/tool");
    let error = spawn_expect_error(&missing, SandboxProfile::new())
        .expect("missing program should fail before Birdcage spawn");
    match error {
        SandboxError::MissingPath { path } => assert_eq!(path, missing),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn rejects_unwhitelisted_programs() {
    let program = PathBuf::from("/bin/echo");
    let error = spawn_expect_error(&program, SandboxProfile::new())
        .expect("unwhitelisted program should fail before Birdcage spawn");
    match error {
        SandboxError::ExecutableNotAuthorised { program: p } => {
            let canonical = program
                .canonicalize()
                .expect("test executable should exist");
            assert_eq!(p, canonical);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn rejects_multithreaded_processes() {
    let sandbox = sandbox_with_forced_thread_count(SandboxProfile::new(), || Ok(4));
    let command = SandboxCommand::new("/usr/bin/true");

    let err =
        spawn_error(&sandbox, command).expect("spawn should fail for multi-threaded processes");
    match err {
        SandboxError::MultiThreaded { thread_count } => assert_eq!(thread_count, 4),
        other => panic!("expected MultiThreaded error, got: {other:?}"),
    }
}

#[test]
fn rejects_when_thread_count_unavailable() {
    let sandbox = sandbox_with_forced_thread_count(SandboxProfile::new(), || {
        Err(io::Error::other("thread count failed"))
    });
    let command = SandboxCommand::new("/usr/bin/true");

    let err =
        spawn_error(&sandbox, command).expect("spawn should fail when thread count is unavailable");
    match err {
        SandboxError::ThreadCountUnavailable { .. } => {}
        other => panic!("expected ThreadCountUnavailable error, got: {other:?}"),
    }
}
