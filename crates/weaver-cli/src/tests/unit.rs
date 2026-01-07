//! Unit tests for Weaver CLI core functionality.
//!
//! Exercises command serialisation, daemon message parsing, configuration
//! loading, and socket connection establishment (TCP and Unix domain sockets).

use super::support::{accept_tcp_connection, decode_utf8, default_daemon_lines, read_fixture};

#[cfg(unix)]
use super::support::accept_unix_connection;

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{self, Cursor};
use std::net::TcpListener;
use std::process::ExitCode;
use std::thread;

use crate::{
    AppError, Cli, CliCommand, CommandDescriptor, CommandInvocation, CommandRequest, ConfigLoader,
    DaemonAction, EMPTY_LINE_LIMIT, IoStreams, connect, exit_code_from_status,
    is_daemon_not_running, read_daemon_messages, run_with_loader,
};
use clap::Parser;
use rstest::rstest;
use weaver_config::{Config, SocketEndpoint};

#[test]
fn serialises_command_request_matches_golden() {
    let invocation = CommandInvocation {
        domain: String::from("observe"),
        operation: String::from("get-definition"),
        arguments: vec![String::from("--symbol"), String::from("main")],
    };
    let request = CommandRequest::from(invocation);
    let mut buffer: Vec<u8> = Vec::new();
    request
        .write_jsonl(&mut buffer)
        .expect("serialises request");
    let actual = decode_utf8(buffer, "request").expect("decode request to utf8");
    let expected =
        read_fixture("request_observe_get_definition.jsonl").expect("load golden request");
    assert_eq!(actual, expected);
}

#[rstest]
#[case(None, Some(String::from("op")), "MissingDomain", "requires domain")]
#[case(
    Some(String::from("   ")),
    Some(String::from("op")),
    "MissingDomain",
    "rejects blank domain"
)]
#[case(
    Some(String::from("observe")),
    None,
    "MissingOperation",
    "requires operation"
)]
#[case(
    Some(String::from("observe")),
    Some(String::from("   ")),
    "MissingOperation",
    "rejects blank operation"
)]
fn command_invocation_validation(
    #[case] domain: Option<String>,
    #[case] operation: Option<String>,
    #[case] expected_error: &str,
    #[case] _description: &str,
) {
    let cli = Cli {
        capabilities: false,
        command: None,
        domain,
        operation,
        arguments: Vec::new(),
    };

    let error = CommandInvocation::try_from(cli).expect_err("validation must fail");

    match expected_error {
        "MissingDomain" => assert!(matches!(error, AppError::MissingDomain)),
        "MissingOperation" => assert!(matches!(error, AppError::MissingOperation)),
        other => panic!("unexpected expected_error marker: {}", other),
    }
}

#[test]
fn cli_parses_daemon_subcommand() {
    let cli = Cli::try_parse_from(["weaver", "daemon", "status"]).expect("parse daemon");
    match cli.command {
        Some(CliCommand::Daemon {
            action: DaemonAction::Status,
        }) => {}
        other => panic!("expected daemon status command, got {other:?}"),
    }
}

#[rstest]
#[case(0, ExitCode::SUCCESS)]
#[case(17, ExitCode::from(17))]
#[case(255, ExitCode::from(255))]
fn exit_code_from_status_within_range(#[case] status: i32, #[case] expected: ExitCode) {
    assert_eq!(exit_code_from_status(status), expected);
}

#[test]
fn exit_code_from_status_out_of_range_defaults_to_failure() {
    assert_eq!(exit_code_from_status(300), ExitCode::FAILURE);
}

