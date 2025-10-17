//! Command-line interface runtime for the Weaver toolchain.
//!
//! The module owns argument parsing, configuration bootstrapping, request
//! serialisation, and daemon transport negotiation. The interface is designed
//! to be exercised both from the binary entrypoint and from tests where
//! configuration loading and IO streams can be substituted.

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use serde::Deserialize;
use thiserror::Error;
use weaver_config::{CapabilityMatrix, Config};

mod command;
mod config;
mod transport;

#[cfg(test)]
pub(crate) use command::CommandDescriptor;
pub(crate) use command::{CommandInvocation, CommandRequest};
use config::{ConfigArgumentSplit, split_config_arguments};
pub(crate) use config::{ConfigLoader, OrthoConfigLoader};
use transport::connect;
const CONFIG_CLI_FLAGS: &[&str] = &[
    "--config-path",
    "--daemon-socket",
    "--log-filter",
    "--log-format",
    "--capability-overrides",
];
const EMPTY_LINE_LIMIT: usize = 10;

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
    let split = split_config_arguments(&args);
    let cli_arguments = build_cli_arguments(&args, &split);

    match run_flow(cli_arguments, &split, stdout, stderr, loader) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            ExitCode::FAILURE
        }
    }
}

fn build_cli_arguments(args: &[OsString], split: &ConfigArgumentSplit) -> Vec<OsString> {
    let mut cli_arguments: Vec<OsString> = Vec::new();
    if let Some(first) = args.first() {
        cli_arguments.push(first.clone());
    }
    if split.command_start < args.len() {
        cli_arguments.extend(args[split.command_start..].iter().cloned());
    }
    cli_arguments
}

fn run_flow<W, E, L>(
    cli_arguments: Vec<OsString>,
    split: &ConfigArgumentSplit,
    stdout: &mut W,
    stderr: &mut E,
    loader: &L,
) -> Result<ExitCode, AppError>
where
    W: Write,
    E: Write,
    L: ConfigLoader,
{
    let cli = Cli::try_parse_from(cli_arguments).map_err(AppError::CliUsage)?;
    let config = loader.load(&split.config_arguments)?;

    if cli.capabilities {
        emit_capabilities(&config, stdout)?;
        return Ok(ExitCode::SUCCESS);
    }

    let invocation = CommandInvocation::try_from(cli)?;
    let request = CommandRequest::from(invocation);
    let mut connection = connect(config.daemon_socket())?;
    request.write_jsonl(&mut connection)?;
    let status = read_daemon_messages(&mut connection, stdout, stderr)?;
    Ok(exit_code_from_status(status))
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

#[derive(Debug, Error)]
enum AppError {
    #[error("failed to load configuration: {0}")]
    LoadConfiguration(Arc<ortho_config::OrthoError>),
    #[error("{0}")]
    CliUsage(clap::Error),
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
