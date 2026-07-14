//! Tests for clap help output augmented with shared configuration flags.

use std::{ffi::OsString, io::Cursor, process::ExitCode};

use rstest::rstest;
use weaver_config::Config;

use crate::{AppError, ConfigLoader, IoStreams, help, run_with_loader};

/// Test-local mirror of the shared configuration help flags.
/// Must be kept in sync with `SHARED_CONFIG_HELP_FLAGS` in `lib.rs`.
/// If this constant drifts, tests will fail, surfacing the discrepancy.
const EXPECTED_SHARED_CONFIG_HELP_FLAGS: &[&str] = &[
    "--config-path <PATH>",
    "--daemon-socket <ENDPOINT>",
    "--log-filter <FILTER>",
    "--log-format <FORMAT>",
    "--capability-overrides <DIRECTIVE>",
    "--locale <LOCALE>",
];

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
    for flag in EXPECTED_SHARED_CONFIG_HELP_FLAGS {
        assert!(text.contains(flag), "help output missing {flag:?}");
    }
}

#[rstest]
#[case(&["weaver", "--help"])]
#[case(&["weaver", "daemon", "start", "--help"])]
#[case(&["weaver", "--config-path", "dummy.toml", "--help"])]
#[case(&["weaver", "--log-format", "JSON", "--help"])]
fn help_lists_shared_config_flags_without_loading_config(#[case] argv: &[&str]) {
    let (exit, stdout, stderr) = run_with_args(argv);
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

#[test]
fn augmented_command_has_expected_arg_structure() {
    use clap::ArgAction;

    let expected: &[(&str, Option<&str>, ArgAction)] = &[
        ("config-path", Some("PATH"), ArgAction::Set),
        ("daemon-socket", Some("ENDPOINT"), ArgAction::Set),
        ("log-filter", Some("FILTER"), ArgAction::Set),
        ("log-format", Some("FORMAT"), ArgAction::Set),
        ("capability-overrides", Some("DIRECTIVE"), ArgAction::Append),
        ("locale", Some("LOCALE"), ArgAction::Set),
    ];

    let cmd = help::command();
    for (long, expected_value_name, expected_action) in expected {
        let Some(arg) = cmd.get_arguments().find(|a| a.get_long() == Some(*long)) else {
            panic!("augmented command missing --{long}");
        };

        assert!(arg.is_global_set(), "arg --{long} must be global");
        assert_eq!(
            arg.get_value_names()
                .and_then(|names| names.first().map(|value| value.as_str())),
            *expected_value_name,
            "arg --{long} value_name mismatch"
        );
        assert_eq!(
            format!("{:?}", arg.get_action()),
            format!("{expected_action:?}"),
            "arg --{long} action mismatch"
        );
    }
}

#[test]
fn config_flag_after_domain_is_not_extracted_as_config_argument() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let load_called = Arc::new(AtomicBool::new(false));
    let load_called_clone = Arc::clone(&load_called);

    struct TrackingLoader(Arc<AtomicBool>);

    impl ConfigLoader for TrackingLoader {
        fn load(&self, args: &[OsString]) -> Result<Config, AppError> {
            self.0.store(true, Ordering::SeqCst);
            let contains_locale = args.iter().any(|arg| arg == "--locale");
            assert!(
                !contains_locale,
                "post-domain --locale must not reach the config loader"
            );
            Err(AppError::MissingDomain)
        }
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let argv: Vec<OsString> = ["weaver", "observe", "status", "--locale", "de-DE"]
        .iter()
        .map(OsString::from)
        .collect();
    let exit = run_with_loader(argv, &mut io, &TrackingLoader(load_called_clone));

    assert!(
        load_called.load(Ordering::SeqCst),
        "config loader must be called for domain command routing"
    );
    assert_eq!(exit, ExitCode::FAILURE);
}

#[test]
fn write_help_for_args_surfaces_io_error_on_broken_writer() {
    struct BrokenWriter;

    impl std::io::Write for BrokenWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }

    let args: Vec<OsString> = vec![OsString::from("weaver"), OsString::from("--help")];
    let result = crate::help::write_help_for_args(&args, &mut BrokenWriter);
    assert!(result.is_err());
    assert_eq!(
        result.expect_err("broken writer should fail").kind(),
        std::io::ErrorKind::BrokenPipe
    );
}
