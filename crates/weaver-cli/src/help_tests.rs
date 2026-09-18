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
    let first = try_command()
        .expect("built-in help command should construct")
        .render_long_help()
        .to_string();
    let second = try_command()
        .expect("built-in help command should construct")
        .render_long_help()
        .to_string();

    assert_eq!(first, second);
    assert!(first.contains("--config-path <PATH>"));
    assert!(first.contains("--locale <LOCALE>"));
    assert!(first.contains(ORDERING_CAVEAT));
}

#[test]
fn command_attaches_ordering_caveat_to_nested_help() {
    let mut command = try_command().expect("built-in help command should construct");
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
        name: "append_field".to_owned(),
        long: "append-field".to_owned(),
        short: None,
        help: "Appends example values",
        takes_value: true,
        multiple: true,
        value_name: Some("VALUE".to_owned()),
    };
    let matches = Command::new("test")
        .arg(config_arg_from_metadata(append))
        .try_get_matches_from(["test", "--append-field", "one", "--append-field", "two"])
        .expect("append flag should parse");
    let values = matches
        .get_many::<String>("append_field")
        .expect("append values should be present")
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(values, ["one", "two"]);

    let switch = ConfigFieldArgMetadata {
        name: "switch_field".to_owned(),
        long: "switch-field".to_owned(),
        short: None,
        help: "Enables the example switch",
        takes_value: false,
        multiple: false,
        value_name: None,
    };
    let matches = Command::new("test")
        .arg(config_arg_from_metadata(switch))
        .try_get_matches_from(["test", "--switch-field"])
        .expect("switch flag should parse");
    assert_eq!(matches.get_one::<bool>("switch_field").copied(), Some(true));

    let switch = ConfigFieldArgMetadata {
        name: "switch_field".to_owned(),
        long: "switch-field".to_owned(),
        short: None,
        help: "Enables the example switch",
        takes_value: false,
        multiple: false,
        value_name: None,
    };
    let error = Command::new("test")
        .arg(config_arg_from_metadata(switch))
        .try_get_matches_from(["test", "--switch-field", "value"])
        .expect_err("switch flag should reject a value");
    assert_eq!(error.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn projection_failure_returns_construction_error_without_a_command() {
    let root = deep_command_chain(command_ir::MAX_COMMAND_DEPTH + 1);

    let result = build_command(root, [metadata::EN_US_MESSAGES]);

    assert!(matches!(
        result,
        Err(HelpConstructionError::ProjectCommandMetadata {
            source: command_ir::ProjectionError::DepthExceeded { .. }
        })
    ));
}

fn deep_command_chain(depth: usize) -> &'static command_tree::CommandNode {
    let mut node = Box::leak(Box::new(command_tree::CommandNode {
        resource_path: &[],
        verb: "leaf",
        summary_id: "weaver-command-root",
        summary: "leaf",
        arguments: &[],
        semantics: command_tree::CommandSemantics::Structured,
        children: &[],
    }));
    for _ in 0..depth {
        node = Box::leak(Box::new(command_tree::CommandNode {
            resource_path: &[],
            verb: "parent",
            summary_id: "weaver-command-root",
            summary: "parent",
            arguments: &[],
            semantics: command_tree::CommandSemantics::Structured,
            children: std::slice::from_ref(node),
        }));
    }
    node
}
