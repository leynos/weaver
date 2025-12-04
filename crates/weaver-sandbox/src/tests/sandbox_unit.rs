//! Unit tests covering sandbox spawn preflight errors.

use std::path::PathBuf;
use std::io;

use crate::sandbox::{Sandbox, SandboxCommand};
use crate::{SandboxError, SandboxProfile};

fn sandbox_with_profile(profile: SandboxProfile) -> Sandbox {
    Sandbox::new(profile)
}

#[cfg(test)]
fn sandbox_with_forced_thread_count<F>(profile: SandboxProfile, counter: F) -> Sandbox
where
    F: Fn() -> Result<usize, io::Error> + Send + Sync + 'static,
{
    Sandbox::with_thread_counter_for_tests(profile, Box::new(counter))
}

#[test]
fn rejects_relative_program_paths() {
    let sandbox = sandbox_with_profile(SandboxProfile::new());
    let command = SandboxCommand::new("relative/bin");

    let err = sandbox.spawn(command).expect_err("spawn should fail");
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
    let sandbox = sandbox_with_profile(SandboxProfile::new());
    let command = SandboxCommand::new(&missing);

    let err = sandbox.spawn(command).expect_err("spawn should fail");
    match err {
        SandboxError::MissingPath { path } => assert_eq!(path, missing),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn rejects_unwhitelisted_programs() {
    let program = PathBuf::from("/bin/echo");
    let sandbox = sandbox_with_profile(SandboxProfile::new());
    let mut command = SandboxCommand::new(&program);
    command.arg("hello");

    let err = sandbox.spawn(command).expect_err("spawn should fail");
    match err {
        SandboxError::ExecutableNotAuthorised { program: p } => assert_eq!(p, program),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn rejects_multithreaded_processes() {
    let sandbox = sandbox_with_forced_thread_count(SandboxProfile::new(), || {
        Ok(4)
    });
    let command = SandboxCommand::new("/usr/bin/true");

    let err = sandbox
        .spawn(command)
        .expect_err("spawn should fail for multi-threaded processes");
    match err {
        SandboxError::MultiThreaded { thread_count } => assert_eq!(thread_count, 4),
        other => panic!("expected MultiThreaded error, got: {other:?}"),
    }
}

#[test]
fn rejects_when_thread_count_unavailable() {
    let sandbox = sandbox_with_forced_thread_count(SandboxProfile::new(), || {
        Err(io::Error::new(io::ErrorKind::Other, "thread count failed"))
    });
    let command = SandboxCommand::new("/usr/bin/true");

    let err = sandbox
        .spawn(command)
        .expect_err("spawn should fail when thread count is unavailable");
    match err {
        SandboxError::ThreadCountUnavailable { .. } => {}
        other => panic!("expected ThreadCountUnavailable error, got: {other:?}"),
    }
}
