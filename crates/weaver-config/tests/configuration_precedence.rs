//! Integration tests for real Weaver configuration discovery and source
//! precedence through [`Config::load_from_iter`].

#![cfg(feature = "cli")]

use std::{cell::RefCell, collections::BTreeMap, ffi::OsString, sync::Arc};

use cap_std::fs::Dir;
use googletest::prelude::*;
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use pretty_assertions::assert_eq;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use weaver_config::{
    CapabilityDirective,
    CapabilityOverride,
    Config,
    SocketEndpoint,
    default_log_filter,
    default_log_format,
    default_socket_endpoint,
};
use weaver_test_macros::allow_fixture_expansion_lints;

const LOCALE_ENV: &str = "WEAVER_LOCALE";
const SOCKET_ENV: &str = "WEAVER_DAEMON_SOCKET";
const CAPABILITY_OVERRIDES_ENV: &str = "WEAVER_CAPABILITY_OVERRIDES";
const CONFIG_PATH_ENV: &str = "WEAVER_CONFIG_PATH";
const CONFIG_FILE_NAME: &str = "weaver.toml";
type HarnessState = Result<Harness, String>;

struct Harness {
    temp_dir: TempDir,
    file_lines: RefCell<Vec<String>>,
    environment: RefCell<BTreeMap<&'static str, String>>,
    cli: RefCell<Vec<OsString>>,
    resolved: RefCell<Option<Result<Config, String>>>,
}

impl Harness {
    fn new() -> HarnessState {
        Ok(Self {
            temp_dir: TempDir::new()
                .map_err(|error| format!("create temporary directory: {error}"))?,
            file_lines: RefCell::new(Vec::new()),
            environment: RefCell::new(BTreeMap::new()),
            cli: RefCell::new(Vec::new()),
            resolved: RefCell::new(None),
        })
    }

    fn add_file_line(&self, line: impl Into<String>) {
        self.file_lines.borrow_mut().push(line.into());
    }

    fn set_environment(&self, key: &'static str, value: String) {
        self.environment.borrow_mut().insert(key, value);
    }

    fn add_cli_arguments(&self, arguments: impl IntoIterator<Item = OsString>) {
        self.cli.borrow_mut().extend(arguments);
    }

    fn add_file_locale(&self, locale: &str) { self.add_file_line(format!("locale = {locale:?}")); }

    fn add_file_socket(&self, endpoint: &str) -> Result<(), String> {
        let endpoint = endpoint
            .parse::<SocketEndpoint>()
            .map_err(|error| format!("parse file socket endpoint: {error}"))?;
        let SocketEndpoint::Tcp { host, port } = endpoint else {
            return Err(String::from(
                "precedence tests require a TCP socket endpoint",
            ));
        };
        self.add_file_line(format!(
            "daemon_socket = {{ transport = \"tcp\", host = {host:?}, port = {port} }}"
        ));
        Ok(())
    }

    fn write_file(&self) -> Result<Option<std::path::PathBuf>, String> {
        let contents = self.file_lines.borrow();
        if contents.is_empty() {
            return Ok(None);
        }
        let directory =
            Dir::open_ambient_dir(self.temp_dir.path(), cap_std::ambient_authority())
                .map_err(|error| format!("open temporary configuration directory: {error}"))?;
        directory
            .write(CONFIG_FILE_NAME, contents.join("\n").as_bytes())
            .map_err(|error| format!("write temporary configuration file: {error}"))?;
        Ok(Some(self.temp_dir.path().join(CONFIG_FILE_NAME)))
    }

    fn resolve(&self) -> Result<(), String> {
        if self.resolved.borrow().is_some() {
            return Ok(());
        }
        let config_path = self.write_file()?;
        let mut environment = self
            .environment
            .borrow()
            .iter()
            .fold(MapEnv::new(), |source, (key, value)| {
                source.with_var(*key, value.clone())
            });
        if let Some(path) = config_path {
            environment =
                environment.with_var(CONFIG_PATH_ENV, path.to_string_lossy().into_owned());
        }
        let environment = Arc::new(environment);
        let discovery: SharedEnvSource = environment.clone();
        let merge: SharedScanEnvSource = environment;
        let arguments = std::iter::once(OsString::from("weaver"))
            .chain(self.cli.borrow().iter().cloned())
            .collect::<Vec<_>>();
        self.resolved.replace(Some(
            Config::load_from_iter_with_sources(arguments, discovery, merge)
                .map_err(|error| error.to_string()),
        ));
        Ok(())
    }

