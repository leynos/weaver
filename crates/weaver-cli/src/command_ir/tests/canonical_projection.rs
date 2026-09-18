//! Independent canonical contract tests for the recursive command projection.

use ortho_config::docs::{
    CliMetadata,
    DocMetadata,
    FieldMetadata,
    HeadingIds,
    ORTHO_DOCS_IR_VERSION,
    SectionsMetadata,
    ValueType,
};

use super::super::project;
use crate::command_tree;

#[test]
fn canonical_projection_matches_the_independent_command_contract() {
    let metadata =
        project(command_tree::root()).expect("the built-in command tree should be bounded");

    assert_canonical_command(&canonical_projection(), &metadata, "weaver");
}

fn canonical_projection() -> DocMetadata {
    command(
        ("weaver", Some("weaver"), "weaver-command-root"),
        vec![],
        vec![
            command(
                ("definitions", None, "weaver-command-definitions"),
                vec![],
                vec![command(
                    ("definitions get", None, "weaver-command-definitions-get"),
                    vec![
                        field("uri", "weaver-command-definitions-get-uri", "URI"),
                        field(
                            "position",
                            "weaver-command-definitions-get-position",
                            "LINE:COLUMN",
                        ),
                    ],
                    vec![],
                )],
            ),
            command(
                ("daemon", None, "weaver-command-daemon"),
                vec![],
                vec![
                    command(
                        ("daemon start", None, "weaver-command-daemon-start"),
                        vec![],
                        vec![],
                    ),
                    command(
                        ("daemon stop", None, "weaver-command-daemon-stop"),
                        vec![],
                        vec![],
                    ),
                    command(
                        ("daemon status", None, "weaver-command-daemon-status"),
                        vec![],
                        vec![],
                    ),
                ],
            ),
        ],
    )
}

fn command(
    (app_name, bin_name, about_id): (&str, Option<&str>, &str),
    fields: Vec<FieldMetadata>,
    subcommands: Vec<DocMetadata>,
) -> DocMetadata {
    DocMetadata {
        ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: app_name.to_owned(),
        bin_name: bin_name.map(str::to_owned),
        about_id: about_id.to_owned(),
        synopsis_id: None,
        sections: canonical_sections(),
        fields,
        subcommands,
        windows: None,
    }
}

fn field(name: &str, help_id: &str, value_name: &str) -> FieldMetadata {
    FieldMetadata {
        name: name.to_owned(),
        help_id: help_id.to_owned(),
        long_help_id: None,
        value: Some(ValueType::String),
        default: None,
        required: true,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some(name.to_owned()),
            short: None,
            value_name: Some(value_name.to_owned()),
            multiple: false,
            takes_value: true,
            possible_values: Vec::new(),
            hide_in_help: false,
        }),
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

fn canonical_sections() -> SectionsMetadata {
    SectionsMetadata {
        headings_ids: HeadingIds {
            name: "weaver-doc-heading-name".to_owned(),
            synopsis: "weaver-doc-heading-synopsis".to_owned(),
            description: "weaver-doc-heading-description".to_owned(),
            options: "weaver-doc-heading-options".to_owned(),
            environment: "weaver-doc-heading-environment".to_owned(),
            files: "weaver-doc-heading-files".to_owned(),
            precedence: "weaver-doc-heading-precedence".to_owned(),
            exit_status: "weaver-doc-heading-exit-status".to_owned(),
            examples: "weaver-doc-heading-examples".to_owned(),
            see_also: "weaver-doc-heading-see-also".to_owned(),
            commands: Some("weaver-doc-heading-commands".to_owned()),
        },
        discovery: None,
        precedence: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

fn assert_canonical_command(expected: &DocMetadata, actual: &DocMetadata, path: &str) {
    assert_eq!(actual.ir_version, expected.ir_version, "{path}.ir_version");
    assert_eq!(actual.app_name, expected.app_name, "{path}.app_name");
    assert_eq!(actual.bin_name, expected.bin_name, "{path}.bin_name");
    assert_eq!(actual.about_id, expected.about_id, "{path}.about_id");
    assert_eq!(
        actual.synopsis_id, expected.synopsis_id,
        "{path}.synopsis_id"
    );
    assert_eq!(actual.sections, expected.sections, "{path}.sections");
    assert_eq!(actual.windows, expected.windows, "{path}.windows");
    assert_eq!(
        actual.fields.len(),
        expected.fields.len(),
        "{path}.fields length and order"
    );
    for (index, (expected_field, actual_field)) in
        expected.fields.iter().zip(&actual.fields).enumerate()
    {
        assert_eq!(actual_field, expected_field, "{path}.fields[{index}]");
    }
    assert_eq!(
        actual.subcommands.len(),
        expected.subcommands.len(),
        "{path}.subcommands and daemon-passthrough exclusion"
    );
    for (expected_child, actual_child) in expected.subcommands.iter().zip(&actual.subcommands) {
        assert_canonical_command(
            expected_child,
            actual_child,
            &format!("{path} {}", expected_child.app_name),
        );
    }
}
