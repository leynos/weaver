//! Manifest builders for refactor plugins.
//!
//! Keeping manifest construction here prevents the main refactor handler from
//! overflowing the repository's 400-line limit while still exposing a small
//! test seam for capability declarations.

use std::path::PathBuf;

use weaver_plugins::{
    CapabilityId,
    manifest::{PluginKind, PluginManifest, PluginMetadata},
};

use super::plugin_paths::{
    ROPE_PLUGIN_NAME,
    ROPE_PLUGIN_VERSION,
    RUST_ANALYZER_PLUGIN_NAME,
    RUST_ANALYZER_PLUGIN_TIMEOUT_SECS,
    RUST_ANALYZER_PLUGIN_VERSION,
};

/// Builds the default rope plugin manifest.
pub(crate) fn rope_manifest(executable: PathBuf) -> PluginManifest {
    let metadata = PluginMetadata::new(ROPE_PLUGIN_NAME, ROPE_PLUGIN_VERSION, PluginKind::Actuator);
    PluginManifest::new(metadata, vec![String::from("python")], executable)
        .with_capabilities(vec![CapabilityId::RenameSymbol])
}

/// Builds the default rust-analyzer plugin manifest.
pub(crate) fn rust_analyzer_manifest(executable: PathBuf) -> PluginManifest {
    let metadata = PluginMetadata::new(
        RUST_ANALYZER_PLUGIN_NAME,
        RUST_ANALYZER_PLUGIN_VERSION,
        PluginKind::Actuator,
    );
    PluginManifest::new(metadata, vec![String::from("rust")], executable)
        .with_capabilities(vec![CapabilityId::RenameSymbol])
        .with_timeout_secs(RUST_ANALYZER_PLUGIN_TIMEOUT_SECS)
}
