//! Shared Tree-sitter extraction helpers across supported languages.

// Use explicit paths so the imports, params, and structure helper modules stay
// colocated beside this file in the same directory.
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

/// Internal metadata collected alongside callable entities.
#[derive(Debug, Clone)]
pub(super) struct CallableMetadata {
    /// Optional container name for methods or nested callables.
    pub(super) container: Option<String>,
    /// Raw decorator or attribute texts attached to the callable.
    pub(super) decorators: Vec<String>,
    /// Language-specific docstring content, when the language exposes one.
    pub(super) docstring: Option<String>,
}

impl CallableMetadata {
    /// Creates internal callable metadata for the extraction pipeline.
    ///
    /// The returned value is used only within `weaver-cards` while assembling
    /// `EntityCandidate` values for callable symbols.
    ///
    /// Parameters:
    /// - `container`: optional container name such as a class or impl target.
    /// - `decorators`: raw decorator or annotation texts collected from the
    ///   syntax tree.
    /// - `docstring`: extracted language-native docstring content, if present.
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

/// Collects grouped top-level imports for a source file.
pub(super) fn top_level_imports(
    language: weaver_syntax::SupportedLanguage,
    root: Node<'_>,
    source: &str,
) -> Vec<crate::extract::ImportBlock> {
    imports::top_level_imports(language, root, source)
}

/// Builds an internal callable entity candidate from a syntax node.
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
    let param_count = params.len();
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
        signature_display: Some(normalise_whitespace_preserving_literals(signature_source)),
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
            param_count,
            branches.len(),
            node.child_by_field_name("body").map_or(0, |_| 1)
        ),
        interstitial: None,
    }
}

/// Builds a non-callable entity candidate from a syntax node.
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

/// Resolves the display name for a symbol node.
pub(super) fn name_text(node: Node<'_>, source: &str) -> String {
    node.child_by_field_name("name")
        .and_then(|name| source.get(name.byte_range()))
        .map_or_else(
            || normalise_whitespace(source.get(node.byte_range()).unwrap_or_default()),
            normalise_whitespace,
        )
}

/// Extracts the container name for Rust impl-like items.
pub(super) fn impl_container_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .and_then(|ty| source.get(ty.byte_range()))
        .map(normalise_whitespace)
}

/// Collects raw decorator texts from a language-specific syntax node.
pub(super) fn decorator_texts(node: Node<'_>, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "decorator" {
            decorators.push(normalise_whitespace_preserving_literals(
                source.get(child.byte_range()).unwrap_or_default(),
            ));
        }
    }
    decorators
}

/// Extracts the leading Python docstring from a callable or class body.
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
    extract_python_string_content(string_node, source)
}

/// Placeholder Rust docstring extractor until native docstring support lands.
pub(super) const fn extract_rust_docstring(_node: Node<'_>) -> Option<String> {
    None
}

/// Normalises whitespace while preserving string and template literal content.
pub(super) fn normalise_whitespace(raw: &str) -> String {
    normalise_whitespace_preserving_literals(raw)
}

/// Converts a Tree-sitter node range into Weaver's source-range model.
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

/// Returns `opening_index` when the prefix before the first quote character
/// is either absent or a valid string-literal prefix (e.g. `r`, `b`, `rb`).
fn string_literal_start(raw: &str, opening_index: usize) -> Option<usize> {
    let prefix = raw.get(..opening_index)?;
    if prefix.is_empty() || prefix.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Some(opening_index)
    } else {
        None
    }
}

/// Identifies the Python string delimiter used in `literal` (longest-match
/// first so that `"""` is preferred over `"`).
fn detect_delimiter(literal: &str) -> Option<&'static str> {
    const DELIMITERS: &[&str] = &["\"\"\"", "'''", "\"", "'"];
    DELIMITERS.iter().copied().find(|&d| literal.starts_with(d))
}

fn has_even_trailing_backslashes(result: &str) -> bool {
    let trailing_backslashes = result
        .chars()
        .rev()
        .skip(1)
        .take_while(|c| *c == '\\')
        .count();
    trailing_backslashes & 1 == 0
}

fn extract_python_string_content(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let parts: Vec<&str> = node
        .named_children(&mut cursor)
        .filter(|child| child.kind().contains("string_content"))
        .filter_map(|child| source.get(child.byte_range()))
        .collect();
    if !parts.is_empty() {
        return Some(parts.concat());
    }

    let raw = source.get(node.byte_range())?;
    let opening_index = raw.find(['"', '\''])?;
    let literal_start = string_literal_start(raw, opening_index)?;
    let literal = raw.get(literal_start..)?.trim();
    let delimiter = detect_delimiter(literal)?;

    let content = literal
        .strip_prefix(delimiter)
        .and_then(|rest| rest.strip_suffix(delimiter))?
        .to_owned();
    Some(content)
}

fn normalise_whitespace_preserving_literals(raw: &str) -> String {
    let mut result = String::new();
    let chars = raw.chars().peekable();
    let mut quote: Option<char> = None;
    let mut pending_space = false;

    for ch in chars {
        if let Some(active_quote) = quote {
            result.push(ch);
            if ch == active_quote && has_even_trailing_backslashes(&result) {
                quote = None;
            }
            continue;
        }

        if ch.is_whitespace() {
            pending_space = !result.is_empty();
            continue;
        }

        if pending_space {
            result.push(' ');
            pending_space = false;
        }

        if matches!(ch, '"' | '\'' | '`') {
            quote = Some(ch);
        }
        result.push(ch);
    }

    result.trim().to_owned()
}
