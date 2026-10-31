//! Downstream compile-time usage of the `weaver-config` public API.
//!
//! The migration to OrthoConfig v0.10.0 changes the generated metadata and the
//! derive that supplies `Config`. This fixture keeps the caller-facing surface
//! compiling from outside the crate: the default configuration, the accessors
//! that read it, the capability matrix built from directives, and the help
//! lookup the CLI help surfaces rely on.

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
}