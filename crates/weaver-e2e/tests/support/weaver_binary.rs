//! Resolves the workspace `weaver` binary for end-to-end tests.

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use assert_cmd::cargo;

pub(crate) fn weaver_binary_path() -> &'static Path {
    static WEAVER_BINARY: OnceLock<PathBuf> = OnceLock::new();
    WEAVER_BINARY.get_or_init(resolve_weaver_binary)
}

#[expect(
    deprecated,
    reason = "assert_cmd resolves the workspace binary path when Cargo exposes it"
)]
fn resolve_weaver_binary() -> PathBuf {
    let cargo_bin = cargo::cargo_bin("weaver");
    if cargo_bin.is_file() {
        return cargo_bin;
    }

    let fallback = target_debug_binary_path();
    if fallback.is_file() {
        return fallback;
    }

    build_workspace_binary(&fallback);
    fallback
}

fn target_debug_binary_path() -> PathBuf {
    workspace_root()
        .join("target")
        .join("debug")
        .join(format!("weaver{}", env::consts::EXE_SUFFIX))
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().and_then(Path::parent).map_or_else(
        || panic!("workspace root should exist for e2e tests"),
        Path::to_path_buf,
    )
}

fn build_workspace_binary(expected_path: &Path) {
    let status_result = Command::new("cargo")
        .current_dir(workspace_root())
        .args(["build", "-p", "weaver-cli", "--bin", "weaver"])
        .status();

    match status_result {
        Ok(status) if status.success() && expected_path.is_file() => {}
        Ok(status) => panic!(
            "building workspace weaver binary failed with status {status}: {}",
            expected_path.display()
        ),
        Err(error) => panic!("failed to build workspace weaver binary: {error}"),
    }
}
