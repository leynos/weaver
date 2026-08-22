//! Tests for the recursive command-metadata projection.

use ortho_config::{
    FluentLocalizer,
    Localizer,
    OrthoConfig,
    docs::{
        DocMetadata,
        EnvMetadata,
        FieldMetadata,
        FileMetadata,
        ORTHO_DOCS_IR_VERSION,
        OrthoConfigDocs,
        ValueType,
    },
};
use proptest::prelude::*;
use serde::{Deserialize, Serialize};

use super::{MAX_COMMAND_DEPTH, ProjectionError, field_metadata, project};
use crate::{
    command_tree::{self, CommandArgument, CommandNode, CommandSemantics},
    localizer::WEAVER_EN_US,
};

#[test]
fn projection_preserves_the_structured_command_hierarchy() {
    let metadata =
        project(command_tree::root()).expect("the built-in command tree should be bounded");

    assert_eq!(metadata.ir_version, ORTHO_DOCS_IR_VERSION);
    assert_eq!(metadata.app_name, "weaver");
    assert_eq!(
        metadata
            .subcommands
            .iter()
            .map(|node| node.app_name.as_str())
            .collect::<Vec<_>>(),
        ["definitions", "daemon", "domain-operation"],
    );
    assert_eq!(
        metadata.subcommands[0]
            .subcommands
            .iter()
            .map(|node| node.app_name.as_str())
            .collect::<Vec<_>>(),
        ["definitions get"],
    );
}

#[test]
fn projection_roundtrips_through_the_upstream_ir_format() {
    let metadata =
        project(command_tree::root()).expect("the built-in command tree should be bounded");
    let json = serde_json::to_string(&metadata).expect("metadata should serialize");
    let roundtrip: DocMetadata = serde_json::from_str(&json).expect("metadata should deserialize");

    assert_eq!(roundtrip, metadata);
}

#[derive(Debug, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "WEAVER")]
struct DerivedFixture {
    #[ortho_config(cli_long = "uri", cli(value_name = "URI"))]
    uri: String,
    #[ortho_config(cli_long = "force")]
    force: bool,
}

#[test]
fn hand_assembled_argument_metadata_matches_the_upstream_derive() {
    let derived = DerivedFixture::get_doc_metadata();
    let Some(uri) = derived_field("uri", Some("URI")) else {
        panic!("hand-assembled URI fixture field must be CLI-visible");
    };
    let Some(force) = derived_field("force", None) else {
        panic!("hand-assembled force fixture field must be CLI-visible");
    };
    let hand_assembled = vec![uri, force];

    assert_eq!(hand_assembled, derived.fields);
}

fn derived_field(long: &'static str, value_name: Option<&'static str>) -> Option<FieldMetadata> {
    let mut field = field_metadata(&CommandArgument {
        long,
        value_name,
        help_id: if long == "uri" {
            "weaver.fields.uri.help"
        } else {
            "weaver.fields.force.help"
        },
        help: "derived fixture fallback",
    });
    field.long_help_id = Some(format!("weaver.fields.{long}.long_help"));
    field.value = Some(if value_name.is_some() {
        ValueType::String
    } else {
        ValueType::Bool
    });
    field.required = true;
    let cli = field.cli.as_mut()?;
    cli.short = long.chars().next();
    field.env = Some(EnvMetadata {
        var_name: format!("WEAVER_{}", long.to_ascii_uppercase()),
    });
    field.file = Some(FileMetadata {
        key_path: long.to_owned(),
    });
    Some(field)
}

#[test]
fn projection_requires_the_reviewed_upstream_schema_version() {
    assert_eq!(ORTHO_DOCS_IR_VERSION, "1.1");
}

#[test]
fn every_projected_node_roundtrips_through_the_upstream_ir_format() -> serde_json::Result<()> {
    let metadata = project(command_tree::root()).expect("built-in command tree should be bounded");
    assert_roundtrips(&metadata)
}

fn assert_roundtrips(metadata: &DocMetadata) -> serde_json::Result<()> {
    let json = serde_json::to_string(metadata)?;
    let roundtrip: DocMetadata = serde_json::from_str(&json)?;
    assert_eq!(&roundtrip, metadata);
    for child in &metadata.subcommands {
        assert_roundtrips(child)?;
    }
    Ok(())
}

#[test]
fn every_projected_identifier_resolves_from_the_embedded_catalogue() {
    let localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
        .expect("embedded Fluent catalogue must parse");
    let metadata = project(command_tree::root()).expect("built-in command tree should be bounded");

    assert_identifiers_resolve(&metadata, &localizer);
}

