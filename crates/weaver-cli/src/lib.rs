//! Command-line interface runtime for the Weaver toolchain.
//!
//! The module owns argument parsing, configuration bootstrapping, request
//! serialisation, and daemon transport negotiation. The interface is designed
//! to be exercised both from the binary entrypoint and from tests where
//! configuration loading and IO streams can be substituted.

use std::{
    ffi::{OsStr, OsString},
    io::{Read, Write},
    process::ExitCode,
};

use clap::Parser;
use ortho_config::Localizer;

mod actionable_guidance;
mod cli;
mod command;
mod config;
mod daemon_output;
mod discoverability;
mod errors;
mod help;
mod lifecycle;
mod localizer;
pub mod output;
mod preflight;
mod runner_glue;
mod runtime_utils;
mod transport;
/// Shared configuration flag renderings expected in clap help output.
///
/// MAINTENANCE: This list must be kept in sync with the `cli_long` attributes
/// defined on `weaver_config::Config`. When adding new configuration options,
/// update this array and `CONFIG_CLI_FLAGS` accordingly.
pub const SHARED_CONFIG_HELP_FLAGS: &[&str] = &[
    "--config-path <PATH>",
    "--daemon-socket <ENDPOINT>",
    "--log-filter <FILTER>",
    "--log-format <FORMAT>",
    "--capability-overrides <DIRECTIVE>",
    "--locale <LOCALE>",
];

pub use cli::OutputFormat;
pub(crate) use cli::{Cli, CliCommand, DaemonAction};
#[cfg(test)]
pub(crate) use command::CommandDescriptor;
pub(crate) use command::{CommandInvocation, CommandRequest};
use config::prepare_cli_arguments;
pub(crate) use config::{ConfigLoader, OrthoConfigLoader, split_config_arguments};
#[cfg(test)]
pub(crate) use daemon_output::{OutputSettings, read_daemon_messages};
pub use discoverability::DOMAIN_OPERATIONS;
pub(crate) use errors::{AppError, is_daemon_not_running};
use lifecycle::{
    LifecycleContext,
    LifecycleError,
    LifecycleInvocation,
    LifecycleOutput,
    SystemLifecycle,
};
use localizer::build_localizer;
pub use output::{OutputContext, ResolvedOutputFormat, render_human_output};
pub(crate) use preflight::handle_preflight;
#[cfg(test)]
pub(crate) use runner_glue::build_request;
pub(crate) use runner_glue::execute_daemon_command;
pub(crate) use runtime_utils::{exit_code_from_status, handle_capabilities_mode};
#[cfg(test)]
pub(crate) use transport::connect;

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
    "--locale",
];
pub(crate) const EMPTY_LINE_LIMIT: usize = 10;
/// Bundles the IO streams provided to the CLI runtime.
///
/// `IoStreams` owns the long-lived writers used while parsing CLI arguments.
/// Lifecycle commands receive a short-lived output wrapper that borrows these
/// streams so helpers can flush individual messages without
/// threading the CLI runtime through every call.
pub struct IoStreams<'a, R: Read, W: Write, E: Write> {
    pub(crate) stdin: &'a mut R,
    pub(crate) stdout: &'a mut W,
    pub(crate) stderr: &'a mut E,
    stdout_is_terminal: bool,
}
impl<'a, R: Read, W: Write, E: Write> IoStreams<'a, R, W, E> {
    pub fn new(
        stdin: &'a mut R,
        stdout: &'a mut W,
        stderr: &'a mut E,
        stdout_is_terminal: bool,
    ) -> Self {
        Self {
            stdin,
            stdout,
            stderr,
            stdout_is_terminal,
        }
    }

    pub(crate) const fn stdout_is_terminal(&self) -> bool { self.stdout_is_terminal }
}
impl Cli {
    /// Returns true when no domain, subcommand, or probe flag was supplied,
    /// indicating the operator needs short help guidance.
    fn is_bare_invocation(&self) -> bool {
        self.domain.is_none() && self.command.is_none() && !self.capabilities
    }
}

struct CliRunner<'a, R: Read, W: Write, E: Write, L: ConfigLoader> {
    io: &'a mut IoStreams<'a, R, W, E>,
    loader: &'a L,
    daemon_binary: Option<&'a OsStr>,
}

