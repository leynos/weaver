//! Import extraction helpers shared across supported languages.

use tree_sitter::Node;
use weaver_syntax::SupportedLanguage;

use crate::extract::ImportBlock;

pub(super) fn top_level_imports(
    language: SupportedLanguage,
    root: Node<'_>,
    source: &str,
) -> Vec<ImportBlock> {
    let kinds: &[&str] = match language {
        SupportedLanguage::Rust => &["use_declaration", "extern_crate_declaration"],
        SupportedLanguage::Python => &["import_statement", "import_from_statement"],
        SupportedLanguage::TypeScript => &["import_statement"],
    };

    let mut cursor = root.walk();
    let nodes: Vec<Node<'_>> = root
        .named_children(&mut cursor)
        .filter(|child| kinds.contains(&child.kind()))
        .collect();

    group_consecutive_nodes(nodes)
        .into_iter()
        .filter_map(|group| import_block_from_group(language, &group, source))
        .collect()
}

pub(super) fn group_consecutive_nodes(nodes: Vec<Node<'_>>) -> Vec<Vec<Node<'_>>> {
    let mut groups: Vec<Vec<Node<'_>>> = Vec::new();
    for node in nodes {
        if let Some(group) = groups.last_mut() {
            let previous_end = group.last().map_or(0, |n| n.end_position().row);
            if node.start_position().row <= previous_end + 1 {
                group.push(node);
                continue;
            }
        }
        groups.push(vec![node]);
    }
    groups
}

pub(super) fn import_block_from_group(
    language: SupportedLanguage,
    group: &[Node<'_>],
    source: &str,
) -> Option<ImportBlock> {
    let start = group.first().map(Node::start_byte)?;
    let end = group.last().map(Node::end_byte)?;
    source.get(start..end)?;
    let normalized = group
        .iter()
        .map(|node| normalise_import(language, source.get(node.byte_range()).unwrap_or_default()))
        .collect();
    Some(ImportBlock {
        byte_start: start,
        byte_end: end,
        normalized,
    })
}

pub(super) fn normalise_import(language: SupportedLanguage, raw: &str) -> String {
    let trimmed = raw.trim();
    match language {
        SupportedLanguage::Rust => trimmed
            .trim_start_matches("pub ")
            .trim_start_matches("use ")
            .trim_start_matches("extern crate ")
            .trim_end_matches(';')
            .trim()
            .to_owned(),
        SupportedLanguage::Python => trimmed
            .trim_start_matches("from ")
            .trim_start_matches("import ")
            .trim()
            .to_owned(),
        SupportedLanguage::TypeScript => trimmed
            .trim_start_matches("import ")
            .trim_end_matches(';')
            .trim()
            .to_owned(),
    }
}
