use super::support::{default_daemon_lines, read_fixture};

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

#[test]
fn command_invocation_requires_domain() {
    let cli = Cli {
        capabilities: false,
        domain: None,
        operation: Some(String::from("op")),
        arguments: Vec::new(),
    };
    let error = CommandInvocation::try_from(cli).unwrap_err();
    assert!(matches!(error, AppError::MissingDomain));
}

#[test]
fn command_invocation_rejects_blank_domain() {
    let cli = Cli {
        capabilities: false,
        domain: Some(String::from("   ")),
        operation: Some(String::from("op")),
        arguments: Vec::new(),
    };
    let error = CommandInvocation::try_from(cli).unwrap_err();
    assert!(matches!(error, AppError::MissingDomain));
}

#[test]
fn command_invocation_requires_operation() {
    let cli = Cli {
        capabilities: false,
        domain: Some(String::from("observe")),
        operation: None,
        arguments: Vec::new(),
    };
    let error = CommandInvocation::try_from(cli).unwrap_err();
    assert!(matches!(error, AppError::MissingOperation));
}

#[test]
fn command_invocation_rejects_blank_operation() {
    let cli = Cli {
        capabilities: false,
        domain: Some(String::from("observe")),
        operation: Some(String::from("   ")),
        arguments: Vec::new(),
    };
    let error = CommandInvocation::try_from(cli).unwrap_err();
    assert!(matches!(error, AppError::MissingOperation));
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
fn connect_successfully_establishes_tcp_connection() {
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

    let mut connection = connect(&SocketEndpoint::tcp("127.0.0.1", port)).expect("connect tcp");
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
    let _ = handle.join();
}

#[cfg(unix)]
#[test]
fn connect_supports_unix_sockets() {
    use std::os::unix::net::UnixListener;

    let dir = tempfile::tempdir().expect("tempdir");
    let socket_path = dir.path().join("daemon.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind unix");

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

    let endpoint = SocketEndpoint::unix(socket_path.to_str().unwrap());
    let mut connection = connect(&endpoint).expect("connect unix");
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
    let _ = handle.join();
}
