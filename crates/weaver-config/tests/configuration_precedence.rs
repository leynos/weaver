#![cfg(feature = "cli")]

use std::cell::RefCell;

use googletest::prelude::*;
use ortho_config::MergeComposer;
use pretty_assertions::assert_eq;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::{Value, json};
use weaver_config::{Config, default_log_filter, default_log_format, default_socket_endpoint};
use weaver_test_macros::allow_fixture_expansion_lints;

struct Harness {
    defaults: Value,
    file: RefCell<Option<Value>>,
    environment: RefCell<Option<Value>>,
    cli: RefCell<Option<Value>>,
    resolved: RefCell<Option<Result<Config, String>>>,
}

impl Harness {
    fn new() -> Self {
        Self {
            defaults: json!({
                "daemon_socket": default_socket_endpoint(),
                "log_filter": default_log_filter(),
                "log_format": default_log_format(),
                "capability_overrides": [],
                "locale": "en-US",
            }),
            file: RefCell::new(None),
            environment: RefCell::new(None),
            cli: RefCell::new(None),
            resolved: RefCell::new(None),
        }
    }

    fn set_file(&self, layer: Value) { self.file.replace(Some(layer)); }

    fn set_environment(&self, layer: Value) { self.environment.replace(Some(layer)); }

    fn set_cli(&self, layer: Value) { self.cli.replace(Some(layer)); }

    fn resolve(&self) {
        if self.resolved.borrow().is_some() {
            return;
        }

        let mut composer = MergeComposer::with_capacity(4);
        composer.push_defaults(self.defaults.clone());
        if let Some(layer) = self.file.borrow().clone() {
            composer.push_file(layer, None);
        }
        if let Some(layer) = self.environment.borrow().clone() {
            composer.push_environment(layer);
        }
        if let Some(layer) = self.cli.borrow().clone() {
            composer.push_cli(layer);
        }

        self.resolved.replace(Some(
            Config::merge_from_layers(composer.layers()).map_err(|error| error.to_string()),
        ));
    }

    fn resolution(&self) -> Result<Config, String> {
        self.resolve();
        self.resolved
            .borrow()
            .as_ref()
            .expect("configuration result should be present")
            .clone()
    }

    fn config(&self) -> Config { self.resolution().expect("configuration should resolve") }

    fn error(&self) -> String { self.resolution().expect_err("configuration should fail") }
}

#[allow_fixture_expansion_lints]
#[fixture]
fn harness() -> Harness { Harness::new() }

#[given("a configuration file setting the locale to \"{locale}\"")]
fn given_file_locale(harness: &Harness, locale: String) {
    harness.set_file(json!({ "locale": locale }));
}

#[given("an environment layer setting the locale to \"{locale}\"")]
fn given_environment_locale(harness: &Harness, locale: String) {
    harness.set_environment(json!({ "locale": locale }));
}

#[given("lower layers allow the Rust rename capability")]
fn given_lower_layers_allow_rename(harness: &Harness) {
    harness.set_file(json!({
        "capability_overrides": [{
            "language": "Rust",
            "capability": "observe.rename",
            "directive": "allow",
        }],
    }));
}

#[when("a CLI layer sets the locale to \"{locale}\"")]
fn when_cli_locale(harness: &Harness, locale: String) {
    harness.set_cli(json!({ "locale": locale }));
}

#[when("the configuration layers are merged without overrides")]
fn when_defaults_merge(harness: &Harness) { harness.resolve(); }

#[when("an environment layer sets the locale to \"{locale}\"")]
fn when_invalid_environment_locale(harness: &Harness, locale: String) {
    harness.set_environment(json!({ "locale": locale }));
    harness.resolve();
}

#[when("a CLI layer denies the Rust rename capability")]
fn when_cli_denies_rename(harness: &Harness) {
    harness.set_cli(json!({
        "capability_overrides": [{
            "language": "rust",
            "capability": "observe.rename",
            "directive": "deny",
        }],
    }));
}

#[then("the resolved locale is \"{locale}\"")]
fn then_locale_is(harness: &Harness, locale: String) {
    assert_eq!(harness.config().locale().to_string(), locale);
}

#[then("the built-in Weaver defaults are returned")]
fn then_defaults_are_returned(harness: &Harness) {
    let config = harness.config();
    assert_eq!(config.daemon_socket(), &default_socket_endpoint());
    assert_eq!(config.log_filter(), default_log_filter());
    assert_eq!(config.log_format(), default_log_format());
    assert_eq!(config.locale().to_string(), "en-US");
    assert_that!(config.capability_matrix().languages, is_empty());
}

#[then("configuration loading reports an invalid locale")]
fn then_invalid_locale_is_reported(harness: &Harness) {
    assert_that!(harness.error().as_str(), contains_substring("locale"));
}

#[then("the resolved capability matrix denies the Rust rename capability")]
fn then_rename_is_denied(harness: &Harness) {
    let matrix = harness.config().capability_matrix();
    assert_eq!(
        matrix.override_for("rust", "observe.rename"),
        Some(weaver_config::CapabilityOverride::Deny)
    );
}

#[scenario(path = "tests/features/configuration_precedence.feature")]
fn configuration_precedence(#[from(harness)] _harness: Harness) {}
