use std::ffi::OsString;
use std::io::{self, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

/// Runs the CLI using the provided arguments and IO handles.
#[must_use]
pub fn run<I, R, W, E>(args: I, _stdin: &mut R, stdout: &mut W, stderr: &mut E) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: io::Read,
    W: Write,
    E: Write,
{
    run_with_loader(args, stdout, stderr, &OrthoConfigLoader)
}

/// Runs the CLI with a custom configuration loader.
#[must_use]
pub(crate) fn run_with_loader<I, W, E, L>(
    args: I,
    stdout: &mut W,
    stderr: &mut E,
    loader: &L,
) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    W: Write,
    E: Write,
    L: ConfigLoader,
{
    let args: Vec<OsString> = args.into_iter().collect();
    let cli = match Cli::try_parse_from(args.clone()) {
        Ok(cli) => cli,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return ExitCode::FAILURE;
        }
    };

    let config = match loader.load(&args) {
        Ok(config) => config,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return ExitCode::FAILURE;
        }
    };

    if cli.capabilities {
        return match emit_capabilities(&config, stdout) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                let _ = writeln!(stderr, "{error}");
                ExitCode::FAILURE
            }
        };
    }

    let invocation = match CommandInvocation::try_from(cli) {
        Ok(invocation) => invocation,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return ExitCode::FAILURE;
        }
    };

    let request = CommandRequest::from(invocation);
    let mut connection = match connect(config.daemon_socket()) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = request.write_jsonl(&mut connection) {
        let _ = writeln!(stderr, "{error}");
        return ExitCode::FAILURE;
    }

    match read_daemon_messages(&mut connection, stdout, stderr) {
        Ok(status) => exit_code_from_status(status),
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            ExitCode::FAILURE
        }
    }
}

fn emit_capabilities<W>(config: &Config, stdout: &mut W) -> Result<(), AppError>
where
    W: Write,
{
    let matrix: CapabilityMatrix = config.capability_matrix();
    serde_json::to_writer_pretty(&mut *stdout, &matrix).map_err(AppError::SerialiseCapabilities)?;
    stdout
        .write_all(b"\n")
        .map_err(AppError::EmitCapabilities)?;
    stdout.flush().map_err(AppError::EmitCapabilities)
}

fn exit_code_from_status(status: i32) -> ExitCode {
    if (0..=255).contains(&status) {
        ExitCode::from(status as u8)
    } else {
        ExitCode::FAILURE
    }
}

fn connect(endpoint: &SocketEndpoint) -> Result<Connection, AppError> {
    match endpoint {
        SocketEndpoint::Tcp { host, port } => TcpStream::connect((host.as_str(), *port))
            .map(Connection::Tcp)
            .map_err(|error| AppError::Connect {
                endpoint: endpoint.to_string(),
                source: error,
            }),
        SocketEndpoint::Unix { path } => {
            #[cfg(unix)]
            {
                UnixStream::connect(path.as_str())
                    .map(Connection::Unix)
                    .map_err(|error| AppError::Connect {
                        endpoint: endpoint.to_string(),
                        source: error,
                    })
            }

            #[cfg(not(unix))]
            {
                Err(AppError::UnsupportedUnixTransport(endpoint.to_string()))
            }
        }
    }
}

