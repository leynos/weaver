//! Tests for clap help output augmented with shared configuration flags.

use std::ffi::OsString;
use std::io::Cursor;
use std::process::ExitCode;

use crate::help;
use crate::{AppError, ConfigLoader, IoStreams, run_with_loader};
use weaver_config::Config;

struct PanickingLoader;

impl ConfigLoader for PanickingLoader {
    fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
        panic!("help output must not attempt configuration loading");
    }
}

fn run_with_args(args: &[&str]) -> (ExitCode, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let owned_args = args
        .iter()
        .map(|value| OsString::from(*value))
        .collect::<Vec<_>>();
    let exit = run_with_loader(owned_args, &mut io, &PanickingLoader);
    (
        exit,
        String::from_utf8(stdout).expect("stdout utf8"),
        String::from_utf8(stderr).expect("stderr utf8"),
    )
}

fn assert_config_flags_present(text: &str) {
    for flag in [
        "--config-path <PATH>",
        "--daemon-socket <ENDPOINT>",
        "--log-filter <FILTER>",
        "--log-format <FORMAT>",
        "--capability-overrides <DIRECTIVE>",
        "--locale <LOCALE>",
    ] {
        assert!(text.contains(flag), "help output missing {flag:?}");
    }
}

#[test]
fn top_level_help_lists_shared_config_flags() {
    let (exit, stdout, stderr) = run_with_args(&["weaver", "--help"]);
    assert_eq!(exit, ExitCode::SUCCESS);
    assert!(stderr.is_empty(), "help output must not write to stderr");
    assert_config_flags_present(&stdout);
}

#[test]
fn daemon_start_help_lists_shared_config_flags() {
    let (exit, stdout, stderr) = run_with_args(&["weaver", "daemon", "start", "--help"]);
    assert_eq!(exit, ExitCode::SUCCESS);
    assert!(stderr.is_empty(), "help output must not write to stderr");
    assert_config_flags_present(&stdout);
}

#[test]
fn top_level_help_snapshot_matches_augmented_command() {
    let rendered = help::command().render_long_help().to_string();
    insta::assert_snapshot!("top_level_augmented_help", rendered);
}

#[test]
fn daemon_start_help_snapshot_matches_augmented_command() {
    let mut command = help::command();
    let daemon = command
        .find_subcommand_mut("daemon")
        .expect("daemon subcommand must exist");
    let rendered = daemon
        .find_subcommand_mut("start")
        .expect("daemon start subcommand must exist")
        .render_long_help()
        .to_string();
    insta::assert_snapshot!("daemon_start_augmented_help", rendered);
}
