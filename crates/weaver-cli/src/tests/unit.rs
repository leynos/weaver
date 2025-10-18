use super::support::{default_daemon_lines, read_fixture};

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{self, BufRead, Cursor, Write};
use std::net::TcpListener;
use std::process::ExitCode;
use std::thread;

use crate::{
    AppError, Cli, CommandDescriptor, CommandInvocation, CommandRequest, ConfigLoader,
    EMPTY_LINE_LIMIT, connect, exit_code_from_status, read_daemon_messages, run_with_loader,
};
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
    let actual = String::from_utf8(buffer).expect("request utf8");
    let expected = read_fixture("request_observe_get_definition.jsonl");
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
    assert_eq!(String::from_utf8(stdout).unwrap(), "hi");
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
    let warning = String::from_utf8(stderr).unwrap();
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
    let exit = run_with_loader(
        vec![OsString::from("weaver")],
        &mut stdout,
        &mut stderr,
        &FailingLoader,
    );
    assert_eq!(exit, ExitCode::FAILURE);
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("command domain")
    );
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
        &mut stdout,
        &mut stderr,
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

    let mut connection = connect(&endpoint).expect("connect to daemon");
    let request = CommandRequest {
        command: CommandDescriptor {
            domain: "observe".into(),
            operation: "noop".into(),
        },
        arguments: Vec::new(),
    };
    request.write_jsonl(&mut connection).expect("write request");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status =
        read_daemon_messages(&mut connection, &mut stdout, &mut stderr).expect("read responses");
    assert_eq!(status, 17);

    handle.join().expect("join listener thread");
}

#[test]
fn connect_successfully_establishes_tcp_connection() {
    test_daemon_connection(|| {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let port = listener.local_addr().unwrap().port();

        let handle = thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let mut buffer = String::new();
                let mut reader = io::BufReader::new(stream.try_clone().unwrap());
                let _ = reader.read_line(&mut buffer);
                let mut writer = stream;
                for line in default_daemon_lines() {
                    writer.write_all(line.as_bytes()).unwrap();
                    writer.write_all(b"\n").unwrap();
                    writer.flush().unwrap();
                }
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
        let listener = UnixListener::bind(&socket_path_for_listener).expect("bind unix");

        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = String::new();
                let mut reader = io::BufReader::new(stream.try_clone().unwrap());
                let _ = reader.read_line(&mut buffer);
                for line in default_daemon_lines() {
                    stream.write_all(line.as_bytes()).unwrap();
                    stream.write_all(b"\n").unwrap();
                    stream.flush().unwrap();
                }
            }
        });

        (SocketEndpoint::unix(socket_display), handle)
    });
}