#[test]
fn read_daemon_messages_errors_without_exit() {
    let input = b"{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"hi\"}\n";
    let mut cursor = Cursor::new(&input[..]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = read_daemon_messages(&mut cursor, &mut stdout, &mut stderr).unwrap_err();
    assert!(matches!(error, AppError::MissingExit));
    let stdout_text = decode_utf8(stdout, "stdout").expect("decode stdout");
    assert_eq!(stdout_text, "hi");
}

#[test]
fn read_daemon_messages_warns_after_empty_lines() {
    let mut payload = Vec::new();
    for _ in 0..EMPTY_LINE_LIMIT {
        payload.extend_from_slice(b"\n");
    }
    let mut cursor = Cursor::new(payload);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = read_daemon_messages(&mut cursor, &mut stdout, &mut stderr).unwrap_err();
    assert!(matches!(error, AppError::MissingExit));
    let warning = decode_utf8(stderr, "stderr").expect("decode stderr");
    assert!(warning.contains("Warning: received"));
}

#[test]
fn read_daemon_messages_fails_on_malformed_json() {
    let mut cursor = Cursor::new(Vec::from("this is not json\n"));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = read_daemon_messages(&mut cursor, &mut stdout, &mut stderr).unwrap_err();
    assert!(matches!(error, AppError::ParseMessage(_)));
}

#[test]
fn run_with_loader_reports_configuration_failures() {
    struct FailingLoader;

    impl ConfigLoader for FailingLoader {
        fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
            Err(AppError::MissingDomain)
        }
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut io = IoStreams::new(&mut stdout, &mut stderr);
    let exit = run_with_loader(vec![OsString::from("weaver")], &mut io, &FailingLoader);
    assert_eq!(exit, ExitCode::FAILURE);
    let stderr_text = decode_utf8(stderr, "stderr").expect("decode stderr");
    assert!(stderr_text.contains("command domain"));
}

#[test]
fn run_with_loader_filters_configuration_arguments() {
    struct RecordingLoader {
        recorded: RefCell<Vec<OsString>>,
    }

    impl RecordingLoader {
        fn new() -> Self {
            Self {
                recorded: RefCell::new(Vec::new()),
            }
        }
    }

    impl ConfigLoader for RecordingLoader {
        fn load(&self, args: &[OsString]) -> Result<Config, AppError> {
            self.recorded.borrow_mut().extend(args.iter().cloned());
            Err(AppError::MissingDomain)
        }
    }

    let loader = RecordingLoader::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut io = IoStreams::new(&mut stdout, &mut stderr);
    let exit = run_with_loader(
        vec![
            OsString::from("weaver"),
            OsString::from("--config-path"),
            OsString::from("custom.toml"),
            OsString::from("--log-filter"),
            OsString::from("debug"),
            OsString::from("observe"),
            OsString::from("get-definition"),
            OsString::from("--"),
            OsString::from("--extra"),
        ],
        &mut io,
        &loader,
    );

    assert_eq!(exit, ExitCode::FAILURE);
    let recorded: Vec<String> = loader
        .recorded
        .borrow()
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        recorded,
        vec![
            String::from("weaver"),
            String::from("--config-path"),
            String::from("custom.toml"),
            String::from("--log-filter"),
            String::from("debug"),
        ]
    );

    assert!(!stderr.is_empty());
    assert!(stdout.is_empty());
}

/// Exercises a full daemon connection cycle: connect, write JSONL request,
/// read daemon messages, and verify exit status. The caller provides a setup
/// closure that spawns a listener thread and returns the endpoint.
fn test_daemon_connection<F>(setup_listener: F)
where
    F: FnOnce() -> (SocketEndpoint, thread::JoinHandle<()>),
{
    let (endpoint, handle) = setup_listener();

    let mut connection =
        connect(&endpoint).unwrap_or_else(|error| panic!("connect to daemon: {error}"));
    let request = CommandRequest {
        command: CommandDescriptor {
            domain: "observe".into(),
            operation: "noop".into(),
        },
        arguments: Vec::new(),
    };
    request
        .write_jsonl(&mut connection)
        .unwrap_or_else(|error| panic!("write request: {error}"));

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = read_daemon_messages(&mut connection, &mut stdout, &mut stderr)
        .unwrap_or_else(|error| panic!("read responses: {error}"));
    assert_eq!(status, 17);

    handle.join().expect("listener thread panicked");
}

