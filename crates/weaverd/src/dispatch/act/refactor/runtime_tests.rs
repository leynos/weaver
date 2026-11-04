//! Unit tests for plugin runtime selection and executable path resolution.
//!
//! These cover the defaults used when the daemon has no explicit plugin
//! overrides. They exercise construction rather than dispatch, so they live
//! apart from the behavioural tests in the sibling `tests` module.

use std::ffi::OsString;

use weaver_plugins::PluginRequest;

use crate::dispatch::act::refactor::{
    default_runtime,
    resolve_rope_plugin_path,
    resolve_rust_analyzer_plugin_path,
};

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