fn read_daemon_messages<R, W, E>(
    connection: &mut R,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<i32, AppError>
where
    R: io::Read,
    W: Write,
    E: Write,
{
    use std::io::BufRead;

    let mut reader = io::BufReader::new(connection);
    let mut line = String::new();
    let mut exit_status = 0;

    while reader
        .read_line(&mut line)
        .map_err(AppError::ReadResponse)?
        != 0
    {
        if line.trim().is_empty() {
            line.clear();
            continue;
        }
        let message: DaemonMessage = serde_json::from_str(&line).map_err(AppError::ParseMessage)?;
        match message {
            DaemonMessage::Stream { stream, data } => {
                match stream {
                    StreamTarget::Stdout => stdout.write_all(data.as_bytes()),
                    StreamTarget::Stderr => stderr.write_all(data.as_bytes()),
                }
                .map_err(AppError::ForwardResponse)?;
            }
            DaemonMessage::Exit { status } => exit_status = status,
        }
        line.clear();
    }

    stdout.flush().map_err(AppError::ForwardResponse)?;
    stderr.flush().map_err(AppError::ForwardResponse)?;
    Ok(exit_status)
}

#[derive(Parser, Debug)]
#[command(name = "weaver", disable_help_subcommand = true)]
struct Cli {
    /// Prints the negotiated capability matrix and exits.
    #[arg(long)]
    capabilities: bool,
    /// The command domain (for example `observe`).
    #[arg(value_name = "DOMAIN")]
    domain: Option<String>,
    /// The command operation (for example `get-definition`).
    #[arg(value_name = "OPERATION")]
    operation: Option<String>,
    /// Additional arguments passed to the daemon.
    #[arg(
        value_name = "ARG",
        num_args = 0..,
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    arguments: Vec<String>,
}

struct CommandInvocation {
    domain: String,
    operation: String,
    arguments: Vec<String>,
}

impl TryFrom<Cli> for CommandInvocation {
    type Error = AppError;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        let domain = cli.domain.ok_or(AppError::MissingDomain)?.trim().to_owned();
        let operation = cli
            .operation
            .ok_or(AppError::MissingOperation)?
            .trim()
            .to_owned();
        if domain.is_empty() {
            return Err(AppError::MissingDomain);
        }
        if operation.is_empty() {
            return Err(AppError::MissingOperation);
        }
        Ok(Self {
            domain,
            operation,
            arguments: cli.arguments,
        })
    }
}

#[derive(Debug, Serialize)]
struct CommandRequest {
    command: CommandDescriptor,
    arguments: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CommandDescriptor {
    domain: String,
    operation: String,
}

impl From<CommandInvocation> for CommandRequest {
    fn from(invocation: CommandInvocation) -> Self {
        Self {
            command: CommandDescriptor {
                domain: invocation.domain,
                operation: invocation.operation,
            },
            arguments: invocation.arguments,
        }
    }
}

impl CommandRequest {
    fn write_jsonl<W>(&self, writer: &mut W) -> Result<(), AppError>
    where
        W: Write,
    {
        serde_json::to_writer(&mut *writer, self).map_err(AppError::SerialiseRequest)?;
        writer.write_all(b"\n").map_err(AppError::SendRequest)?;
        writer.flush().map_err(AppError::SendRequest)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DaemonMessage {
    Stream { stream: StreamTarget, data: String },
    Exit { status: i32 },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StreamTarget {
    Stdout,
    Stderr,
}

enum Connection {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl io::Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buf),
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buf),
        }
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(buf),
            #[cfg(unix)]
            Self::Unix(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            #[cfg(unix)]
            Self::Unix(stream) => stream.flush(),
        }
    }
}

/// Loads configuration for the CLI.
trait ConfigLoader {
    fn load(&self, args: &[OsString]) -> Result<Config, AppError>;
}

struct OrthoConfigLoader;

impl ConfigLoader for OrthoConfigLoader {
    fn load(&self, args: &[OsString]) -> Result<Config, AppError> {
        Config::load_from_iter(args.iter().cloned()).map_err(AppError::LoadConfiguration)
    }
}

#[derive(Debug, Error)]
enum AppError {
    #[error("failed to load configuration: {0}")]
    LoadConfiguration(Arc<ortho_config::OrthoError>),
    #[error("the command domain must be provided")]
    MissingDomain,
    #[error("the command operation must be provided")]
    MissingOperation,
    #[error("failed to connect to daemon at {endpoint}: {source}")]
    Connect { endpoint: String, source: io::Error },
    #[cfg(not(unix))]
    #[error("platform does not support Unix sockets: {0}")]
    UnsupportedUnixTransport(String),
    #[error("failed to serialise command request: {0}")]
    SerialiseRequest(serde_json::Error),
    #[error("failed to send request to daemon: {0}")]
    SendRequest(io::Error),
    #[error("failed to read response from daemon: {0}")]
    ReadResponse(io::Error),
    #[error("failed to parse daemon message: {0}")]
    ParseMessage(serde_json::Error),
    #[error("failed to forward daemon output: {0}")]
    ForwardResponse(io::Error),
    #[error("failed to serialise capability matrix: {0}")]
    SerialiseCapabilities(serde_json::Error),
    #[error("failed to emit capabilities: {0}")]
    EmitCapabilities(io::Error),
}

#[cfg(test)]
mod tests;
