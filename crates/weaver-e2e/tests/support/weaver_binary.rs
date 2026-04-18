//! Resolves the workspace `weaver` binary for end-to-end tests.

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

pub(crate) fn weaver_binary_path() -> &'static Path {
    static WEAVER_BINARY: OnceLock<PathBuf> = OnceLock::new();
    WEAVER_BINARY.get_or_init(resolve_weaver_binary)
}

fn resolve_weaver_binary() -> PathBuf {
    if let Some(cargo_bin) = cargo_bin_from_env("weaver") {
        return cargo_bin;
    }

    if let Some(target_dir_binary) = target_dir_binary_path("weaver") {
        return target_dir_binary;
    }

    let fallback = target_debug_binary_path();
    if fallback.is_file() {
        return fallback;
    }

    build_workspace_binary(&fallback);
    fallback
}

fn cargo_bin_from_env(name: &str) -> Option<PathBuf> {
    let variable_name = format!("CARGO_BIN_EXE_{name}");
    env::var_os(variable_name)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn target_dir_binary_path(name: &str) -> Option<PathBuf> {
    let mut target_dir = env::current_exe().ok()?;
    target_dir.pop();
    if target_dir.ends_with("deps") {
        target_dir.pop();
    }

    let binary_path = target_dir.join(format!("{name}{}", env::consts::EXE_SUFFIX));
    binary_path.is_file().then_some(binary_path)
}

fn target_debug_binary_path() -> PathBuf {
    workspace_root()
        .join("target")
        .join("debug")
        .join(format!("weaver{}", env::consts::EXE_SUFFIX))
}

#[expect(
    clippy::expect_used,
    reason = "test binary discovery should panic with an explicit setup message"
)]
fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should exist for e2e tests")
        .to_path_buf()
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
