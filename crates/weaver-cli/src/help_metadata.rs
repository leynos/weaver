//! Applies recursive command metadata to Clap help rendering.

use clap::Command;
use ortho_config::{FluentLocalizer, Localizer, NoOpLocalizer, docs::DocMetadata};

use crate::command_tree::{CommandNode, CommandSemantics};

const EN_US_MESSAGES: &str = include_str!("../locales/en-US/messages.ftl");

/// Applies projected command metadata to the parser-shaped help command.
pub(super) fn apply(command: Command, metadata: &DocMetadata, node: &CommandNode) -> Command {
    let localizer = FluentLocalizer::with_en_us_defaults([EN_US_MESSAGES]);
    match localizer {
        Ok(localizer) => apply_node(command, metadata, node, &localizer),
        Err(error) => {
            tracing::warn!(error = %error, "failed to load help localisation catalogue");
            apply_node(command, metadata, node, &NoOpLocalizer)
        }
    }
}

/// Applies one projected node and its descendants to the matching clap command.
fn apply_node(
    command: Command,
    metadata: &DocMetadata,
    node: &CommandNode,
    localizer: &dyn Localizer,
) -> Command {
    let command = command.about(localizer.message(&metadata.about_id, None, node.summary));
    let command = apply_arguments(command, metadata, node, localizer);
    let command = apply_subcommands(command, metadata, node, localizer);
    apply_passthrough_help(command, node, localizer)
}

/// Localizes projected long-flag help onto the matching clap arguments.
fn apply_arguments(
    mut command: Command,
    metadata: &DocMetadata,
    node: &CommandNode,
    localizer: &dyn Localizer,
) -> Command {
    for (argument, field) in node.arguments.iter().zip(&metadata.fields) {
        let help = localizer.message(&field.help_id, None, argument.help);
        command = command.mut_arg(argument.long, |arg| arg.help(help));
    }
    command
}

/// Recursively applies projected metadata to structured clap subcommands.
fn apply_subcommands(
    mut command: Command,
    metadata: &DocMetadata,
    node: &CommandNode,
    localizer: &dyn Localizer,
) -> Command {
    for (child, child_metadata) in node.children.iter().zip(&metadata.subcommands) {
        if let CommandSemantics::Structured = child.semantics {
            command = command.mut_subcommand(child.verb, |subcommand| {
                apply_node(subcommand, child_metadata, child, localizer)
            });
        }
    }
    command
}

/// Adds the legacy daemon-passthrough catalogue to the root help command.
fn apply_passthrough_help(
    command: Command,
    node: &CommandNode,
    localizer: &dyn Localizer,
) -> Command {
    let CommandSemantics::Structured = node.semantics else {
        return command;
    };
    let Some(passthrough) = node
        .children
        .iter()
        .find(|child| matches!(child.semantics, CommandSemantics::DaemonPassthrough { .. }))
    else {
        return command;
    };
    let CommandSemantics::DaemonPassthrough { domains } = passthrough.semantics else {
        return command;
    };

    let mut output = String::from("Commands:\n\n");
    append_structured_commands(&mut output, node, &[], localizer);
    output.push_str("\nDomains and operations:\n");
    debug_assert_eq!(domains, crate::command_tree::domain_operations());
    for domain in crate::command_tree::domain_operations() {
        let summary = localizer.message(domain.summary_id, None, domain.summary);
        output.push_str(&format!("\n  {} \u{2014} {summary}\n", domain.domain));
        for operations in domain.operations.chunks(3) {
            output.push_str("    ");
            output.push_str(&format_operation_row(operations));
            output.push('\n');
        }
    }

    output.pop();

    command.after_help(output)
}

/// Appends structured command signatures in depth-first display order.
fn append_structured_commands(
    output: &mut String,
    node: &CommandNode,
    path: &[&str],
    localizer: &dyn Localizer,
) {
    for child in node.children {
        if let CommandSemantics::Structured = child.semantics {
            let mut child_path = path.to_vec();
            child_path.push(child.verb);
            let summary = localizer.message(child.summary_id, None, child.summary);
            output.push_str(&format!(
                "  {} \u{2014} {summary}\n",
                command_signature(&child_path, child),
            ));
            append_argument_help(output, child, localizer);
            append_structured_commands(output, child, &child_path, localizer);
        }
    }
}

