//! Projects Weaver's command surface into OrthoConfig documentation metadata.

use ortho_config::docs::{
    CliMetadata,
    DocMetadata,
    FieldMetadata,
    HeadingIds,
    ORTHO_DOCS_IR_VERSION,
    SectionsMetadata,
    ValueType,
};

use crate::command_tree::{CommandArgument, CommandNode, CommandSemantics};

/// Maximum supported command-tree depth.
pub(crate) const MAX_COMMAND_DEPTH: usize = 8;

/// Failure to project a command-surface tree.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ProjectionError {
    /// The tree exceeds the bounded recursive representation Weaver supports.
    #[error("command tree exceeds the maximum depth of {max_depth}")]
    DepthExceeded {
        /// Largest supported depth, counting the root as depth zero.
        max_depth: usize,
    },
}

/// Projects Weaver's command tree into the OrthoConfig documentation IR.
pub(crate) fn project(root: &CommandNode) -> Result<DocMetadata, ProjectionError> {
    project_node(root, 0)
}

/// Projects one tree node after confirming its depth is within Weaver's bound.
fn project_node(node: &CommandNode, depth: usize) -> Result<DocMetadata, ProjectionError> {
    if depth > MAX_COMMAND_DEPTH {
        return Err(ProjectionError::DepthExceeded {
            max_depth: MAX_COMMAND_DEPTH,
        });
    }

    Ok(DocMetadata {
        ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: command_name(node),
        bin_name: (depth == 0).then(|| node.verb.to_owned()),
        about_id: node.summary_id.to_owned(),
        synopsis_id: None,
        sections: sections_metadata(),
        fields: node.arguments.iter().map(field_metadata).collect(),
        subcommands: project_children(node, depth)?,
        windows: None,
    })
}

/// Joins a node's resource path and verb into its metadata command name.
fn command_name(node: &CommandNode) -> String {
    let mut segments = node.resource_path.to_vec();
    if segments.last().copied() != Some(node.verb) {
        segments.push(node.verb);
    }
    segments.join(" ")
}

/// Recursively projects children that represent structured clap subcommands.
fn project_children(node: &CommandNode, depth: usize) -> Result<Vec<DocMetadata>, ProjectionError> {
    match node.semantics {
        CommandSemantics::Structured => node
            .children
            .iter()
            .filter(|child| matches!(child.semantics, CommandSemantics::Structured))
            .map(|child| project_node(child, depth + 1))
            .collect(),
        CommandSemantics::DaemonPassthrough { .. } => Ok(Vec::new()),
    }
}

/// Projects one command-tree argument into OrthoConfig field metadata.
fn field_metadata(argument: &CommandArgument) -> FieldMetadata {
    FieldMetadata {
        name: argument.long.to_owned(),
        help_id: argument.help_id.to_owned(),
        long_help_id: None,
        value: argument.value_name.map(|_| ValueType::String),
        default: None,
        required: argument.required,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some(argument.long.to_owned()),
            short: None,
            value_name: argument.value_name.map(str::to_owned),
            multiple: false,
            takes_value: argument.value_name.is_some(),
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

/// Creates the shared localized section headings for projected command nodes.
fn sections_metadata() -> SectionsMetadata {
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

#[cfg(test)]
mod tests;
