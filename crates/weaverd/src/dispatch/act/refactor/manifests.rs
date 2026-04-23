//! Manifest builders for refactor plugins.
//!
//! Keeping manifest construction here prevents the main refactor handler from
//! overflowing the repository's 400-line limit while still exposing a small
//! test seam for capability declarations.

use std::path::PathBuf;

use weaver_plugins::CapabilityId;
use weaver_plugins::manifest::{PluginKind, PluginManifest, PluginMetadata};

use super::plugin_paths::{
    ROPE_PLUGIN_NAME, ROPE_PLUGIN_VERSION, RUST_ANALYZER_PLUGIN_NAME,
    RUST_ANALYZER_PLUGIN_TIMEOUT_SECS, RUST_ANALYZER_PLUGIN_VERSION,
};

struct BuiltInProviderSpec {
    name: &'static str,
    version: &'static str,
    languages: &'static [&'static str],
    timeout_secs: Option<u64>,
}

macro_rules! built_in_provider_catalogue {
    (
        $(
            {
                name: $name:expr,
                version: $version:expr,
                languages: [$($language:expr),* $(,)?],
                timeout_secs: $timeout_secs:expr
            }
        ),+ $(,)?
    ) => {
        const BUILT_IN_PROVIDER_SPECS: &[BuiltInProviderSpec] = &[
            $(
                BuiltInProviderSpec {
                    name: $name,
                    version: $version,
                    languages: &[$($language),*],
                    timeout_secs: $timeout_secs,
                },
            )+
        ];

        pub(crate) const BUILT_IN_PROVIDER_NAMES: &[&str] = &[$($name),+];
    };
}

built_in_provider_catalogue!(
    {
        name: ROPE_PLUGIN_NAME,
        version: ROPE_PLUGIN_VERSION,
        languages: ["python"],
        timeout_secs: None
    },
    {
        name: RUST_ANALYZER_PLUGIN_NAME,
        version: RUST_ANALYZER_PLUGIN_VERSION,
        languages: ["rust"],
        timeout_secs: Some(RUST_ANALYZER_PLUGIN_TIMEOUT_SECS)
    },
);

/// Builds the default rope plugin manifest.
pub(crate) fn rope_manifest(executable: PathBuf) -> PluginManifest {
    manifest_from_spec(provider_spec(ROPE_PLUGIN_NAME), executable)
}

/// Builds the default rust-analyzer plugin manifest.
pub(crate) fn rust_analyzer_manifest(executable: PathBuf) -> PluginManifest {
    manifest_from_spec(provider_spec(RUST_ANALYZER_PLUGIN_NAME), executable)
}

pub(crate) fn built_in_provider_names() -> &'static [&'static str] {
    BUILT_IN_PROVIDER_NAMES
}

fn manifest_from_spec(spec: &BuiltInProviderSpec, executable: PathBuf) -> PluginManifest {
    let metadata = PluginMetadata::new(spec.name, spec.version, PluginKind::Actuator);
    let manifest = PluginManifest::new(
        metadata,
        spec.languages
            .iter()
            .map(|language| String::from(*language))
            .collect(),
        executable,
    )
    .with_capabilities(vec![CapabilityId::RenameSymbol]);

    if let Some(timeout_secs) = spec.timeout_secs {
        manifest.with_timeout_secs(timeout_secs)
    } else {
        manifest
    }
}

fn provider_spec(name: &str) -> &'static BuiltInProviderSpec {
    BUILT_IN_PROVIDER_SPECS
        .iter()
        .find(|spec| spec.name == name)
        .unwrap_or_else(|| panic!("missing built-in provider spec for '{name}'"))
}
