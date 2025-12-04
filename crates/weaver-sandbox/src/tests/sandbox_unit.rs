//! Unit tests covering sandbox spawn preflight errors.

use std::path::PathBuf;

use crate::sandbox::{Sandbox, SandboxCommand};
use crate::{SandboxError, SandboxProfile};

fn sandbox_with_profile(profile: SandboxProfile) -> Sandbox {
    Sandbox::new(profile)
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
