//! Command-line interface runtime for the Weaver toolchain.
//!
//! The module owns argument parsing, configuration bootstrapping, request
//! serialisation, and daemon transport negotiation. The interface is designed
//! to be exercised both from the binary entrypoint and from tests where
//! configuration loading and IO streams can be substituted.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use weaver_config::{CapabilityMatrix, Config};

mod transport;

use transport::connect;
const EMPTY_LINE_LIMIT: usize = 10;
const CONFIG_CLI_FLAGS: &[&str] = &[
    "--config-path",
    "--daemon-socket",
    "--log-filter",
    "--log-format",
    "--capability-overrides",
];

/// Runs the CLI using the provided arguments and IO handles.
#[must_use]
pub fn run<I, W, E>(args: I, stdout: &mut W, stderr: &mut E) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
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
    let mut exit_status: Option<i32> = None;
    let mut consecutive_empty_lines = 0;

    while reader
        .read_line(&mut line)
        .map_err(AppError::ReadResponse)?
        != 0
    {
        if line.trim().is_empty() {
            consecutive_empty_lines += 1;
            if consecutive_empty_lines >= EMPTY_LINE_LIMIT {
                writeln!(stderr, "Warning: received {EMPTY_LINE_LIMIT} consecutive empty lines from daemon; aborting.")
                    .map_err(AppError::ForwardResponse)?;
                break;
            }
            line.clear();
            continue;
        }
        consecutive_empty_lines = 0;
        let message: DaemonMessage = serde_json::from_str(&line).map_err(AppError::ParseMessage)?;
        match message {
            DaemonMessage::Stream { stream, data } => {
                match stream {
                    StreamTarget::Stdout => stdout.write_all(data.as_bytes()),
                    StreamTarget::Stderr => stderr.write_all(data.as_bytes()),
                }
                .map_err(AppError::ForwardResponse)?;
            }
            DaemonMessage::Exit { status } => exit_status = Some(status),
        }
        line.clear();
    }

    stdout.flush().map_err(AppError::ForwardResponse)?;
    stderr.flush().map_err(AppError::ForwardResponse)?;

    exit_status.ok_or(AppError::MissingExit)
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

#[derive(Debug)]
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

/// Loads configuration for the CLI.
trait ConfigLoader {
    fn load(&self, args: &[OsString]) -> Result<Config, AppError>;
}

struct OrthoConfigLoader;

impl ConfigLoader for OrthoConfigLoader {
    fn load(&self, args: &[OsString]) -> Result<Config, AppError> {
        let mut filtered: Vec<OsString> = Vec::new();
        if let Some(first) = args.first() {
            filtered.push(first.clone());
        }

        let mut pending_values = 0usize;
        for argument in args.iter().skip(1) {
            if pending_values > 0 {
                filtered.push(argument.clone());
                pending_values -= 1;
                continue;
            }

            if argument == OsStr::new("--") {
                break;
            }

            let argument_text = argument.to_string_lossy();
            if argument_text.starts_with("--") {
                let mut flag_parts = argument_text.splitn(2, '=');
                let flag = flag_parts.next().unwrap();
                let has_inline_value = flag_parts.next().is_some();
                if CONFIG_CLI_FLAGS.contains(&flag) {
                    filtered.push(argument.clone());
                    if !has_inline_value {
                        pending_values = 1;
                    }
                    continue;
                }
            }

            break;
        }

        Config::load_from_iter(filtered).map_err(AppError::LoadConfiguration)
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
    #[error("failed to resolve daemon address {endpoint}: {source}")]
    Resolve { endpoint: String, source: io::Error },
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
    #[error("daemon closed the stream without sending an exit status")]
    MissingExit,
    #[error("failed to serialise capability matrix: {0}")]
    SerialiseCapabilities(serde_json::Error),
    #[error("failed to emit capabilities: {0}")]
    EmitCapabilities(io::Error),
}

#[cfg(test)]
mod tests;