#[test]
fn connect_successfully_establishes_tcp_connection() {
    test_daemon_connection(|| {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let port = listener.local_addr().expect("listener local addr").port();

        let handle = thread::spawn(move || {
            if let Err(error) = accept_tcp_connection(listener, default_daemon_lines()) {
                panic!("tcp listener failed: {error:?}");
            }
        });

        (SocketEndpoint::tcp("127.0.0.1", port), handle)
    });
}

#[cfg(unix)]
#[test]
fn connect_supports_unix_sockets() {
    use std::os::unix::net::UnixListener;

    let dir = tempfile::tempdir().expect("tempdir");
    let socket_path = dir.path().join("daemon.sock");
    let socket_path_for_listener = socket_path.clone();
    let socket_display = socket_path
        .to_str()
        .expect("socket path is utf8")
        .to_owned();

    test_daemon_connection(move || {
        let listener = UnixListener::bind(&socket_path_for_listener).expect("bind unix socket");

        let handle = thread::spawn(move || {
            if let Err(error) = accept_unix_connection(listener, default_daemon_lines()) {
                panic!("unix listener failed: {error:?}");
            }
        });

        (SocketEndpoint::unix(socket_display), handle)
    });
}

#[rstest]
#[case(io::ErrorKind::ConnectionRefused, true, "connection refused")]
#[case(io::ErrorKind::NotFound, true, "socket not found")]
#[case(io::ErrorKind::AddrNotAvailable, true, "address unavailable")]
#[case(io::ErrorKind::PermissionDenied, false, "permission denied")]
#[case(io::ErrorKind::TimedOut, false, "timed out")]
fn is_daemon_not_running_classifies_errors(
    #[case] kind: io::ErrorKind,
    #[case] expected: bool,
    #[case] _description: &str,
) {
    let error = AppError::Connect {
        endpoint: String::from("test:1234"),
        source: io::Error::new(kind, "test error"),
    };
    assert_eq!(is_daemon_not_running(&error), expected);
}

#[test]
fn is_daemon_not_running_rejects_non_connect_errors() {
    let error = AppError::MissingDomain;
    assert!(!is_daemon_not_running(&error));

    let error = AppError::MissingOperation;
    assert!(!is_daemon_not_running(&error));

    let error = AppError::MissingExit;
    assert!(!is_daemon_not_running(&error));

    let error = AppError::SerialiseRequest(serde_json::from_str::<()>("bad").unwrap_err());
    assert!(!is_daemon_not_running(&error));
}

/// Tests for execute_daemon_command auto-start decision logic.
mod auto_start_decision {
    use super::*;
    use crate::lifecycle::LifecycleContext;
    use crate::{CommandInvocation, IoStreams, execute_daemon_command};
    use std::ffi::OsStr;
    use weaver_config::SocketEndpoint;

    fn make_invocation() -> CommandInvocation {
        CommandInvocation {
            domain: String::from("observe"),
            operation: String::from("test"),
            arguments: Vec::new(),
        }
    }

    // Socket that refuses connections (nothing listening).
    #[rstest]
    #[case("Waiting for daemon start...", "auto-start triggered")]
    #[case("failed to spawn", "spawn failure reported")]
    fn auto_start_behavior(#[case] expected_substring: &str, #[case] _description: &str) {
        let config = Config {
            daemon_socket: SocketEndpoint::tcp("127.0.0.1", 1),
            ..Config::default()
        };
        let context = LifecycleContext {
            config: &config,
            config_arguments: &[],
            daemon_binary: Some(OsStr::new("/nonexistent/weaverd")),
        };
        let invocation = make_invocation();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut io = IoStreams::new(&mut stdout, &mut stderr);

        let exit = execute_daemon_command(invocation, context, &mut io);

        assert_eq!(exit, std::process::ExitCode::FAILURE);
        let stderr_text = decode_utf8(stderr, "stderr").expect("stderr utf8");
        assert!(
            stderr_text.contains(expected_substring),
            "expected stderr to contain {expected_substring:?}, got: {stderr_text:?}"
        );
    }
}
