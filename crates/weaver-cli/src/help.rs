//! Shared clap help rendering for runtime help output and manpage generation.
//!
//! The runtime parser intentionally keeps configuration flags outside clap so
//! they only take effect before the command domain. This module augments the
//! help-only clap command with those flags so help and generated manpages stay
//! truthful without weakening runtime parsing semantics.

use std::{ffi::OsString, io::Write, sync::OnceLock};

use clap::{Arg, ArgAction, Command, CommandFactory};
use ortho_config::docs::{FieldMetadata, OrthoConfigDocs};
use weaver_config::{Config, config_field_help};

use crate::cli::Cli;

const CONFIG_PATH_ARG_ID: &str = "config-path";
const CONFIG_HELP_HEADING: &str = "Options";
const ORDERING_CAVEAT: &str = "Config flags must appear before the command domain or structured \
                               subcommand to take effect; for example, `weaver daemon start \
                               --log-filter debug` is ignored because `--log-filter` appears \
                               after `start`.";

static AUGMENTED_COMMAND: OnceLock<Command> = OnceLock::new();

struct ConfigFieldArgMetadata {
    name: &'static str,
    long: &'static str,
    short: Option<char>,
    help: &'static str,
    takes_value: bool,
    multiple: bool,
    value_name: Option<&'static str>,
}

/// Returns an augmented `clap::Command` that adds shared configuration flags
/// for help rendering and manpage generation without affecting the runtime
/// parser.
pub(crate) fn command() -> Command { AUGMENTED_COMMAND.get_or_init(build_command).clone() }

/// Writes help for the provided arguments using the augmented help command.
pub fn write_help_for_args<W: Write>(args: &[OsString], writer: &mut W) -> std::io::Result<()> {
    match command().try_get_matches_from(args.iter().cloned()) {
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            write!(writer, "{error}")
        }
        Err(error) => write!(writer, "{error}"),
        Ok(_) => {
            let mut fallback = command();
            fallback.write_long_help(writer)?;
            writeln!(writer)
        }
    }
}

fn build_command() -> Command {
    let mut command = Cli::command();
    command = command.arg(config_path_arg());

    for field in Config::get_doc_metadata().fields {
        if let Some(arg) = config_field_arg(&field) {
            command = command.arg(arg);
        }
    }

    attach_ordering_caveat(command)
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
        name: promote_static(field.name.clone()),
        long: promote_static(long.to_string()),
        short: cli.short,
        help: config_field_help(&field.help_id),
        takes_value: cli.takes_value,
        multiple: cli.multiple,
        value_name: cli.value_name.clone().map(promote_static),
    };

    Some(config_arg_from_metadata(&metadata))
}

fn promote_static(value: String) -> &'static str {
    // SAFETY: This intentionally promotes the given `String` to a `'static`
    // `str` because clap requires process-lifetime metadata for dynamically
    // built arguments. The leak is effectively process-lifetime and bounded by
    // the `OnceLock` command cache, which performs this promotion once per
    // field. This tradeoff satisfies clap's `'static` argument metadata
    // requirement while avoiding unbounded leaks.
    Box::leak(value.into_boxed_str())
}

/// Maps shared configuration metadata to a `clap::Arg`.
fn config_arg_from_metadata(field: &ConfigFieldArgMetadata) -> Arg {
    let mut arg = Arg::new(field.name)
        .long(field.long)
        .help(field.help)
        .help_heading(CONFIG_HELP_HEADING)
        .global(true);

    arg = apply_arg_shape(arg, field);

    arg
}

fn attach_ordering_caveat(command: Command) -> Command {
    let command = command.mut_subcommands(attach_ordering_caveat);
    let after_help = command.get_after_help().map_or_else(
        || ORDERING_CAVEAT.to_string(),
        |existing| format!("{existing}\n\n{ORDERING_CAVEAT}"),
    );
    command.after_help(after_help)
}

/// Configures value or flag behaviour, optional short alias, `value_name`, and
/// intentionally defers allowed-value validation to runtime config parsing.
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
    } else {
        shaped = shaped.action(ArgAction::SetTrue);
    }

    shaped
}

#[cfg(test)]
mod tests {
    //! Tests for augmented help command construction and argument shaping.

    use clap::error::ErrorKind;
    use ortho_config::docs::CliMetadata;

    use super::*;

