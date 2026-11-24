//! Unit tests for plugin runtime selection and executable path resolution.
//!
//! These cover the defaults used when the daemon has no explicit plugin
//! overrides. They exercise construction rather than dispatch, so they live
//! apart from the behavioural tests in the sibling `tests` module.

use std::{ffi::OsString, path::Path};

use mockable::MockEnv;
use weaver_plugins::PluginRequest;

use crate::dispatch::act::refactor::{
    SandboxRefactorRuntime,
    default_runtime,
    plugin_paths::{
        DEFAULT_ROPE_PLUGIN_PATH,
        DEFAULT_RUST_ANALYZER_PLUGIN_PATH,
        ROPE_PLUGIN_NAME,
        ROPE_PLUGIN_PATH_ENV,
        RUST_ANALYZER_PLUGIN_NAME,
        RUST_ANALYZER_PLUGIN_PATH_ENV,
    },
    resolve_rope_plugin_path,
    resolve_rust_analyzer_plugin_path,
};

/// Builds a reader that answers the two plugin path variables.
///
/// Each variable is matched by name, so a runtime that swapped the two keys, or
/// ignored the injected reader, would register the wrong executables.
fn plugin_path_env(rope: Option<&'static str>, rust_analyzer: Option<&'static str>) -> MockEnv {
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == ROPE_PLUGIN_PATH_ENV)
        .returning(move |_| rope.map(OsString::from));
    env.expect_os_string()
        .withf(|key| key == RUST_ANALYZER_PLUGIN_PATH_ENV)
        .returning(move |_| rust_analyzer.map(OsString::from));
    env
}

/// Returns the executable registered for `provider` in `runtime`.
fn registered_executable(runtime: &SandboxRefactorRuntime, provider: &str) -> std::path::PathBuf {
    let Some(manifest) = runtime.registry.get(provider) else {
        panic!("the runtime must register {provider}");
    };
    manifest.executable().to_path_buf()
}

#[test]
fn from_environment_registers_each_plugin_at_its_own_override() {
    let env = plugin_path_env(Some("/opt/rope/plugin"), Some("/opt/ra/plugin"));

    let Ok(runtime) = SandboxRefactorRuntime::from_environment(&env) else {
        panic!("registration with absolute overrides must succeed");
    };

    assert_eq!(
        registered_executable(&runtime, ROPE_PLUGIN_NAME),
        Path::new("/opt/rope/plugin")
    );
    assert_eq!(
        registered_executable(&runtime, RUST_ANALYZER_PLUGIN_NAME),
        Path::new("/opt/ra/plugin")
    );
}

#[test]
fn from_environment_uses_the_defaults_when_both_variables_are_unset() {
    let env = plugin_path_env(None, None);

    let Ok(runtime) = SandboxRefactorRuntime::from_environment(&env) else {
        panic!("registration with the default paths must succeed");
    };

    assert_eq!(
        registered_executable(&runtime, ROPE_PLUGIN_NAME),
        Path::new(DEFAULT_ROPE_PLUGIN_PATH)
    );
    assert_eq!(
        registered_executable(&runtime, RUST_ANALYZER_PLUGIN_NAME),
        Path::new(DEFAULT_RUST_ANALYZER_PLUGIN_PATH)
    );
}

#[test]
fn resolve_rope_plugin_path_makes_relative_overrides_absolute() {
    let path = resolve_rope_plugin_path(Some(OsString::from("bin/rope")));
    assert!(path.is_absolute());
}

#[test]
fn resolve_rust_analyzer_plugin_path_makes_relative_overrides_absolute() {
    let path = resolve_rust_analyzer_plugin_path(Some(OsString::from("bin/rust-analyzer-plugin")));
    assert!(path.is_absolute());
}

#[test]
fn default_runtime_returns_shared_trait_object() {
    let runtime = default_runtime();
    let request = PluginRequest::new("rename", Vec::new());
    let result = runtime.execute("rope", &request);
    assert!(result.is_err());
}