/// Appends localised argument descriptions for a structured command.
fn append_argument_help(output: &mut String, node: &CommandNode, localizer: &dyn Localizer) {
    for argument in node.arguments {
        let help = localizer.message(argument.help_id, None, argument.help);
        output.push_str("    --");
        output.push_str(argument.long);
        if let Some(value_name) = argument.value_name {
            output.push_str(" <");
            output.push_str(value_name);
            output.push('>');
        }
        output.push_str(" \u{2014} ");
        output.push_str(&help);
        output.push('\n');
    }
}

/// Formats one command path with its long flags for the shared reference.
fn command_signature(path: &[&str], node: &CommandNode) -> String {
    let mut signature = path.join(" ");
    for argument in node.arguments {
        signature.push_str(" --");
        signature.push_str(argument.long);
        if let Some(value_name) = argument.value_name {
            signature.push_str(" <");
            signature.push_str(value_name);
            signature.push('>');
        }
    }
    signature
}

/// Formats one row of daemon operations for fixed-column help output.
fn format_operation_row(operations: &[&str]) -> String {
    const SECOND_COLUMN_START: usize = 18;
    const THIRD_COLUMN_START: usize = 37;

    let mut row = operations.first().copied().unwrap_or_default().to_owned();
    if let Some(second) = operations.get(1) {
        pad_to(&mut row, SECOND_COLUMN_START);
        row.push_str(second);
    }
    if let Some(third) = operations.get(2) {
        pad_to(&mut row, THIRD_COLUMN_START);
        row.push_str(third);
    }
    row
}

/// Pads a row to a fixed display width before the next operation column.
fn pad_to(row: &mut String, width: usize) {
    if row.len() < width {
        row.extend(std::iter::repeat_n(' ', width - row.len()));
    }
}

#[cfg(test)]
mod tests {
    //! Behavioural tests for recursive metadata application to rendered help.

    use clap::CommandFactory;
    use ortho_config::{LocalizationArgs, Localizer};

    use super::{apply_node, apply_passthrough_help};
    use crate::{cli::Cli, command_ir, command_tree};

    struct DistinctLocalizer;

    impl Localizer for DistinctLocalizer {
        fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
            match id {
                "weaver-command-definitions-get" => {
                    Some("Translated definition lookup summary".to_owned())
                }
                "weaver-command-definitions-get-uri" => {
                    Some("Translated document URI argument".to_owned())
                }
                "weaver-command-definitions-get-position" => {
                    Some("Translated source position argument".to_owned())
                }
                "weaver-command-domain-observe" => {
                    Some("Translated code-structure domain".to_owned())
                }
                _ => None,
            }
        }
    }

    #[test]
    fn metadata_application_localizes_recursive_help_and_manpage() -> anyhow::Result<()> {
        let root = command_tree::root();
        let metadata = command_ir::project(root)?;
        let localizer = DistinctLocalizer;
        let mut command = apply_node(Cli::command(), &metadata, root, &localizer);
        command = apply_passthrough_help(command, root, &localizer);

        assert_eq!(metadata.subcommands.len(), 2);
        assert!(
            command.get_after_help().is_some_and(|help| help
                .to_string()
                .contains("Translated code-structure domain")),
            "passthrough domains must be rendered from the tree, not projected as Clap subcommands"
        );

        let definitions = command
            .find_subcommand_mut("definitions")
            .expect("definitions command should exist");
        let rendered_help = definitions
            .find_subcommand_mut("get")
            .expect("definitions get command should exist")
            .render_long_help()
            .to_string();
        assert!(rendered_help.contains("Translated definition lookup summary"));
        assert!(rendered_help.contains("Translated document URI argument"));
        assert!(rendered_help.contains("Translated source position argument"));

        let mut manpage = Vec::new();
        clap_mangen::Man::new(command).render(&mut manpage)?;
        let rendered_manpage = String::from_utf8(manpage)?;
        assert!(
            rendered_manpage.contains("Translated definition lookup summary"),
            "manpage must consume recursively applied command metadata"
        );
        assert!(
            rendered_manpage.contains("Translated document URI argument"),
            "manpage must consume recursively applied argument metadata"
        );
        assert!(
            rendered_manpage.contains("Translated source position argument"),
            "manpage must consume recursively applied argument metadata"
        );

        Ok(())
    }
}