    fn field_metadata(cli: Option<CliMetadata>) -> FieldMetadata {
        FieldMetadata {
            name: "example_field".to_string(),
            help_id: "example-help".to_string(),
            long_help_id: None,
            value: None,
            default: None,
            required: false,
            deprecated: None,
            cli,
            env: None,
            file: None,
            examples: Vec::new(),
            links: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn cli_metadata(takes_value: bool) -> CliMetadata {
        CliMetadata {
            long: Some("example-field".to_string()),
            short: Some('e'),
            value_name: Some("VALUE".to_string()),
            multiple: false,
            takes_value,
            possible_values: Vec::new(),
            hide_in_help: false,
        }
    }

    #[test]
    fn command_returns_reusable_augmented_command() {
        let first = command().render_long_help().to_string();
        let second = command().render_long_help().to_string();

        assert_eq!(first, second);
        assert!(first.contains("--config-path <PATH>"));
        assert!(first.contains("--locale <LOCALE>"));
        assert!(first.contains(ORDERING_CAVEAT));
    }

    #[test]
    fn command_attaches_ordering_caveat_to_nested_help() {
        let mut command = command();
        let daemon = command
            .find_subcommand_mut("daemon")
            .expect("daemon subcommand should exist");
        let start = daemon
            .find_subcommand_mut("start")
            .expect("daemon start subcommand should exist");

        assert!(
            start
                .render_long_help()
                .to_string()
                .contains(ORDERING_CAVEAT)
        );
    }

    #[test]
    fn config_path_arg_accepts_path_value() {
        let matches = Command::new("test")
            .arg(config_path_arg())
            .try_get_matches_from(["test", "--config-path", "weaver.toml"])
            .expect("config path should parse");

        assert_eq!(
            matches
                .get_one::<String>(CONFIG_PATH_ARG_ID)
                .map(String::as_str),
            Some("weaver.toml")
        );
    }

    #[test]
    fn config_field_arg_omits_hidden_or_unflagged_fields() {
        let mut hidden = cli_metadata(true);
        hidden.hide_in_help = true;
        let mut unflagged = cli_metadata(true);
        unflagged.long = None;

        assert!(config_field_arg(&field_metadata(Some(hidden))).is_none());
        assert!(config_field_arg(&field_metadata(Some(unflagged))).is_none());
        assert!(config_field_arg(&field_metadata(None)).is_none());
    }

    #[test]
    fn config_field_arg_uses_value_shape_without_enum_validation() {
        let mut cli = cli_metadata(true);
        cli.possible_values = vec!["json".to_string(), "compact".to_string()];
        let arg = config_field_arg(&field_metadata(Some(cli))).expect("arg should be visible");
        let matches = Command::new("test")
            .arg(arg)
            .try_get_matches_from(["test", "--example-field", "JSON"])
            .expect("help parser should not validate config values");

        assert_eq!(
            matches
                .get_one::<String>("example_field")
                .map(String::as_str),
            Some("JSON")
        );
    }

    #[test]
    fn config_field_arg_uses_help_id_metadata_for_help_text() {
        let mut field = field_metadata(Some(cli_metadata(true)));
        field.help_id = "weaver.fields.locale.help".to_string();
        let arg = config_field_arg(&field).expect("arg should be visible");
        let mut command = Command::new("test").arg(arg);

        assert!(
            command
                .render_long_help()
                .to_string()
                .contains("Selects the operator-facing locale")
        );
    }

    #[test]
    fn apply_arg_shape_supports_append_and_boolean_flags() {
        let append = ConfigFieldArgMetadata {
            name: "append_field",
            long: "append-field",
            short: None,
            help: "Appends example values",
            takes_value: true,
            multiple: true,
            value_name: Some("VALUE"),
        };
        let matches = Command::new("test")
            .arg(config_arg_from_metadata(&append))
            .try_get_matches_from(["test", "--append-field", "one", "--append-field", "two"])
            .expect("append flag should parse");
        let values = matches
            .get_many::<String>("append_field")
            .expect("append values should be present")
            .map(String::as_str)
            .collect::<Vec<_>>();
        assert_eq!(values, ["one", "two"]);

        let switch = ConfigFieldArgMetadata {
            name: "switch_field",
            long: "switch-field",
            short: None,
            help: "Enables the example switch",
            takes_value: false,
            multiple: false,
            value_name: None,
        };
        let matches = Command::new("test")
            .arg(config_arg_from_metadata(&switch))
            .try_get_matches_from(["test", "--switch-field"])
            .expect("switch flag should parse");
        assert_eq!(matches.get_one::<bool>("switch_field").copied(), Some(true));

        let error = Command::new("test")
            .arg(config_arg_from_metadata(&switch))
            .try_get_matches_from(["test", "--switch-field", "value"])
            .expect_err("switch flag should reject a value");
        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }
}