impl<'a, R, W, E, L> CliRunner<'a, R, W, E, L>
where
    R: Read,
    W: Write,
    E: Write,
    L: ConfigLoader,
{
    fn new(io: &'a mut IoStreams<'a, R, W, E>, loader: &'a L) -> Self {
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

    /// Parses CLI arguments, rendering help to stdout and returning `Ok(None)`
    /// if a `DisplayHelp` error is produced. Returns `Ok(Some(cli))` on
    /// success or `Err` on a genuine parse error or help-write failure.
    fn parse_or_render_help(
        &mut self,
        args: &[OsString],
        split: &config::ConfigArgumentSplit,
    ) -> Result<Option<Cli>, AppError> {
        let cli_arguments = prepare_cli_arguments(args, split);
        match Cli::try_parse_from(cli_arguments) {
            Ok(cli) => Ok(Some(cli)),
            Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
                tracing::debug!("rendering clap help");
                help::write_help_for_args(args, &mut *self.io.stdout)
                    .map(|()| None)
                    .map_err(|io_error| {
                        tracing::warn!(
                            error_kind = ?io_error.kind(),
                            error = %io_error,
                            "failed to write clap help"
                        );
                        AppError::EmitHelp(io_error)
                    })
            }
            Err(error) => Err(AppError::CliUsage(error)),
        }
    }

    fn run<I>(&mut self, args: I) -> ExitCode
    where
        I: IntoIterator<Item = OsString>,
    {
        let localizer = build_localizer();
        let mut lifecycle = SystemLifecycle;
        self.run_with_handler(args, localizer.as_ref(), |invocation, context, output| {
            lifecycle.handle(invocation, context, output)
        })
    }

    fn run_with_handler<I, F>(
        &mut self,
        args: I,
        localizer: &dyn Localizer,
        mut handler: F,
    ) -> ExitCode
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

        let parsed_cli = match self.parse_or_render_help(&args, &split) {
            Ok(Some(cli)) => Ok(cli),
            Ok(None) => return ExitCode::SUCCESS,
            Err(e) => return self.map_result_to_exit_code(Err(e)),
        };

        let result = parsed_cli
            .and_then(|cli| {
                handle_preflight(&cli, &split, &mut *self.io.stderr, localizer)?;
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

        self.map_result_to_exit_code(result)
    }

    fn map_result_to_exit_code(&mut self, result: Result<ExitCode, AppError>) -> ExitCode {
        match result {
            Ok(exit_code) => exit_code,
            Err(AppError::BareInvocation) => ExitCode::FAILURE,
            Err(AppError::PreflightGuidance) => ExitCode::FAILURE,
            Err(AppError::CliUsage(ref clap_err)) if !clap_err.use_stderr() => {
                write!(self.io.stdout, "{clap_err}").ok();
                ExitCode::SUCCESS
            }
            Err(AppError::Lifecycle(ref lifecycle_err)) => {
                actionable_guidance::write_startup_guidance(&mut *self.io.stderr, lifecycle_err)
                    .ok();
                ExitCode::FAILURE
            }
            Err(error) => {
                writeln!(self.io.stderr, "{error}").ok();
                ExitCode::FAILURE
            }
        }
    }
}

/// Runs the CLI using the provided arguments and IO handles.
#[must_use]
pub fn run<'a, I, R, W, E>(args: I, io: &'a mut IoStreams<'a, R, W, E>) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    run_with_loader(args, io, &OrthoConfigLoader)
}
/// Runs the CLI with a custom configuration loader.
#[must_use]
pub(crate) fn run_with_loader<'a, I, R, W, E, L>(
    args: I,
    io: &'a mut IoStreams<'a, R, W, E>,
    loader: &'a L,
) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: Read,
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
pub(crate) fn run_with_daemon_binary<'a, I, R, W, E, L, F>(
    args: I,
    io: &'a mut IoStreams<'a, R, W, E>,
    loader: &'a L,
    daemon_binary: Option<&'a OsStr>,
    localizer: &dyn Localizer,
    handler: F,
) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
    R: Read,
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
        .run_with_handler(args, localizer, handler)
}

#[cfg(test)]
mod tests;