    fn config(&self) -> Result<Config, String> {
        self.resolved
            .borrow()
            .as_ref()
            .ok_or_else(|| String::from("configuration should load before assertions"))?
            .clone()
    }

    fn error(&self) -> Result<String, String> {
        match self.config() {
            Ok(config) => Err(format!("configuration should fail, got {config:?}")),
            Err(error) => Ok(error),
        }
    }
}

#[allow_fixture_expansion_lints]
#[fixture]
fn harness() -> HarnessState { Harness::new() }

fn configured_harness(harness: &HarnessState) -> Result<&Harness, String> {
    harness.as_ref().map_err(Clone::clone)
}

#[given("a configuration file setting the locale to \"{locale}\"")]
fn given_file_locale(harness: &HarnessState, locale: String) -> Result<(), String> {
    configured_harness(harness)?.add_file_locale(&locale);
    Ok(())
}

#[given("the environment overrides the locale to \"{locale}\"")]
fn given_environment_locale(harness: &HarnessState, locale: String) -> Result<(), String> {
    configured_harness(harness)?.set_environment(LOCALE_ENV, locale);
    Ok(())
}

#[given("a configuration file setting the daemon socket to \"{endpoint}\"")]
fn given_file_socket(harness: &HarnessState, endpoint: String) -> Result<(), String> {
    configured_harness(harness)?.add_file_socket(&endpoint)
}

#[given("the environment overrides the daemon socket to \"{endpoint}\"")]
fn given_environment_socket(harness: &HarnessState, endpoint: String) -> Result<(), String> {
    configured_harness(harness)?.set_environment(SOCKET_ENV, endpoint);
    Ok(())
}

#[given("a configuration file allowing the Rust rename capability")]
fn given_file_allows_rename(harness: &HarnessState) -> Result<(), String> {
    configured_harness(harness)?.add_file_line(
        "[[capability_overrides]]\nlanguage = \"Rust\"\ncapability = \
         \"observe.rename\"\ndirective = \"allow\"",
    );
    Ok(())
}

#[given("the environment forces the Rust rename capability")]
fn given_environment_forces_rename(harness: &HarnessState) -> Result<(), String> {
    // OrthoConfig parses environment values with Figment's TOML-like inline
    // syntax, not the directive's `Display` spelling or JSON, so a collection
    // must be encoded as an array of inline tables.
    configured_harness(harness)?.set_environment(
        CAPABILITY_OVERRIDES_ENV,
        String::from(
            r#"[{language = "Rust", capability = "observe.rename", directive = "force"}]"#,
        ),
    );
    Ok(())
}

#[when("the CLI sets the locale to \"{locale}\"")]
fn when_cli_locale(harness: &HarnessState, locale: String) -> Result<(), String> {
    let harness = configured_harness(harness)?;
    harness.add_cli_arguments([OsString::from("--locale"), OsString::from(locale)]);
    harness.resolve()
}

#[when("the configuration loads")]
fn when_configuration_loads(harness: &HarnessState) -> Result<(), String> {
    configured_harness(harness)?.resolve()
}

#[when("the configuration loads without overrides")]
fn when_defaults_load(harness: &HarnessState) -> Result<(), String> {
    configured_harness(harness)?.resolve()
}

#[when("the environment sets the locale to \"{locale}\"")]
fn when_invalid_environment_locale(harness: &HarnessState, locale: String) -> Result<(), String> {
    let harness = configured_harness(harness)?;
    harness.set_environment(LOCALE_ENV, locale);
    harness.resolve()
}

