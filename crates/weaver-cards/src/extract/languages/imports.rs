//! Import extraction helpers shared across supported languages.

use tree_sitter::Node;
use weaver_syntax::SupportedLanguage;

use crate::extract::ImportBlock;

/// Collects top-level import blocks for the requested language.
///
/// `language` selects the import node kinds to match, `root` is the parsed
/// syntax-tree root, and `source` provides the text slices used to build each
/// [`ImportBlock`]. Returns consecutive import statements grouped into
/// normalized blocks, or an empty vector when the file has no top-level
/// imports.
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

/// Groups consecutive import nodes that touch or are separated by one line.
///
/// The returned outer vector preserves source order. A new group starts when a
/// node begins more than one row after the previous node's end row.
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

/// Builds a single [`ImportBlock`] from one consecutive import group.
///
/// `language` controls normalization rules, `group` supplies the contiguous
/// import nodes, and `source` provides the original byte slices. Returns
/// `None` when the group is empty or its byte range cannot be sliced from the
/// source text.
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

/// Normalizes a raw import statement for language-specific comparisons.
///
/// The returned string strips leading syntax such as `use`, `import`, or Rust
/// visibility modifiers, removes trailing statement terminators where
/// appropriate, and trims surrounding whitespace.
pub(super) fn normalise_import(language: SupportedLanguage, raw: &str) -> String {
    let trimmed = raw.trim();
    match language {
        SupportedLanguage::Rust => strip_rust_visibility(trimmed)
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

fn strip_rust_visibility(raw: &str) -> &str {
    let Some(after_pub) = raw.strip_prefix("pub") else {
        return raw;
    };
    if let Some(rest) = after_pub.strip_prefix(char::is_whitespace) {
        return rest.trim_start();
    }
    let Some(scoped) = after_pub.strip_prefix('(') else {
        return raw;
    };

    let mut depth = 1usize;
    for (index, ch) in scoped.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return scoped.get(index + 1..).map_or(raw, str::trim_start);
                }
            }
            _ => {}
        }
    }

    raw
}
