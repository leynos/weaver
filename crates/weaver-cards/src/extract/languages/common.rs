//! Shared Tree-sitter extraction helpers across supported languages.

#[path = "imports.rs"]
mod imports;
#[path = "params.rs"]
mod params;
#[path = "structure.rs"]
mod structure;

use tree_sitter::Node;

use crate::{CardSymbolKind, SourcePosition, SourceRange};

use super::super::EntityCandidate;
use params::parse_parameters;
use structure::collect_structure;

#[derive(Debug, Clone)]
pub(super) struct CallableMetadata {
    pub(super) container: Option<String>,
    pub(super) decorators: Vec<String>,
    pub(super) docstring: Option<String>,
}

impl CallableMetadata {
    pub(super) const fn new(
        container: Option<String>,
        decorators: Vec<String>,
        docstring: Option<String>,
    ) -> Self {
        Self {
            container,
            decorators,
            docstring,
        }
    }
}

pub(super) fn top_level_imports(
    language: weaver_syntax::SupportedLanguage,
    root: Node<'_>,
    source: &str,
) -> Vec<crate::extract::ImportBlock> {
    imports::top_level_imports(language, root, source)
}

pub(super) fn callable_candidate(
    node: Node<'_>,
    source: &str,
    kind: CardSymbolKind,
    metadata: CallableMetadata,
) -> EntityCandidate {
    let body = node.child_by_field_name("body");
    let signature_end = body.map_or_else(|| node.end_byte(), |body_node| body_node.start_byte());
    let signature_source = source
        .get(node.start_byte()..signature_end)
        .unwrap_or_default();
    let range = to_range(node);
    let (locals, branches) = collect_structure(node, body, source);
    let params = node
        .child_by_field_name("parameters")
        .map_or_else(Vec::new, |param_node| parse_parameters(param_node, source));
    let returns = node
        .child_by_field_name("return_type")
        .map_or_else(String::new, |return_node| {
            normalise_whitespace(source.get(return_node.byte_range()).unwrap_or_default())
        });

    EntityCandidate {
        kind,
        name: name_text(node, source),
        container: metadata.container,
        byte_range: node.byte_range(),
        range: range.clone(),
        signature_display: Some(normalise_whitespace(signature_source)),
        params,
        returns,
        locals,
        branches: branches.clone(),
        decorators: metadata.decorators,
        attachment_anchor: Some(node.start_byte()),
        docstring: metadata.docstring,
        lines: range.end.line.saturating_sub(range.start.line) + 1,
        structure_fingerprint: format!(
            "{}:{}:{}:{}",
            node.kind(),
            node.child_by_field_name("parameters")
                .map_or(0, |param_node| parse_parameters(param_node, source).len()),
            branches.len(),
            node.child_by_field_name("body").map_or(0, |_| 1)
        ),
        interstitial: None,
    }
}

pub(super) fn simple_candidate(
    node: Node<'_>,
    source: &str,
    kind: CardSymbolKind,
    container: Option<String>,
) -> EntityCandidate {
    let range = to_range(node);
    EntityCandidate {
        kind,
        name: name_text(node, source),
        container,
        byte_range: node.byte_range(),
        range: range.clone(),
        signature_display: None,
        params: Vec::new(),
        returns: String::new(),
        locals: Vec::new(),
        branches: Vec::new(),
        decorators: Vec::new(),
        attachment_anchor: Some(node.start_byte()),
        docstring: None,
        lines: range.end.line.saturating_sub(range.start.line) + 1,
        structure_fingerprint: String::from(node.kind()),
        interstitial: None,
    }
}

pub(super) fn name_text(node: Node<'_>, source: &str) -> String {
    node.child_by_field_name("name")
        .and_then(|name| source.get(name.byte_range()))
        .map_or_else(
            || normalise_whitespace(source.get(node.byte_range()).unwrap_or_default()),
            normalise_whitespace,
        )
}

pub(super) fn impl_container_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .and_then(|ty| source.get(ty.byte_range()))
        .map(normalise_whitespace)
}

pub(super) fn decorator_texts(node: Node<'_>, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "decorator" {
            decorators.push(normalise_whitespace(
                source.get(child.byte_range()).unwrap_or_default(),
            ));
        }
    }
    decorators
}

pub(super) fn python_docstring(node: Node<'_>, source: &str) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();
    let first_statement = body.named_children(&mut cursor).next()?;
    if first_statement.kind() != "expression_statement" {
        return None;
    }

    let mut statement_cursor = first_statement.walk();
    let string_node = first_statement
        .named_children(&mut statement_cursor)
        .find(|child| child.kind() == "string")?;
    let raw = source.get(string_node.byte_range())?;
    let trimmed = raw.trim_matches('"').trim_matches('\'').trim();
    (!trimmed.is_empty()).then(|| String::from(trimmed))
}

pub(super) const fn extract_rust_docstring(_source: &str, _node: Node<'_>) -> Option<String> {
    None
}

pub(super) fn normalise_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn to_range(node: Node<'_>) -> SourceRange {
    let start = node.start_position();
    let end = node.end_position();
    SourceRange {
        start: SourcePosition {
            line: to_u32(start.row),
            column: to_u32(start.column),
        },
        end: SourcePosition {
            line: to_u32(end.row),
            column: to_u32(end.column),
        },
    }
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
