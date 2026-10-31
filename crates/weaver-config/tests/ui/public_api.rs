//! Downstream compile-time usage of the `weaver-config` public API.
//!
//! The migration to OrthoConfig v0.10.0 changes the generated metadata and the
//! derive that supplies `Config`. This fixture keeps the caller-facing surface
//! compiling from outside the crate: the default configuration, the accessors
//! that read it, the capability matrix built from directives, the help lookup
//! the CLI help surfaces rely on, and the generated localization and
//! documentation metadata that `weaver-cli` renders.

use ortho_config::{ArgLocalizationIds, OrthoConfig, OrthoConfigDocs, OrthoConfigLocalization};
use weaver_config::{
    CapabilityDirective,
    CapabilityMatrix,
    CapabilityOverride,
    Config,
    LogFormat,
    config_field_help,
    default_socket_endpoint,
};

fn main() {
    let config = Config::default();

    let _socket = config.daemon_socket();
    let _filter = config.log_filter();
    let _format: LogFormat = config.log_format();
    let _locale = config.locale();
    let matrix: CapabilityMatrix = config.capability_matrix();
    assert!(matrix.languages.is_empty());

    let directive = CapabilityDirective::new("rust", "observe.rename", CapabilityOverride::Deny);
    let _override = matrix.override_for("rust", "observe.rename");
    let _directive = directive;

    let defaults_socket = default_socket_endpoint();
    assert_eq!(&defaults_socket, config.daemon_socket());

    assert_eq!(
        config_field_help("weaver-args-locale-help"),
        "Selects the operator-facing locale"
    );

    // The loader signature is part of the contract; loading is allowed to fail
    // when no configuration source is present, so only compilation is asserted.
    let _loaded = Config::load_from_iter(["weaver"]);
    let _trait_loaded: ortho_config::OrthoResult<Config> =
        <Config as OrthoConfig>::load_from_iter(["weaver"]);

    generated_metadata_surface();
}

/// Compiles the generated localization identifiers and documentation metadata
/// that `weaver-cli` renders, including the field access its help path
/// performs. A derive or metadata change must be visible here, not only in
/// in-crate runtime assertions.
fn generated_metadata_surface() {
    let _localization_base: &'static str = Config::LOCALIZATION_BASE;
    let _about_id: &'static str = Config::ABOUT_ID;
    let _usage_id: &'static str = Config::USAGE_ID;
    let arg_ids: &'static [ArgLocalizationIds] = Config::ARG_IDS;
    assert!(arg_ids.iter().any(|ids| ids.name == "locale"));
    assert!(arg_ids.iter().all(|ids| !ids.help_id.is_empty()));

    let metadata = Config::get_doc_metadata();
    assert!(!metadata.ir_version.is_empty());
    assert!(!metadata.fields.is_empty());
    for field in &metadata.fields {
        let _name: &str = &field.name;
        let _help_id: &str = &field.help_id;
        let _required: bool = field.required;
        let _long: Option<&str> = field.cli.as_ref().and_then(|cli| cli.long.as_deref());
        let _hidden: bool = field.cli.as_ref().is_some_and(|cli| cli.hide_in_help);
    }

    let _mounted = Config::get_doc_metadata_for_path(&[String::from("weaver")]);
}
