//! CLI argument definitions for the Weaver toolchain.
//!
//! This module defines the command-line interface structure used by
//! both the runtime parser and the build script for manpage generation.

use clap::{Parser, Subcommand};

use crate::output::OutputFormat;

/// Command-line interface for the Weaver semantic code tool.
#[derive(Parser, Debug)]
#[command(
    name = "weaver",
    disable_help_subcommand = true,
    subcommand_negates_reqs = true
)]
pub struct Cli {
    /// Prints the negotiated capability matrix and exits.
    #[arg(long)]
    pub capabilities: bool,
    /// Controls how daemon output is rendered.
    #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
    pub output: OutputFormat,
    /// Structured subcommands (for example `daemon start`).
    #[command(subcommand)]
    pub command: Option<CliCommand>,
    /// The command domain (for example `observe`).
    #[arg(value_name = "DOMAIN")]
    pub domain: Option<String>,
    /// The command operation (for example `get-definition`).
    #[arg(value_name = "OPERATION")]
    pub operation: Option<String>,
    /// Additional arguments passed to the daemon.
    #[arg(
        value_name = "ARG",
        num_args = 0..,
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    pub arguments: Vec<String>,
}

/// Structured subcommands for the Weaver CLI.
#[derive(Subcommand, Debug, Clone)]
pub enum CliCommand {
    /// Runs daemon lifecycle commands.
    Daemon {
        /// The lifecycle action to perform.
        #[command(subcommand)]
        action: DaemonAction,
    },
}

/// Daemon lifecycle actions.
#[derive(Subcommand, Debug, Clone, Copy)]
pub enum DaemonAction {
    /// Starts the daemon and waits for readiness.
    Start,
    /// Stops the daemon gracefully.
    Stop,
    /// Prints daemon health information.
    Status,
}