#[when("the CLI sets the daemon socket to \"{endpoint}\"")]
fn when_cli_socket(harness: &HarnessState, endpoint: String) -> Result<(), String> {
    let harness = configured_harness(harness)?;
    harness.add_cli_arguments([OsString::from("--daemon-socket"), OsString::from(endpoint)]);
    harness.resolve()
}

#[when("the CLI denies the Rust rename capability")]
fn when_cli_denies_rename(harness: &HarnessState) -> Result<(), String> {
    let harness = configured_harness(harness)?;
    harness.add_cli_arguments([
        OsString::from("--capability-overrides"),
        OsString::from("rust:observe.rename=deny"),
    ]);
    harness.resolve()
}

#[then("loading the configuration resolves the locale to \"{locale}\"")]
fn then_locale_is(harness: &HarnessState, locale: String) -> Result<(), String> {
    assert_eq!(
        configured_harness(harness)?.config()?.locale().to_string(),
        locale
    );
    Ok(())
}

#[then("loading the configuration applies the built-in defaults")]
fn then_defaults_are_returned(harness: &HarnessState) -> Result<(), String> {
    let config = configured_harness(harness)?.config()?;
    assert_eq!(config.daemon_socket(), &default_socket_endpoint());
    assert_eq!(config.log_filter(), default_log_filter());
    assert_eq!(config.log_format(), default_log_format());
    assert_eq!(config.locale(), Config::default().locale());
    assert_that!(config.capability_matrix().languages, is_empty());
    Ok(())
}

#[then("configuration loading reports an invalid locale")]
fn then_invalid_locale_is_reported(harness: &HarnessState) -> Result<(), String> {
    let error = configured_harness(harness)?.error()?;
    assert_that!(error.as_str(), contains_substring("locale"));
    Ok(())
}

#[then("loading the configuration resolves the daemon socket to \"{endpoint}\"")]
fn then_socket_is(harness: &HarnessState, endpoint: String) -> Result<(), String> {
    assert_eq!(
        configured_harness(harness)?
            .config()?
            .daemon_socket()
            .to_string(),
        endpoint
    );
    Ok(())
}

#[then(
    "loading the configuration resolves the capability matrix to deny the Rust rename capability"
)]
fn then_rename_is_denied(harness: &HarnessState) -> Result<(), String> {
    let config = configured_harness(harness)?.config()?;
    assert_eq!(
        config.capability_overrides,
        vec![CapabilityDirective::new(
            "rust",
            "observe.rename",
            CapabilityOverride::Deny,
        )]
    );
    assert_eq!(
        config
            .capability_matrix()
            .override_for("rust", "observe.rename"),
        Some(CapabilityOverride::Deny)
    );
    Ok(())
}

// Bind every scenario by name. `#[scenario(path = ...)]` without `name` or
// `index` expands only the first scenario in the file, which silently leaves
// the rest unexercised.

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "CLI locale overrides environment and configuration file values"
)]
fn cli_locale_overrides_lower_layers(#[from(harness)] _harness: HarnessState) {}

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "Explicit CLI value equal to the default overrides lower layers"
)]
fn explicit_cli_default_overrides_lower_layers(#[from(harness)] _harness: HarnessState) {}

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "Environment locale overrides a configuration file value"
)]
fn environment_locale_overrides_file(#[from(harness)] _harness: HarnessState) {}

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "Configuration file locale overrides the built-in default"
)]
fn file_locale_overrides_builtin_default(#[from(harness)] _harness: HarnessState) {}

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "Defaults are returned when no configuration sources are provided"
)]
fn defaults_without_sources(#[from(harness)] _harness: HarnessState) {}

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "Invalid environment locale fails closed"
)]
fn invalid_environment_locale_fails_closed(#[from(harness)] _harness: HarnessState) {}

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "CLI daemon socket overrides environment and configuration file values"
)]
fn cli_socket_overrides_lower_layers(#[from(harness)] _harness: HarnessState) {}

#[scenario(
    path = "tests/features/configuration_precedence.feature",
    name = "The last duplicate capability directive wins during configuration loading"
)]
fn last_duplicate_capability_directive_wins(#[from(harness)] _harness: HarnessState) {}
