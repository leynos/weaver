//! Shared clap help rendering for runtime help output and manpage generation.
//!
//! The runtime parser intentionally keeps configuration flags outside clap so
//! they only take effect before the command domain. This module augments the
//! help-only clap command with those flags so help and generated manpages stay
//! truthful without weakening runtime parsing semantics.

use clap::{Arg, ArgAction, Command, CommandFactory};
use ortho_config::docs::{FieldMetadata, OrthoConfigDocs};
use std::sync::OnceLock;
use weaver_config::Config;

use crate::cli::Cli;

const CONFIG_PATH_ARG_ID: &str = "config-path";
const CONFIG_HELP_HEADING: &str = "Options";

static CONFIG_FIELD_ARGS: OnceLock<Vec<ConfigFieldArgMetadata>> = OnceLock::new();

struct ConfigFieldArgMetadata {
    name: &'static str,
    long: &'static str,
    short: Option<char>,
    takes_value: bool,
    multiple: bool,
    value_name: Option<&'static str>,
    possible_values: Vec<&'static str>,
}

/// Returns an augmented `clap::Command` that adds shared configuration flags
/// for help rendering and manpage generation without affecting the runtime
/// parser.
pub(crate) fn command() -> Command {
    let mut command = Cli::command();
    command = command.arg(config_path_arg());

    for field in config_field_args() {
        command = command.arg(config_field_arg(field));
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

fn config_field_args() -> &'static [ConfigFieldArgMetadata] {
    CONFIG_FIELD_ARGS.get_or_init(|| {
        Config::get_doc_metadata()
            .fields
            .iter()
            .filter_map(config_field_arg_metadata)
            .collect()
    })
}

/// Maps a [`FieldMetadata`] entry to bounded static metadata, returning `None`
/// for fields marked `hide_in_help` or lacking a long flag name.
fn config_field_arg_metadata(field: &FieldMetadata) -> Option<ConfigFieldArgMetadata> {
    let cli = field.cli.as_ref()?;
    let long = cli.long.as_deref()?;
    if cli.hide_in_help {
        return None;
    }

    Some(ConfigFieldArgMetadata {
        name: promote_static(field.name.clone()),
        long: promote_static(long.to_string()),
        short: cli.short,
        takes_value: cli.takes_value,
        multiple: cli.multiple,
        value_name: cli.value_name.clone().map(promote_static),
        possible_values: cli
            .possible_values
            .iter()
            .cloned()
            .map(promote_static)
            .collect(),
    })
}

fn promote_static(value: String) -> &'static str {
    // Clap requires process-lifetime metadata for dynamically built arguments.
    // Cache construction calls this once per field, keeping the promotion
    // bounded even when callers build the help command repeatedly.
    Box::leak(value.into_boxed_str())
}

/// Maps shared configuration metadata to a `clap::Arg`.
fn config_field_arg(field: &ConfigFieldArgMetadata) -> Arg {
    let mut arg = Arg::new(field.name)
        .long(field.long)
        .help_heading(CONFIG_HELP_HEADING)
        .global(true);

    arg = apply_arg_shape(arg, field);

    arg
}

/// Configures value or flag behaviour, optional short alias, `value_name`, and
/// allowed values on an [`Arg`] from shared configuration metadata.
fn apply_arg_shape(arg: Arg, field: &ConfigFieldArgMetadata) -> Arg {
    let mut shaped = arg;

    if let Some(short) = field.short {
        shaped = shaped.short(short);
    }

    if field.takes_value {
        shaped = shaped.action(if field.multiple {
            ArgAction::Append
        } else {
            ArgAction::Set
        });
        if let Some(value_name) = field.value_name {
            shaped = shaped.value_name(value_name);
        }
        if !field.possible_values.is_empty() {
            shaped = shaped.value_parser(field.possible_values.clone());
        }
    } else {
        shaped = shaped.action(ArgAction::SetTrue);
    }

    shaped
}
