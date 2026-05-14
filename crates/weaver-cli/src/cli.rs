//! CLI argument definitions for the Weaver toolchain.
//!
//! This module defines the command-line interface structure used by
//! both the runtime parser and the build script for manpage generation.

use clap::{Args, Parser, Subcommand, ValueEnum};

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
    version,
    disable_help_subcommand = true,
    subcommand_negates_reqs = true,
    about = concat!(
        "Semantic code intelligence tool for observing, ",
        "acting on, and verifying code",
    ),
    long_about = concat!(
        "Semantic code intelligence tool for observing, ",
        "acting on, and verifying code.\n",
        "\n",
        "Quick start:\n",
        "\n",
        "  weaver definitions get \\\n",
        "    --uri file:///src/main.rs --position 10:5\n",
        "  weaver act apply-patch < changes.patch\n",
        "  weaver daemon status\n",
        "\n",
        "Configuration flags such as --config-path and --daemon-socket\n",
        "must appear before the command domain.",
    ),
    after_help = concat!(
        "Domains and operations:\n",
        "\n",
        "  observe \u{2014} Query code structure and relationships\n",
        "    get-definition    find-references    grep\n",
        "    diagnostics       call-hierarchy     get-card\n",
        "    graph-slice\n",
        "\n",
        "  act \u{2014} Perform code modifications\n",
        "    rename-symbol     apply-edits        apply-patch\n",
        "    apply-rewrite     refactor\n",
        "\n",
        "  verify \u{2014} Validate code correctness\n",
        "    diagnostics       syntax",
    )
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

/// Structured subcommands for the Weaver CLI.
#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CliCommand {
    /// Query symbol definitions.
    Definitions {
        /// The definition operation to perform.
        #[command(subcommand)]
        action: DefinitionsAction,
    },
    /// Runs daemon lifecycle commands.
    Daemon {
        /// The lifecycle action to perform.
        #[command(subcommand)]
        action: DaemonAction,
    },
}

/// Resource-first definition commands.
#[derive(Subcommand, Debug, Clone)]
pub(crate) enum DefinitionsAction {
    /// Returns the definition location for a source position.
    Get(DefinitionGetArgs),
}

/// Arguments for `weaver definitions get`.
#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefinitionGetArgs {
    /// The document URI containing the reference position.
    #[arg(long)]
    pub(crate) uri: String,
    /// The 1-indexed line:column position to resolve.
    #[arg(long)]
    pub(crate) position: String,
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
