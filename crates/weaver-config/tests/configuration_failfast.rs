#![cfg(feature = "cli")]

use std::ffi::OsString;

use cap_std::fs::Dir;
use googletest::prelude::*;
use ortho_config::OrthoError;
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_config::Config;
use weaver_test_macros::allow_fixture_expansion_lints;

#[allow_fixture_expansion_lints]
#[fixture]
fn temp_dir() -> TempDir { TempDir::new().expect("create temporary directory") }

#[rstest]
fn malformed_explicit_config_returns_file_error(temp_dir: TempDir) {
    let cli_path = temp_dir.path().join("cli_weaver.toml");
    let dir = Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority())
        .expect("open temp dir");

    dir.write(
        "cli_weaver.toml",
        r#"daemon_socket = { transport = "tcp" host = "127.0.0.1" }"#,
    )
    .expect("write malformed cli config");

    let args = vec![
        OsString::from("weaver"),
        OsString::from("--config-path"),
        cli_path.clone().into_os_string(),
    ];

    let error = Config::load_from_iter(args).expect_err("loading must fail");
    let message = error.to_string();
    assert_that!(
        message.as_str(),
        contains_substring("Configuration file error")
    );

    match error.as_ref() {
        OrthoError::File { path, .. } => assert_eq!(path, &cli_path),
        other => panic!("expected file error, got {other:?}"),
    }
}

#[rstest]
fn missing_extends_reports_referencing_and_resolved_paths(temp_dir: TempDir) {
    let config_path = temp_dir.path().join("weaver.toml");
    let missing_parent_path = temp_dir.path().join("missing.toml");
    let dir = Dir::open_ambient_dir(temp_dir.path(), cap_std::ambient_authority())
        .expect("open temp dir");

    dir.write("weaver.toml", "extends = \"missing.toml\"\n")
        .expect("write extending config");

    let args = vec![
        OsString::from("weaver"),
        OsString::from("--config-path"),
        config_path.clone().into_os_string(),
    ];

    let error = Config::load_from_iter(args).expect_err("missing parent must fail");
    let message = error.to_string();
    let referencing_path = config_path.display().to_string();
    let missing_parent = missing_parent_path.display().to_string();
    let normalized_message = message.replace(&temp_dir.path().display().to_string(), "<TEMP>");

    assert_that!(
        message.as_str(),
        contains_substring(referencing_path.as_str())
    );
    assert_that!(
        message.as_str(),
        contains_substring(missing_parent.as_str())
    );
    insta::assert_snapshot!(normalized_message);
}
