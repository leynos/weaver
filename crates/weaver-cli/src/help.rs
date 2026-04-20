//! Shared clap help rendering for runtime help output and manpage generation.
//!
//! The runtime parser intentionally keeps configuration flags outside clap so
//! they only take effect before the command domain. This module augments the
//! help-only clap command with those flags so help and generated manpages stay
//! truthful without weakening runtime parsing semantics.

use clap::{Arg, ArgAction, Command, CommandFactory};
use ortho_config::docs::{CliMetadata, FieldMetadata, OrthoConfigDocs};
use weaver_config::Config;

use crate::cli::Cli;

const CONFIG_PATH_ARG_ID: &str = "config-path";
const CONFIG_HELP_HEADING: &str = "Options";

/// Returns an augmented `clap::Command` that adds shared configuration flags
/// for help rendering and manpage generation without affecting the runtime
/// parser.
pub(crate) fn command() -> Command {
    let mut command = Cli::command();
    command = command.arg(config_path_arg());

    for field in Config::get_doc_metadata().fields {
        if let Some(arg) = config_field_arg(&field) {
            command = command.arg(arg);
        }
    }

    command
}

/// Returns the `--config-path` clap argument.
fn config_path_arg() -> Arg {
    Arg::new(CONFIG_PATH_ARG_ID)
        .long(CONFIG_PATH_ARG_ID)
        .value_name("PATH")
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

    // SAFETY: clap requires `'static` lifetimes for dynamically constructed
    // argument IDs. The augmented help command is built for the current
    // process, and the leaked allocation intentionally lives for the process
    // lifetime.
    let field_id: &'static str = Box::leak(field.name.clone().into_boxed_str());
    // SAFETY: clap requires `'static` lifetimes for dynamically constructed
    // long flag names. The augmented help command is built for the current
    // process, and the leaked allocation intentionally lives for the process
    // lifetime.
    let long_flag: &'static str = Box::leak(long.to_string().into_boxed_str());
    let mut arg = Arg::new(field_id)
        .long(long_flag)
        .help_heading(CONFIG_HELP_HEADING)
        .global(true);

    arg = apply_arg_shape(arg, cli);

    Some(arg)
}

/// Configures value or flag behaviour, optional short alias, `value_name`, and
/// allowed values on an [`Arg`] from [`CliMetadata`].
fn apply_arg_shape(arg: Arg, cli: &CliMetadata) -> Arg {
    let mut shaped = arg;

    if let Some(short) = cli.short {
        shaped = shaped.short(short);
    }

    if cli.takes_value {
        shaped = shaped.action(if cli.multiple {
            ArgAction::Append
        } else {
            ArgAction::Set
        });
        if let Some(value_name) = cli.value_name.as_deref() {
            // SAFETY: clap requires `'static` lifetimes for dynamically
            // constructed value names. The augmented help command is built for
            // the current process, and the leaked allocation intentionally
            // lives for the process lifetime.
            let leaked_value_name: &'static str =
                Box::leak(value_name.to_string().into_boxed_str());
            shaped = shaped.value_name(leaked_value_name);
        }
        if !cli.possible_values.is_empty() {
            let possible_values = cli
                .possible_values
                .iter()
                // SAFETY: clap requires `'static` lifetimes for dynamically
                // constructed possible values. The augmented help command is
                // built for the current process, and the leaked allocations
                // intentionally live for the process lifetime.
                .map(|value| Box::leak(value.clone().into_boxed_str()) as &'static str)
                .collect::<Vec<_>>();
            shaped = shaped.value_parser(possible_values);
        }
    } else {
        shaped = shaped.action(ArgAction::SetTrue);
    }

    shaped
}
