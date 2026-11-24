//! Unit tests for the crate-private profile accessors and the spawn preflight.
//!
//! None of these cases spawns a process or touches the environment: the
//! preflight errors are raised before `birdcage` is involved, and the profile
//! cases only read the profile's recorded policy. Process-level behaviour lives
//! in `tests/sandbox_process.rs`.

use std::io;

use crate::{
    Sandbox,
    SandboxCommand,
    SandboxError,
    profile::{EnvironmentPolicy, SandboxProfile},
};

#[test]
fn profile_whitelists_linux_runtime_roots() {
    let profile = SandboxProfile::new();
    let canonical = profile.read_only_paths_canonicalised();
    let Ok(roots) = canonical else {
        panic!("the default roots should canonicalise: {canonical:?}");
    };
    assert_eq!(
        !roots.is_empty(),
        cfg!(target_os = "linux"),
        "only linux whitelists runtime roots by default"
    );
}

#[test]
fn environment_allowlist_deduplicates_entries() {
    let profile = SandboxProfile::new()
        .allow_environment_variable("KEEP_ME")
        .allow_environment_variable("KEEP_ME");

    match profile.environment_policy() {
        EnvironmentPolicy::AllowList(keys) => {
            assert_eq!(keys.len(), 1);
            assert!(keys.contains("KEEP_ME"));
        }
        other => panic!("unexpected environment policy: {other:?}"),
    }
}

#[test]
fn full_environment_ignores_later_allowlist_entries() {
    let profile = SandboxProfile::new()
        .allow_full_environment()
        .allow_environment_variable("SHOULD_BE_IGNORED");

    assert_eq!(profile.environment_policy(), &EnvironmentPolicy::InheritAll);
}

#[test]
fn read_and_read_write_paths_are_recorded_separately() {
    let base = tempfile::tempdir().expect("tempdir");
    let read = base.path().join("read");
    let write = base.path().join("write");
    std::fs::create_dir(&read).expect("create the read directory");
    std::fs::create_dir(&write).expect("create the write directory");

    let profile = SandboxProfile::new()
        .allow_read_path(&read)
        .allow_read_write_path(&write);

    let canonical_read = std::fs::canonicalize(&read).expect("canonicalise read");
    let canonical_write = std::fs::canonicalize(&write).expect("canonicalise write");
    let read_only = profile.read_only_paths_canonicalised().expect("read paths");
    let read_write = profile
        .read_write_paths_canonicalised()
        .expect("write paths");
    assert!(read_only.contains(&canonical_read));
    assert!(!read_only.contains(&canonical_write));
    assert!(read_write.contains(&canonical_write));
    assert!(!read_write.contains(&canonical_read));
}

#[test]
fn records_nonexistent_future_path() {
    let base = tempfile::tempdir().expect("tempdir");
    let target = base.path().join("future_dir").join("file.txt");

    let profile = SandboxProfile::new().allow_read_write_path(&target);

    let recorded = profile
        .read_write_paths_canonicalised()
        .expect("a not-yet-created path should canonicalise from its ancestor");
    assert!(
        recorded
            .iter()
            .any(|path| path.ends_with("future_dir/file.txt")),
        "expected the future file to be recorded, got {recorded:?}"
    );
}

/// Builds a sandbox whose thread counter returns `counter`'s result.
fn sandbox_with_thread_count(counter: fn() -> Result<usize, io::Error>) -> Sandbox {
    Sandbox::with_thread_counter_for_tests(SandboxProfile::new(), Box::new(counter))
}

/// Returns the error `sandbox` raises when asked to spawn `/usr/bin/true`.
fn spawn_rejection(sandbox: &Sandbox) -> SandboxError {
    match sandbox.spawn(SandboxCommand::new("/usr/bin/true")) {
        Err(err) => err,
        Ok(_) => panic!("the preflight should have rejected the spawn"),
    }
}

#[test]
fn rejects_multithreaded_processes() {
    let sandbox = sandbox_with_thread_count(|| Ok(4));

    match spawn_rejection(&sandbox) {
        SandboxError::MultiThreaded { thread_count } => assert_eq!(thread_count, 4),
        other => panic!("expected MultiThreaded error, got: {other:?}"),
    }
}

#[test]
fn rejects_when_thread_count_unavailable() {
    let sandbox = sandbox_with_thread_count(|| Err(io::Error::other("thread count failed")));

    let err = spawn_rejection(&sandbox);
    assert!(
        matches!(err, SandboxError::ThreadCountUnavailable { .. }),
        "expected ThreadCountUnavailable, got: {err:?}"
    );
}
