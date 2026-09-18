//! Shared clap help rendering for runtime help output and manpage generation.
//!
//! The runtime parser intentionally keeps configuration flags outside clap so
//! they only take effect before the command domain. This module augments the
//! help-only clap command with those flags so help and generated manpages stay
//! truthful without weakening runtime parsing semantics.

use std::{ffi::OsString, io, io::Write};

use clap::{Arg, ArgAction, Command, CommandFactory};
use ortho_config::{
    FluentLocalizerError,
    docs::{FieldMetadata, OrthoConfigDocs},
};
use thiserror::Error;
use weaver_config::{Config, config_field_help};

use crate::{cli::Cli, command_ir, command_tree};

#[path = "help_metadata.rs"]
mod metadata;

const CONFIG_PATH_ARG_ID: &str = "config-path";
const CONFIG_HELP_HEADING: &str = "Options";
const ORDERING_CAVEAT: &str = "Config flags must appear before the command domain or structured \
                               subcommand to take effect; for example, `weaver daemon start \
                               --log-filter debug` is ignored because `--log-filter` appears \
                               after `start`.";

struct ConfigFieldArgMetadata {
    name: String,
    long: String,
    short: Option<char>,
    help: &'static str,
    takes_value: bool,
    multiple: bool,
    value_name: Option<String>,
}

/// Error raised while constructing the shared runtime and manpage help command.
#[derive(Debug, Error)]
pub(crate) enum HelpConstructionError {
    /// The canonical command tree could not be represented in the docs IR.
    #[error("failed to project the command tree into documentation metadata: {source}")]
    ProjectCommandMetadata {
        /// Projection failure that prevents complete help metadata.
        #[source]
        source: command_ir::ProjectionError,
    },
    /// The embedded Fluent catalogue could not localise help metadata.
    #[error("failed to load the embedded help localisation catalogue: {source}")]
    LoadLocalisationCatalogue {
        /// Fluent resource failure that prevents localised help output.
        #[source]
        source: FluentLocalizerError,
    },
}

impl HelpConstructionError {
    /// Adds the help-construction context to a command metadata projection failure.
    fn project_command_metadata(source: command_ir::ProjectionError) -> Self {
        Self::ProjectCommandMetadata { source }
    }

    /// Adds the help-construction context to a Fluent catalogue loading failure.
    pub(super) fn load_localisation_catalogue(source: FluentLocalizerError) -> Self {
        Self::LoadLocalisationCatalogue { source }
    }
}

/// Error raised while rendering an otherwise complete help command.
#[derive(Debug, Error)]
pub(crate) enum HelpWriteError {
    /// Complete help metadata could not be constructed.
    #[error(transparent)]
    Construction(#[from] HelpConstructionError),
    /// The requested output stream rejected rendered help.
    #[error("failed to write rendered help: {0}")]
    Write(#[from] io::Error),
}

/// Returns an augmented `clap::Command` for runtime help and manpage generation.
///
/// The command is returned only when the full recursive metadata projection and
/// the embedded localisation catalogue both succeed.
pub(crate) fn try_command() -> Result<Command, HelpConstructionError> {
    build_command(command_tree::root(), [metadata::EN_US_MESSAGES])
}

/// Writes help for the provided arguments using the augmented help command.
pub fn write_help_for_args<W: Write>(
    args: &[OsString],
    writer: &mut W,
) -> Result<(), HelpWriteError> {
    let mut cmd = try_command()?;
    match cmd.try_get_matches_from_mut(args.iter().cloned()) {
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            write!(writer, "{error}").map_err(HelpWriteError::Write)
        }
        Err(error) => write!(writer, "{error}").map_err(HelpWriteError::Write),
        Ok(_) => {
            cmd.write_long_help(writer)
                .inspect_err(|e| {
                    tracing::warn!(error = %e, "failed to write long help to writer");
                })
                .map_err(HelpWriteError::Write)?;
            writeln!(writer).map_err(HelpWriteError::Write)
        }
    }
}

fn build_command(
    root: &command_tree::CommandNode,
    localisation_resources: impl IntoIterator<Item = &'static str>,
) -> Result<Command, HelpConstructionError> {
    tracing::debug!("building augmented help command");
    let mut command = Cli::command();
    let projected =
        command_ir::project(root).map_err(HelpConstructionError::project_command_metadata)?;
    command = metadata::apply(command, &projected, root, localisation_resources)?;
    command = command.arg(config_path_arg());

    for field in Config::get_doc_metadata().fields {
        if let Some(arg) = config_field_arg(&field) {
            tracing::trace!(field = %field.name, "attached config arg to help command");
            command = command.arg(arg);
        }
    }

    Ok(attach_ordering_caveat(command))
}

/// Returns the `--config-path` clap argument.
fn config_path_arg() -> Arg {
    Arg::new(CONFIG_PATH_ARG_ID)
        .long(CONFIG_PATH_ARG_ID)
        .value_name("PATH")
        .help("Path to the configuration file supplied by --config-path")
        .help_heading(CONFIG_HELP_HEADING)
        .global(true)
        .action(ArgAction::Set)
}

/// Maps a [`FieldMetadata`] entry to an optional `clap::Arg`, returning `None`
/// for fields marked `hide_in_help` or lacking a long flag name.
fn config_field_arg(field: &FieldMetadata) -> Option<Arg> {
    let cli = field.cli.as_ref()?;
    let long = cli.long.as_deref()?;
    if cli.hide_in_help {
        return None;
    }

    let metadata = ConfigFieldArgMetadata {
        name: field.name.clone(),
        long: long.to_owned(),
        short: cli.short,
        help: config_field_help(&field.help_id),
        takes_value: cli.takes_value,
        multiple: cli.multiple,
        value_name: cli.value_name.clone(),
    };

    Some(config_arg_from_metadata(metadata))
}

/// Maps shared configuration metadata to a `clap::Arg`.
fn config_arg_from_metadata(field: ConfigFieldArgMetadata) -> Arg {
    let ConfigFieldArgMetadata {
        name,
        long,
        short,
        help,
        takes_value,
        multiple,
        value_name,
    } = field;
    let mut arg = Arg::new(name)
        .long(long)
        .help(help)
        .help_heading(CONFIG_HELP_HEADING)
        .global(true);

    if let Some(short) = short {
        arg = arg.short(short);
    }

    if takes_value {
        arg = arg.action(if multiple {
            ArgAction::Append
        } else {
            ArgAction::Set
        });
        if let Some(value_name) = value_name {
            arg = arg.value_name(value_name);
        }
    } else {
        arg = arg.action(ArgAction::SetTrue);
    }

    arg
}

/// Recursively appends the configuration-ordering caveat to help output.
fn attach_ordering_caveat(command: Command) -> Command {
    let command = command.mut_subcommands(attach_ordering_caveat);
    let after_help = command.get_after_help().map_or_else(
        || ORDERING_CAVEAT.to_string(),
        |existing| format!("{existing}\n\n{ORDERING_CAVEAT}"),
    );
    command.after_help(after_help)
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;

#[cfg(test)]
mod prop_tests {
    //! Property tests for augmented help command construction.

    use proptest::prelude::*;

    use super::try_command;

    proptest! {
        #[test]
        fn command_always_includes_config_path(_seed in 0u64..) {
            let cmd = try_command().expect("built-in help command should construct");

            prop_assert!(
                cmd.get_arguments().any(|a| a.get_id().as_str() == "config-path"),
                "--config-path must always be present in the augmented command"
            );
        }
    }
}
