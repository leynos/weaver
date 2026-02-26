//! CLI argument definitions for the Weaver toolchain.
//!
//! This module defines the command-line interface structure used by
//! both the runtime parser and the build script for manpage generation.

use clap::{Parser, Subcommand, ValueEnum};

/// Output format selection for domain command responses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Selects `human` for terminal output and `json` for redirected output.
    #[default]
    Auto,
    /// Always render human-readable output.
    Human,
    /// Always emit raw JSON payloads from the daemon.
    Json,
}

/// Command-line interface for the Weaver semantic code tool.
#[derive(Parser, Debug)]
#[command(
    name = "weaver",
    disable_help_subcommand = true,
    subcommand_negates_reqs = true
)]
pub(crate) struct Cli {
    /// Prints the negotiated capability matrix and exits.
    #[arg(long)]
    pub(crate) capabilities: bool,
    /// Controls how daemon output is rendered.
    #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
    pub(crate) output: OutputFormat,
    /// Structured subcommands (for example `daemon start`).
    #[command(subcommand)]
    pub(crate) command: Option<CliCommand>,
    /// The command domain (for example `observe`).
    #[arg(value_name = "DOMAIN")]
    pub(crate) domain: Option<String>,
    /// The command operation (for example `get-definition`).
    #[arg(value_name = "OPERATION")]
    pub(crate) operation: Option<String>,
    /// Additional arguments passed to the daemon.
    #[arg(
        value_name = "ARG",
        num_args = 0..,
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    pub(crate) arguments: Vec<String>,
}

impl Cli {
    /// Returns true when no domain, subcommand, or probe flag was supplied.
    ///
    /// This detects the case where the operator invoked `weaver` with no
    /// meaningful arguments, so the runner can emit short help guidance
    /// before attempting configuration loading or daemon contact.
    #[allow(
        dead_code,
        reason = "used by lib.rs but not by build.rs which #[path]-includes cli.rs"
    )]
    pub(crate) fn is_bare_invocation(&self) -> bool {
        self.domain.is_none() && self.command.is_none() && !self.capabilities
    }
}

/// Structured subcommands for the Weaver CLI.
#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CliCommand {
    /// Runs daemon lifecycle commands.
    Daemon {
        /// The lifecycle action to perform.
        #[command(subcommand)]
        action: DaemonAction,
    },
}

/// Daemon lifecycle actions.
#[derive(Subcommand, Debug, Clone, Copy)]
pub(crate) enum DaemonAction {
    /// Starts the daemon and waits for readiness.
    Start,
    /// Stops the daemon gracefully.
    Stop,
    /// Prints daemon health information.
    Status,
}
