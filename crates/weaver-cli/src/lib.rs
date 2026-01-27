//! Command-line interface runtime for the Weaver toolchain.
//!
//! The module owns argument parsing, configuration bootstrapping, request
//! serialisation, and daemon transport negotiation. The interface is designed
//! to be exercised both from the binary entrypoint and from tests where
//! configuration loading and IO streams can be substituted.

use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use serde::Deserialize;
use thiserror::Error;
use weaver_config::{CapabilityMatrix, Config};

mod command;
mod config;
mod lifecycle;
pub mod output;
mod transport;

#[cfg(test)]
pub(crate) use command::CommandDescriptor;
pub(crate) use command::{CommandInvocation, CommandRequest};
use config::{ConfigArgumentSplit, split_config_arguments};
pub(crate) use config::{ConfigLoader, OrthoConfigLoader};
use lifecycle::{
    LifecycleContext, LifecycleError, LifecycleInvocation, LifecycleOutput, SystemLifecycle,
    try_auto_start_daemon,
};
pub use output::{OutputContext, OutputFormat, ResolvedOutputFormat, render_human_output};
use transport::connect;
/// CLI flags recognised by the configuration loader.
///
/// MAINTENANCE: This list must be kept in sync with the configuration flags
/// defined in `weaver-config`. When adding new configuration options, update
/// this array accordingly.
const CONFIG_CLI_FLAGS: &[&str] = &[
    "--config-path",
    "--daemon-socket",
    "--log-filter",
    "--log-format",
    "--capability-overrides",
];
const EMPTY_LINE_LIMIT: usize = 10;

/// Bundles the IO streams provided to the CLI runtime.
///
/// `IoStreams` owns the long-lived writers used while parsing CLI arguments.
/// Lifecycle commands receive a short-lived [`LifecycleOutput`] wrapper that
/// borrows these streams so helpers can flush individual messages without
/// threading the CLI runtime through every call.
pub(crate) struct IoStreams<'a, W: Write, E: Write> {
    pub(crate) stdout: &'a mut W,
    pub(crate) stderr: &'a mut E,
    stdout_is_terminal: bool,
}

