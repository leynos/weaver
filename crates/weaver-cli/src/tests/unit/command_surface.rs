//! Unit tests for resource-first command-surface adapter behaviour.

use clap::Parser;

use crate::{Cli, CommandInvocation, READ_ONLY_COMMANDS};

#[test]
fn structured_definitions_get_uses_command_surface_adapter() {
    let cli = Cli::try_parse_from([
        "weaver",
        "definitions",
        "get",
        "--uri",
        "file:///src/main.rs",
        "--position",
        "10:5",
    ])
    .expect("parse definitions get");

    let invocation = CommandInvocation::try_from(cli).expect("map structured command");

    assert_eq!(
        invocation,
        CommandInvocation {
            domain: String::from("observe"),
            operation: String::from("get-definition"),
            arguments: vec![
                String::from("--uri"),
                String::from("file:///src/main.rs"),
                String::from("--position"),
                String::from("10:5"),
            ],
        }
    );
}

#[test]
fn read_only_command_surface_metadata_keeps_family_shape() {
    let records: Vec<(&[&str], &str, &str)> = READ_ONLY_COMMANDS
        .iter()
        .map(|record| (record.resource_path, record.verb, record.capability_id))
        .collect();

    assert_eq!(
        records,
        vec![
            (&["definitions"][..], "get", "definition.get"),
            (&["references"][..], "list", "references.list"),
        ]
    );
}
