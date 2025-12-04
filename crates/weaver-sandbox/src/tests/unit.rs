//! Unit tests for sandbox configuration helpers.

use std::path::PathBuf;

use crate::profile::{EnvironmentPolicy, NetworkPolicy, SandboxProfile};

#[test]
fn profile_whitelists_linux_runtime_roots() {
    let profile = SandboxProfile::new();
    if cfg!(target_os = "linux") {
        assert!(
            !profile.read_only_paths().is_empty(),
            "linux runtime roots should be whitelisted by default"
        );
    } else {
        assert!(profile.read_only_paths().is_empty());
    }
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
        },
        other => panic!("unexpected environment policy: {other:?}"),
    }
}

#[test]
fn network_is_denied_by_default() {
    let profile = SandboxProfile::new();
    assert_eq!(profile.network_policy(), NetworkPolicy::Deny);
}

#[test]
fn network_can_be_allowed() {
    let profile = SandboxProfile::new().allow_networking();
    assert_eq!(profile.network_policy(), NetworkPolicy::Allow);
    assert!(!NetworkPolicy::Allow.is_denied());
}

#[test]
fn full_environment_can_be_inherited() {
    let profile = SandboxProfile::new().allow_full_environment();
    assert!(matches!(
        profile.environment_policy(),
        EnvironmentPolicy::InheritAll
    ));
}

#[test]
fn environment_allowlist_has_no_effect_after_full_inherit() {
    let profile = SandboxProfile::new()
        .allow_full_environment()
        .allow_environment_variable("SHOULD_BE_IGNORED");

    assert!(matches!(
        profile.environment_policy(),
        EnvironmentPolicy::InheritAll
    ));
}

#[test]
fn read_write_paths_are_recorded() {
    let profile = SandboxProfile::new()
        .allow_read_path(PathBuf::from("/tmp"))
        .allow_read_write_path(PathBuf::from("/var/tmp"));

    assert!(profile
        .read_only_paths()
        .iter()
        .any(|path| path.ends_with("tmp")));
    assert!(profile
        .read_write_paths()
        .iter()
        .any(|path| path.ends_with("tmp")));
}

#[test]
fn canonicalises_nonexistent_child_when_parent_exists() {
    let base = tempfile::tempdir().expect("tempdir");
    let target = base.path().join("future_dir").join("file.txt");

    let profile = SandboxProfile::new().allow_read_write_path(&target);

    let set = profile.read_write_paths();
    assert!(
        set.iter().any(|p| p.ends_with("file.txt")),
        "expected future file to be recorded"
    );
}