#[test]
fn unresolved_identifiers_are_not_silently_accepted_by_the_test_helper() {
    let localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
        .expect("embedded Fluent catalogue must parse");

    assert!(
        localizer
            .lookup("weaver-command-does-not-exist", None)
            .is_none()
    );
}

fn assert_identifiers_resolve(metadata: &DocMetadata, localizer: &dyn Localizer) {
    assert_resolves(&metadata.about_id, localizer);
    if let Some(synopsis_id) = &metadata.synopsis_id {
        assert_resolves(synopsis_id, localizer);
    }
    let headings = &metadata.sections.headings_ids;
    for id in [
        &headings.name,
        &headings.synopsis,
        &headings.description,
        &headings.options,
        &headings.environment,
        &headings.files,
        &headings.precedence,
        &headings.exit_status,
        &headings.examples,
        &headings.see_also,
    ] {
        assert_resolves(id, localizer);
    }
    if let Some(commands) = &headings.commands {
        assert_resolves(commands, localizer);
    }
    for field in &metadata.fields {
        assert_resolves(&field.help_id, localizer);
        if let Some(long_help_id) = &field.long_help_id {
            assert_resolves(long_help_id, localizer);
        }
    }
    for child in &metadata.subcommands {
        assert_identifiers_resolve(child, localizer);
    }
}

fn assert_resolves(id: &str, localizer: &dyn Localizer) {
    assert!(
        localizer.lookup(id, None).is_some(),
        "projected Fluent identifier {id:?} must resolve without a fallback"
    );
}

const LABELS: &[&str] = &["zero", "one", "two", "three", "four"];

fn generated_tree(depth: usize, branching: usize, label: &'static str) -> &'static CommandNode {
    if depth == 0 {
        return Box::leak(Box::new(CommandNode {
            resource_path: &[],
            verb: label,
            summary_id: "weaver-command-root",
            summary: "generated command",
            arguments: &[],
            semantics: CommandSemantics::Structured,
            children: &[],
        }));
    }

    let children = (0..branching)
        .map(|index| *generated_tree(depth - 1, branching, LABELS[index]))
        .collect::<Vec<_>>();
    let children = Box::leak(children.into_boxed_slice());

    Box::leak(Box::new(CommandNode {
        resource_path: &[],
        verb: label,
        summary_id: "weaver-command-root",
        summary: "generated command",
        arguments: &[],
        semantics: CommandSemantics::Structured,
        children,
    }))
}

fn assert_shape_preserved(node: &CommandNode, metadata: &DocMetadata) {
    assert_eq!(metadata.subcommands.len(), node.children.len());
    for (child, child_metadata) in node.children.iter().zip(&metadata.subcommands) {
        assert_eq!(child_metadata.app_name, child.verb);
        assert_shape_preserved(child, child_metadata);
    }
}

proptest! {
    #[test]
    fn projection_preserves_generated_tree_shape(
        depth in 0usize..=4,
        branching in 0usize..=5,
    ) {
        let tree = generated_tree(depth, branching, "root");
        let metadata = project(tree).expect("generated trees remain within the depth bound");

        assert_shape_preserved(tree, &metadata);
    }
}

#[test]
fn projection_handles_a_leaf_only_tree_without_filtering_cases() {
    let tree = generated_tree(0, 0, "root");
    assert_shape_preserved(tree, &project(tree).expect("leaf should project"));
}

#[test]
fn projection_handles_a_tree_with_at_least_two_levels_without_filtering_cases() {
    let tree = generated_tree(2, 2, "root");
    assert_shape_preserved(tree, &project(tree).expect("nested tree should project"));
}

fn chain(depth: usize) -> &'static CommandNode {
    let mut node = Box::leak(Box::new(CommandNode {
        resource_path: &[],
        verb: "leaf",
        summary_id: "weaver-command-root",
        summary: "leaf",
        arguments: &[],
        semantics: CommandSemantics::Structured,
        children: &[],
    }));
    for _ in 0..depth {
        node = Box::leak(Box::new(CommandNode {
            resource_path: &[],
            verb: "parent",
            summary_id: "weaver-command-root",
            summary: "parent",
            arguments: &[],
            semantics: CommandSemantics::Structured,
            children: std::slice::from_ref(node),
        }));
    }
    node
}

#[test]
fn projection_accepts_trees_at_the_supported_depth_bound() {
    assert!(project(chain(MAX_COMMAND_DEPTH)).is_ok());
}

#[test]
fn projection_rejects_a_tree_beyond_the_supported_depth_bound() {
    assert_eq!(
        project(chain(MAX_COMMAND_DEPTH + 1)),
        Err(ProjectionError::DepthExceeded {
            max_depth: MAX_COMMAND_DEPTH,
        })
    );
}
