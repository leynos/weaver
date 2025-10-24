use super::support::{default_daemon_lines, read_fixture};

use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::ExitCode;
use std::thread;

use anyhow::{Context, Result, anyhow};

use crate::{
    AppError, Cli, CommandDescriptor, CommandInvocation, CommandRequest, ConfigLoader,
    EMPTY_LINE_LIMIT, connect, exit_code_from_status, read_daemon_messages, run_with_loader,
};
use rstest::rstest;
use weaver_config::{Config, SocketEndpoint};

#[test]
fn serialises_command_request_matches_golden() -> Result<()> {
    let invocation = CommandInvocation {
        domain: String::from("observe"),
        operation: String::from("get-definition"),
        arguments: vec![String::from("--symbol"), String::from("main")],
    };
    let request = CommandRequest::from(invocation);
    let mut buffer: Vec<u8> = Vec::new();
    request
        .write_jsonl(&mut buffer)
        .context("serialises request")?;
    let actual = String::from_utf8(buffer).context("request utf8")?;
    let expected = read_fixture("request_observe_get_definition.jsonl")?;
    assert_eq!(actual, expected);
    Ok(())
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
fn read_daemon_messages_errors_without_exit() -> Result<()> {
    let input = b"{\"kind\":\"stream\",\"stream\":\"stdout\",\"data\":\"hi\"}\n";
    let mut cursor = Cursor::new(&input[..]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = read_daemon_messages(&mut cursor, &mut stdout, &mut stderr).unwrap_err();
    assert!(matches!(error, AppError::MissingExit));
    assert_eq!(String::from_utf8(stdout).context("stdout utf8")?, "hi");
    Ok(())
}

#[test]
fn read_daemon_messages_warns_after_empty_lines() -> Result<()> {
    let mut payload = Vec::new();
    for _ in 0..EMPTY_LINE_LIMIT {
        payload.extend_from_slice(b"\n");
    }
    let mut cursor = Cursor::new(payload);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = read_daemon_messages(&mut cursor, &mut stdout, &mut stderr).unwrap_err();
    assert!(matches!(error, AppError::MissingExit));
    let warning = String::from_utf8(stderr).context("stderr utf8")?;
    assert!(warning.contains("Warning: received"));
    Ok(())
}

#[test]
fn read_daemon_messages_fails_on_malformed_json() -> Result<()> {
    let mut cursor = Cursor::new(Vec::from("this is not json\n"));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = read_daemon_messages(&mut cursor, &mut stdout, &mut stderr).unwrap_err();
    assert!(matches!(error, AppError::ParseMessage(_)));
    Ok(())
}

#[test]
fn run_with_loader_reports_configuration_failures() -> Result<()> {
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
            .context("stderr utf8")?
            .contains("command domain")
    );
    Ok(())
}

#[test]
fn run_with_loader_filters_configuration_arguments() -> Result<()> {
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
    Ok(())
}

/// Exercises a full daemon connection cycle: connect, write JSONL request,
/// read daemon messages, and verify exit status. The caller provides a setup
/// closure that spawns a listener thread and returns the endpoint.
fn test_daemon_connection<F>(setup_listener: F) -> Result<()>
where
    F: FnOnce() -> Result<(SocketEndpoint, thread::JoinHandle<()>)>,
{
    let (endpoint, handle) = setup_listener()?;

    let mut connection = connect(&endpoint).context("connect to daemon")?;
    let request = CommandRequest {
        command: CommandDescriptor {
            domain: "observe".into(),
            operation: "noop".into(),
        },
        arguments: Vec::new(),
    };
    request
        .write_jsonl(&mut connection)
        .context("write request")?;

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = read_daemon_messages(&mut connection, &mut stdout, &mut stderr)
        .context("read responses")?;
    assert_eq!(status, 17);

    handle
        .join()
        .map_err(|_| anyhow!("listener thread panicked"))?;
    Ok(())
}

#[test]
fn connect_successfully_establishes_tcp_connection() -> Result<()> {
    test_daemon_connection(|| {
        let listener = TcpListener::bind(("127.0.0.1", 0)).context("bind listener")?;
        let port = listener.local_addr().context("listener local addr")?.port();

        let handle = thread::spawn(move || {
            if let Err(error) = accept_tcp_connection(listener, default_daemon_lines()) {
                panic!("tcp listener failed: {error:?}");
            }
        });

        Ok((SocketEndpoint::tcp("127.0.0.1", port), handle))
    })?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn connect_supports_unix_sockets() -> Result<()> {
    use std::os::unix::net::UnixListener;

    let dir = tempfile::tempdir().context("tempdir")?;
    let socket_path = dir.path().join("daemon.sock");
    let socket_path_for_listener = socket_path.clone();
    let socket_display = socket_path
        .to_str()
        .context("socket path is utf8")?
        .to_owned();

    test_daemon_connection(move || {
        let listener = UnixListener::bind(&socket_path_for_listener).context("bind unix")?;

        let handle = thread::spawn(move || {
            if let Err(error) = accept_unix_connection(listener, default_daemon_lines()) {
                panic!("unix listener failed: {error:?}");
            }
        });

        Ok((SocketEndpoint::unix(socket_display), handle))
    })?;
    Ok(())
}

fn accept_tcp_connection(listener: TcpListener, lines: Vec<String>) -> Result<()> {
    let (stream, _) = listener.accept().context("accept tcp connection")?;
    respond_to_request(stream, &lines)
}

#[cfg(unix)]
fn accept_unix_connection(
    listener: std::os::unix::net::UnixListener,
    lines: Vec<String>,
) -> Result<()> {
    let (stream, _) = listener.accept().context("accept unix connection")?;
    respond_to_request(stream, &lines)
}

fn respond_to_request<T>(mut stream: T, lines: &[String]) -> Result<()>
where
    T: TryCloneStream,
{
    let mut buffer = String::new();
    {
        let clone = stream.try_clone().context("clone stream")?;
        let mut reader = BufReader::new(clone);
        let _ = reader.read_line(&mut buffer).context("read request")?;
    }
    write_lines(&mut stream, lines).context("write response lines")
}

fn write_lines(stream: &mut impl Write, lines: &[String]) -> io::Result<()> {
    for line in lines {
        stream.write_all(line.as_bytes())?;
        stream.write_all(b"\n")?;
    }
    stream.flush()
}

trait TryCloneStream: Write {
    type Owned: Read + Write + Send + 'static;

    fn try_clone(&self) -> io::Result<Self::Owned>;
}

impl TryCloneStream for TcpStream {
    type Owned = TcpStream;

    fn try_clone(&self) -> io::Result<Self::Owned> {
        TcpStream::try_clone(self)
    }
}

#[cfg(unix)]
impl TryCloneStream for std::os::unix::net::UnixStream {
    type Owned = std::os::unix::net::UnixStream;

    fn try_clone(&self) -> io::Result<Self::Owned> {
        std::os::unix::net::UnixStream::try_clone(self)
    }
}
