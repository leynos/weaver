//! Integration tests for the `weaver` binary entry point.
//!
//! Verifies the capabilities probe behaviour, version output, help output,
//! and user-facing error handling when required arguments are missing.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::{
    prelude::PredicateBooleanExt,
    str::{contains, is_empty},
};
use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_cli::DOMAIN_OPERATIONS;

const EXPECTED_SHARED_CONFIG_HELP_FLAGS: &[&str] = &[
    "--config-path <PATH>",
    "--daemon-socket <ENDPOINT>",
    "--log-filter <FILTER>",
    "--log-format <FORMAT>",
    "--capability-overrides <DIRECTIVE>",
    "--locale <LOCALE>",
];

/// Builds a scratch `$HOME`/XDG environment and a `weaver` command
/// pre-configured to use it, so integration tests never touch the
/// invoking user's real configuration.
#[fixture]
fn isolated_environment() -> anyhow::Result<(TempDir, assert_cmd::Command)> {
    let temp_dir = TempDir::new()?;
    let mut command = cargo_bin_cmd!("weaver");
    command
        .env_clear()
        .env("HOME", temp_dir.path())
        .env("XDG_CONFIG_HOME", temp_dir.path().join("xdg-config"))
        .env("XDG_CACHE_HOME", temp_dir.path().join("xdg-cache"));
    Ok((temp_dir, command))
}

#[rstest]
fn capabilities_probe_succeeds(
    isolated_environment: anyhow::Result<(TempDir, assert_cmd::Command)>,
) {
    let (_temp_dir, mut command) = isolated_environment.expect("build isolated environment");
    command.arg("--capabilities");
    command.assert().success();
}

#[rstest]
fn invalid_child_environment_locale_fails_capabilities(
    isolated_environment: anyhow::Result<(TempDir, assert_cmd::Command)>,
) {
    let (_temp_dir, mut command) = isolated_environment.expect("build isolated environment");
    command.env("WEAVER_LOCALE", "not_a_locale");
    command.arg("--capabilities");

    command
        .assert()
        .failure()
        .stderr(contains("locale"))
        .stderr(contains("not_a_locale"));
}

#[rstest]
fn cli_locale_overrides_invalid_child_environment_locale(
    isolated_environment: anyhow::Result<(TempDir, assert_cmd::Command)>,
) {
    let (_temp_dir, mut command) = isolated_environment.expect("build isolated environment");
    command
        .env("WEAVER_LOCALE", "not_a_locale")
        .args(["--locale", "en-GB", "--capabilities"]);

    command.assert().success();
}

#[rstest]
fn missing_extends_parent_is_reported_by_the_binary(
    isolated_environment: anyhow::Result<(TempDir, assert_cmd::Command)>,
) {
    use cap_std::{ambient_authority, fs::Dir};

    let (temp_dir, mut command) = isolated_environment.expect("build isolated environment");
    let config_path = temp_dir.path().join("weaver.toml");
    let missing_path = temp_dir.path().join("missing.toml");
    let directory = Dir::open_ambient_dir(temp_dir.path(), ambient_authority())
        .expect("open temporary configuration directory");
    directory
        .write("weaver.toml", "extends = \"missing.toml\"\n")
        .expect("write extending configuration");

    command.args([
        "--config-path",
        config_path.to_str().expect("temporary path is UTF-8"),
        "--capabilities",
    ]);

    command
        .assert()
        .failure()
        .stderr(contains(config_path.display().to_string()))
        .stderr(contains(missing_path.display().to_string()));
}

#[test]
fn missing_operation_exits_with_failure() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("observe");
    command
        .assert()
        .failure()
        .stdout(is_empty())
        .stderr(contains("error: operation required for domain 'observe'"))
        .stderr(contains("get-card"))
        .stderr(contains("weaver observe get-definition --help"));
}

#[test]
fn unknown_domain_exits_with_failure_before_daemon_startup() {
    let mut command = cargo_bin_cmd!("weaver");
    command.args(["unknown-domain", "get-definition"]);
    command
        .assert()
        .failure()
        .stdout(is_empty())
        .stderr(contains("error: unknown domain 'unknown-domain'"))
        .stderr(contains("Valid domains: observe, act, verify"))
        .stderr(predicates::str::contains("Did you mean").not())
        .stderr(predicates::str::contains("Waiting for daemon start...").not());
}

