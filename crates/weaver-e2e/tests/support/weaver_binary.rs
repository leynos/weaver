//! Resolves the workspace `weaver` binary for end-to-end tests.

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

/// Returns the resolved path to the `weaver` binary used by e2e tests.
pub(crate) fn weaver_binary_path() -> &'static Path {
    static WEAVER_BINARY: OnceLock<PathBuf> = OnceLock::new();
    WEAVER_BINARY.get_or_init(|| match resolve_weaver_binary() {
        Ok(path) => path,
        Err(error) => panic!("failed to locate weaver binary: {error}"),
    })
}

fn resolve_weaver_binary() -> Result<PathBuf, String> {
    if let Some(cargo_bin) = cargo_bin_from_env("weaver") {
        return Ok(cargo_bin);
    }

    let target_dir_candidate = target_dir_binary_path("weaver")?;
    if target_dir_candidate.is_file() {
        return Ok(target_dir_candidate);
    }

    let fallback = target_debug_binary_path()?;
    if fallback.is_file() {
        return Ok(fallback);
    }

    build_workspace_binary()?;
    if target_dir_candidate.is_file() {
        return Ok(target_dir_candidate);
    }
    if fallback.is_file() {
        return Ok(fallback);
    }

    Err(format!(
        "failed to locate built weaver binary after cargo build: checked {} and {}",
        target_dir_candidate.display(),
        fallback.display()
    ))
}

fn cargo_bin_from_env(name: &str) -> Option<PathBuf> {
    let variable_name = format!("CARGO_BIN_EXE_{name}");
    env::var_os(variable_name)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn target_dir_binary_path(name: &str) -> Result<PathBuf, String> {
    let mut target_dir =
        env::current_exe().map_err(|error| format!("current executable path: {error}"))?;
    target_dir.pop();
    if target_dir.ends_with("deps") {
        target_dir.pop();
    }

    let binary_path = target_dir.join(format!("{name}{}", env::consts::EXE_SUFFIX));
    Ok(binary_path)
}

fn target_debug_binary_path() -> Result<PathBuf, String> {
    Ok(workspace_root()?
        .join("target")
        .join("debug")
        .join(format!("weaver{}", env::consts::EXE_SUFFIX)))
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| String::from("workspace root should exist for e2e tests"))
}

fn build_workspace_binary() -> Result<(), String> {
    let status = Command::new("cargo")
        .current_dir(workspace_root()?)
        .args(["build", "-p", "weaver-cli", "--bin", "weaver"])
        .status()
        .map_err(|error| format!("failed to build workspace weaver binary: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "building workspace weaver binary failed with status {status}"
        ))
    }
}