impl<'a, W: Write, E: Write> IoStreams<'a, W, E> {
    pub(crate) fn new(stdout: &'a mut W, stderr: &'a mut E) -> Self {
        Self {
            stdout,
            stderr,
            stdout_is_terminal: io::stdout().is_terminal(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_terminal_status(
        stdout: &'a mut W,
        stderr: &'a mut E,
        stdout_is_terminal: bool,
    ) -> Self {
        Self {
            stdout,
            stderr,
            stdout_is_terminal,
        }
    }

    pub(crate) const fn stdout_is_terminal(&self) -> bool {
        self.stdout_is_terminal
    }
}

struct CliRunner<'a, W: Write, E: Write, L: ConfigLoader> {
    io: &'a mut IoStreams<'a, W, E>,
    loader: &'a L,
    daemon_binary: Option<&'a OsStr>,
}

impl<'a, W, E, L> CliRunner<'a, W, E, L>
where
    W: Write,
    E: Write,
    L: ConfigLoader,
{
    fn new(io: &'a mut IoStreams<'a, W, E>, loader: &'a L) -> Self {
        Self {
            io,
            loader,
            daemon_binary: None,
        }
    }

    #[cfg(test)]
    fn with_daemon_binary(mut self, daemon_binary: Option<&'a OsStr>) -> Self {
        self.daemon_binary = daemon_binary;
        self
    }

    fn run<I>(&mut self, args: I) -> ExitCode
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut lifecycle = SystemLifecycle;
        self.run_with_handler(args, |invocation, context, output| {
            lifecycle.handle(invocation, context, output)
        })
    }

    fn run_with_handler<I, F>(&mut self, args: I, mut handler: F) -> ExitCode
    where
        I: IntoIterator<Item = OsString>,
        F: FnMut(
            LifecycleInvocation,
            LifecycleContext<'_>,
            &mut LifecycleOutput<&mut W, &mut E>,
        ) -> Result<ExitCode, LifecycleError>,
    {
        let args: Vec<OsString> = args.into_iter().collect();
        let split = split_config_arguments(&args);
        let cli_arguments = prepare_cli_arguments(&args, &split);

        let result = Cli::try_parse_from(cli_arguments)
            .map_err(AppError::CliUsage)
            .and_then(|cli| {
                self.loader
                    .load(&split.config_arguments)
                    .map(|config| (cli, config))
            })
            .and_then(|(cli, config)| {
                if let Some(exit_code) = handle_capabilities_mode(&cli, &config, self.io) {
                    return Ok(exit_code);
                }

                if let Some(CliCommand::Daemon { action }) = cli.command.as_ref() {
                    let invocation = LifecycleInvocation {
                        command: (*action).into(),
                        arguments: Vec::new(),
                    };
                    let context = LifecycleContext {
                        config: &config,
                        config_arguments: &split.config_arguments,
                        daemon_binary: self.daemon_binary,
                    };
                    let mut output =
                        LifecycleOutput::new(&mut *self.io.stdout, &mut *self.io.stderr);
                    return handler(invocation, context, &mut output).map_err(AppError::from);
                }

                let output_format = cli.output.resolve(self.io.stdout_is_terminal());
                let invocation = CommandInvocation::try_from(cli)?;
                let context = LifecycleContext {
                    config: &config,
                    config_arguments: &split.config_arguments,
                    daemon_binary: self.daemon_binary,
                };
                Ok(execute_daemon_command(
                    invocation,
                    context,
                    self.io,
                    output_format,
                ))
            });

        match result {
            Ok(exit_code) => exit_code,
            Err(error) => {
                let _ = writeln!(self.io.stderr, "{error}");
                ExitCode::FAILURE
            }
        }
    }
}

/// Runs the CLI using the provided arguments and IO handles.
#[must_use]
pub fn run<I, W, E>(args: I, stdout: &mut W, stderr: &mut E) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    W: Write,
    E: Write,
{
    let mut io = IoStreams::new(stdout, stderr);
    run_with_loader(args, &mut io, &OrthoConfigLoader)
}

fn prepare_cli_arguments(args: &[OsString], split: &ConfigArgumentSplit) -> Vec<OsString> {
    let mut cli_arguments: Vec<OsString> = Vec::new();
    if let Some(first) = args.first() {
        cli_arguments.push(first.clone());
    }
    if split.command_start < args.len() {
        cli_arguments.extend(args[split.command_start..].iter().cloned());
    }
    cli_arguments
}

fn handle_capabilities_mode<W, E>(
    cli: &Cli,
    config: &Config,
    io: &mut IoStreams<'_, W, E>,
) -> Option<ExitCode>
where
    W: Write,
    E: Write,
{
    if !cli.capabilities {
        return None;
    }

    match emit_capabilities(config, io.stdout) {
        Ok(()) => Some(ExitCode::SUCCESS),
        Err(error) => {
            let _ = writeln!(io.stderr, "{error}");
            Some(ExitCode::FAILURE)
        }
    }
}

fn execute_daemon_command<W, E>(
    invocation: CommandInvocation,
    context: LifecycleContext<'_>,
    io: &mut IoStreams<'_, W, E>,
    output_format: ResolvedOutputFormat,
) -> ExitCode
where
    W: Write,
    E: Write,
{
    let output_context = OutputContext::new(
        invocation.domain.clone(),
        invocation.operation.clone(),
        invocation.arguments.clone(),
    );
    let request = CommandRequest::from(invocation);
    let mut connection = match connect(context.config.daemon_socket()) {
        Ok(connection) => connection,
        Err(error) if is_daemon_not_running(&error) => {
            if let Err(start_error) = try_auto_start_daemon(context, &mut *io.stderr) {
                let _ = writeln!(io.stderr, "{start_error}");
                return ExitCode::FAILURE;
            }
            // Retry connection after daemon started successfully.
            match connect(context.config.daemon_socket()) {
                Ok(connection) => connection,
                Err(retry_error) => {
                    let _ = writeln!(io.stderr, "{retry_error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        Err(error) => {
            let _ = writeln!(io.stderr, "{error}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = request.write_jsonl(&mut connection) {
        let _ = writeln!(io.stderr, "{error}");
        return ExitCode::FAILURE;
    }

    match read_daemon_messages(
        &mut connection,
        io,
        OutputSettings {
            format: output_format,
            context: &output_context,
        },
    ) {
        Ok(status) => exit_code_from_status(status),
        Err(error) => {
            let _ = writeln!(io.stderr, "{error}");
            ExitCode::FAILURE
        }
    }
}

/// Runs the CLI with a custom configuration loader.
#[must_use]
pub(crate) fn run_with_loader<'a, I, W, E, L>(
    args: I,
    io: &'a mut IoStreams<'a, W, E>,
    loader: &'a L,
) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    W: Write,
    E: Write,
    L: ConfigLoader,
{
    CliRunner::new(io, loader).run(args)
}

#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "test-only function requires full parameter set for dependency injection"
)]
pub(crate) fn run_with_daemon_binary<'a, I, W, E, L, F>(
    args: I,
    io: &'a mut IoStreams<'a, W, E>,
    loader: &'a L,
    daemon_binary: Option<&'a OsStr>,
    handler: F,
) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    W: Write,
    E: Write,
    L: ConfigLoader,
    F: FnMut(
        LifecycleInvocation,
        LifecycleContext<'_>,
        &mut LifecycleOutput<&mut W, &mut E>,
    ) -> Result<ExitCode, LifecycleError>,
{
    CliRunner::new(io, loader)
        .with_daemon_binary(daemon_binary)
        .run_with_handler(args, handler)
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

/// Determines whether an error indicates the daemon is not running.
///
/// Returns true for connection-refused, socket-not-found, and address-unavailable
/// errors, which typically indicate the daemon process is not listening.
fn is_daemon_not_running(error: &AppError) -> bool {
    match error {
        AppError::Connect { source, .. } => matches!(
            source.kind(),
            io::ErrorKind::ConnectionRefused
                | io::ErrorKind::NotFound
                | io::ErrorKind::AddrNotAvailable
        ),
        _ => false,
    }
}

/// Settings for rendering daemon output.
pub(crate) struct OutputSettings<'a> {
    pub(crate) format: ResolvedOutputFormat,
    pub(crate) context: &'a OutputContext,
}

fn read_daemon_messages<R, W, E>(
    connection: &mut R,
    io: &mut IoStreams<'_, W, E>,
    settings: OutputSettings<'_>,
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
                writeln!(
                    io.stderr,
                    "Warning: received {EMPTY_LINE_LIMIT} consecutive empty lines from daemon; aborting."
                )
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
                let rendered = match settings.format {
                    ResolvedOutputFormat::Human => render_human_output(settings.context, &data),
                    ResolvedOutputFormat::Json => None,
                };
                let payload = rendered.as_deref().unwrap_or(&data);
                match stream {
                    StreamTarget::Stdout => io.stdout.write_all(payload.as_bytes()),
                    StreamTarget::Stderr => io.stderr.write_all(payload.as_bytes()),
                }
                .map_err(AppError::ForwardResponse)?;
            }
            DaemonMessage::Exit { status } => exit_status = Some(status),
        }
        line.clear();
    }

    io.stdout.flush().map_err(AppError::ForwardResponse)?;
    io.stderr.flush().map_err(AppError::ForwardResponse)?;

    exit_status.ok_or(AppError::MissingExit)
}

#[derive(Parser, Debug)]
#[command(
    name = "weaver",
    disable_help_subcommand = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    /// Prints the negotiated capability matrix and exits.
    #[arg(long)]
    capabilities: bool,
    /// Controls how daemon output is rendered.
    #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
    output: OutputFormat,
    /// Structured subcommands (for example `daemon start`).
    #[command(subcommand)]
    command: Option<CliCommand>,
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

#[derive(Subcommand, Debug, Clone)]
enum CliCommand {
    /// Runs daemon lifecycle commands.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Subcommand, Debug, Clone, Copy)]
enum DaemonAction {
    /// Starts the daemon and waits for readiness.
    Start,
    /// Stops the daemon gracefully.
    Stop,
    /// Prints daemon health information.
    Status,
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
    #[error("daemon lifecycle command failed: {0}")]
    Lifecycle(#[from] LifecycleError),
}

#[cfg(test)]
mod tests;
