use std::ffi::OsString;
use std::fs;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;

use weaver_config::{
    Config, SocketEndpoint, default_log_filter, default_log_format, default_socket_endpoint,
};

struct Harness {
    temp_dir: TempDir,
    cli_args: std::cell::RefCell<Vec<OsString>>,
    env_overrides: std::cell::RefCell<Vec<(String, Option<OsString>)>>,
    loaded: std::cell::RefCell<Option<Config>>,
    error: std::cell::RefCell<Option<String>>,
}

impl Harness {
    fn new() -> Self {
        let temp_dir = match TempDir::new() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create temporary directory: {error}"),
        };
        Self {
            temp_dir,
            cli_args: std::cell::RefCell::new(vec![OsString::from("weaver-cli")]),
            env_overrides: std::cell::RefCell::new(Vec::new()),
            loaded: std::cell::RefCell::new(None),
            error: std::cell::RefCell::new(None),
        }
    }

    fn write_config(&self, socket: &SocketEndpoint) {
        let path = self.temp_dir.path().join("weaver.toml");
        let toml = match socket {
            SocketEndpoint::Unix { path } => {
                format!(
                    "daemon_socket = {{ transport = \"unix\", path = \"{}\" }}\n",
                    path
                )
            }
            SocketEndpoint::Tcp { host, port } => format!(
                "daemon_socket = {{ transport = \"tcp\", host = \"{}\", port = {} }}\n",
                host, port
            ),
        };

        if let Err(error) = fs::write(&path, toml) {
            panic!("failed to write configuration: {error}");
        }

        let mut args = self.cli_args.borrow_mut();
        args.push(OsString::from("--config-path"));
        args.push(path.into_os_string());
    }

    fn set_env(&self, key: &str, value: &str) {
        let previous = std::env::var_os(key);
        // The nightly toolchain marks environment mutation as `unsafe` while the
        // API stabilises. The harness restores overrides in `Drop` to keep the
        // wider process environment unchanged.
        unsafe { std::env::set_var(key, value) };
        self.env_overrides
            .borrow_mut()
            .push((key.to_string(), previous));
    }

    fn push_cli_arg(&self, arg: impl Into<OsString>) {
        self.cli_args.borrow_mut().push(arg.into());
    }

    fn load(&self) {
        if self.loaded.borrow().is_some() || self.error.borrow().is_some() {
            return;
        }

        let args = self.cli_args.borrow().clone();
        match Config::load_from_iter(args) {
            Ok(config) => {
                *self.loaded.borrow_mut() = Some(config);
            }
            Err(error) => {
                *self.error.borrow_mut() = Some(error.to_string());
            }
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let mut overrides = self.env_overrides.borrow_mut();
        while let Some((key, value)) = overrides.pop() {
            if let Some(os_value) = value {
                unsafe { std::env::set_var(&key, os_value) };
            } else {
                unsafe { std::env::remove_var(&key) };
            }
        }
    }
}

#[fixture]
fn harness() -> Harness {
    Harness::new()
}

#[given("a configuration file setting the daemon socket to \"{socket}\"")]
fn given_configuration_file(harness: &Harness, socket: String) {
    let endpoint = match socket.parse::<SocketEndpoint>() {
        Ok(endpoint) => endpoint,
        Err(error) => panic!("invalid socket '{socket}': {error}"),
    };
    harness.write_config(&endpoint);
}

#[given("the environment overrides the daemon socket to \"{socket}\"")]
fn given_environment_override(harness: &Harness, socket: String) {
    harness.set_env("WEAVER_DAEMON_SOCKET", &socket);
}

#[when("the CLI sets the daemon socket to \"{socket}\"")]
fn when_cli_override(harness: &Harness, socket: String) {
    harness.push_cli_arg("--daemon-socket");
    harness.push_cli_arg(OsString::from(&socket));
}

#[when("the configuration loads without overrides")]
fn when_load_without_overrides(harness: &Harness) {
    harness.load();
}

#[then("loading the configuration resolves the daemon socket to \"{socket}\"")]
fn then_resolved_socket(harness: &Harness, socket: String) {
    harness.load();

    if let Some(error) = harness.error.borrow().as_ref() {
        panic!("configuration failed to load: {error}");
    }

    let loaded = harness.loaded.borrow();
    let config = match loaded.as_ref() {
        Some(config) => config,
        None => panic!("configuration was not loaded"),
    };

    let expected = match socket.parse::<SocketEndpoint>() {
        Ok(endpoint) => endpoint,
        Err(error) => panic!("invalid expected socket '{socket}': {error}"),
    };

    assert_eq!(config.daemon_socket(), &expected);
}

#[then("loading the configuration applies the built-in defaults")]
fn then_defaults_applied(harness: &Harness) {
    harness.load();

    if let Some(error) = harness.error.borrow().as_ref() {
        panic!("configuration failed to load: {error}");
    }

    let loaded = harness.loaded.borrow();
    let config = match loaded.as_ref() {
        Some(config) => config,
        None => panic!("configuration was not loaded"),
    };

    assert_eq!(config.daemon_socket(), &default_socket_endpoint());
    assert_eq!(config.log_filter(), default_log_filter());
    assert_eq!(config.log_format(), default_log_format());

    let matrix = config.capability_matrix();
    assert!(
        matrix.languages.is_empty(),
        "expected no capability overrides"
    );
}

#[scenario(path = "tests/features/configuration_precedence.feature")]
fn configuration_precedence(#[from(harness)] harness: Harness) {
    let _ = harness;
}
