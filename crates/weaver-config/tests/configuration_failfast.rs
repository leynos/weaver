#![cfg(feature = "cli")]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::sync::{Mutex, MutexGuard};

use once_cell::sync::Lazy;
use ortho_config::OrthoError;
use tempfile::TempDir;
use weaver_config::Config;

static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct EnvOverride {
    key: &'static str,
    previous: Option<OsString>,
    guard: Option<MutexGuard<'static, ()>>,
}

impl EnvOverride {
    fn set_var(key: &'static str, value: &OsStr) -> Self {
        let guard = ENV_MUTEX.lock().expect("env mutex poisoned");
        let previous = std::env::var_os(key);
        // Nightly currently marks environment mutation as unsafe while the API
        // stabilises, so mirror the pattern used in other tests.
        unsafe { std::env::set_var(key, value) };
        Self {
            key,
            previous,
            guard: Some(guard),
        }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        // Restore any previous value (or remove the override) so other tests
        // inherit a clean environment.
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
        drop(self.guard.take());
    }
}

#[test]
fn malformed_configs_return_aggregated_error() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let cli_path = temp_dir.path().join("cli_weaver.toml");
    let env_path = temp_dir.path().join("env_weaver.toml");

    fs::write(
        &cli_path,
        r#"daemon_socket = { transport = "tcp" host = "127.0.0.1" }"#,
    )
    .expect("write malformed cli config");
    fs::write(
        &env_path,
        r#"daemon_socket = { transport = "tcp", port = not_a_number }"#,
    )
    .expect("write malformed env config");

    let _env = EnvOverride::set_var("WEAVER_CONFIG_PATH", env_path.as_os_str());

    let args = vec![
        OsString::from("weaver-cli"),
        OsString::from("--config-path"),
        cli_path.clone().into_os_string(),
    ];

    let error = Config::load_from_iter(args).expect_err("loading must fail");
    let message = error.to_string();
    assert!(
        message.contains("multiple configuration errors"),
        "expected aggregate message, got {message:?}"
    );

    match error.as_ref() {
        OrthoError::Aggregate(aggregate) => {
            let mut mentioned_paths = aggregate
                .iter()
                .filter_map(|err| match err {
                    OrthoError::File { path, .. } => Some(path.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            mentioned_paths.sort();

            assert_eq!(
                mentioned_paths.len(),
                2,
                "expected both failing files to be reported, got {mentioned_paths:?}"
            );
            assert!(
                mentioned_paths.contains(&cli_path),
                "missing CLI path in aggregate: {mentioned_paths:?}"
            );
            assert!(
                mentioned_paths.contains(&env_path),
                "missing env path in aggregate: {mentioned_paths:?}"
            );
        }
        other => panic!("expected aggregated error, got {other:?}"),
    }
}