#[test]
fn typo_domain_suggests_closest_known_domain() {
    let mut command = cargo_bin_cmd!("weaver");
    command.args(["obsrve", "get-definition"]);
    let output = command.output().expect("failed to execute weaver");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "process should fail");
    assert!(output.stdout.is_empty(), "stdout should be empty");
    assert!(
        stderr.contains("error: unknown domain 'obsrve'"),
        "stderr should contain unknown domain error"
    );
    assert!(
        stderr.contains("Valid domains: observe, act, verify"),
        "stderr should contain valid domains list"
    );
    assert_eq!(
        stderr.matches("Did you mean 'observe'?").count(),
        1,
        "stderr should contain exactly one suggestion"
    );
    assert!(
        !stderr.contains("Waiting for daemon start..."),
        "stderr should not contain daemon startup message"
    );
}

#[test]
fn help_output_lists_all_domains_and_operations() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--help");
    // We intentionally avoid asserting on the exit code so this test
    // remains valid if --help is later changed to exit 0.
    let output = command.output().expect("failed to execute weaver --help");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("Domains and operations:"),
        "weaver --help output missing header"
    );
    for (domain, _, ops) in DOMAIN_OPERATIONS {
        assert!(
            combined.contains(domain),
            "weaver --help output missing domain {domain:?}"
        );
        for op in *ops {
            assert!(
                combined.contains(op),
                "weaver --help output missing operation {op:?}"
            );
        }
    }

    for flag in EXPECTED_SHARED_CONFIG_HELP_FLAGS {
        assert!(
            combined.contains(flag),
            "weaver --help output missing config flag {flag:?}"
        );
    }
}

#[test]
fn version_flag_exits_successfully() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--version");
    command
        .assert()
        .success()
        .stdout(contains("weaver"))
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn short_version_flag_exits_successfully() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("-V");
    command.assert().success().stdout(contains("weaver"));
}

#[test]
fn help_flag_exits_successfully_with_quick_start() {
    let mut command = cargo_bin_cmd!("weaver");
    command.arg("--help");
    command
        .assert()
        .success()
        .stdout(contains("Quick start:"))
        .stdout(contains("weaver definitions get"));
}

#[test]
fn daemon_start_help_lists_all_config_flags() {
    let mut command = cargo_bin_cmd!("weaver");
    command.args(["daemon", "start", "--help"]);
    let mut assert = command.assert().success();
    for flag in EXPECTED_SHARED_CONFIG_HELP_FLAGS {
        assert = assert.stdout(contains(*flag));
    }
    assert
        .stdout(contains("Starting").not())
        .stdout(contains("started").not())
        .stdout(contains("launch").not())
        .stdout(contains("daemon socket opened").not())
        .stdout(contains("Waiting for daemon start...").not())
        .stderr(is_empty())
        .stderr(contains("Starting").not())
        .stderr(contains("started").not())
        .stderr(contains("launch").not())
        .stderr(contains("daemon socket opened").not())
        .stderr(contains("Waiting for daemon start...").not());
}

#[test]
fn generated_man_page_contains_all_shared_config_flags() {
    use cap_std::{ambient_authority, fs::Dir};

    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates parent")
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let workspace =
        Dir::open_ambient_dir(&workspace_root, ambient_authority()).expect("open workspace root");
    let target = format!("{}-unknown-linux-gnu", std::env::consts::ARCH);
    let man_pages = ["debug", "release"]
        .map(|profile| format!("target/generated-man/{target}/{profile}/weaver.1"));

    let content = man_pages.iter().find_map(|path| {
        workspace
            .read_to_string(path)
            .ok()
            .map(|content| (path, content))
    });
    let Some((man_page_path, content)) = content else {
        panic!("generated man page weaver.1 not found for target {target}");
    };

    for flag in EXPECTED_SHARED_CONFIG_HELP_FLAGS {
        let flag_name = flag.split_whitespace().next().expect("flag has name");
        let roff_flag_name = flag_name.replace('-', "\\-");
        assert!(
            content.contains(flag_name) || content.contains(&roff_flag_name),
            "man page {man_page_path} missing flag {flag_name:?}",
        );
    }
}
