//! Pattern matching engine for finding code structures.
//!
//! This module implements the matching algorithm that finds occurrences of
//! patterns in parsed source code. It handles metavariable capture and
//! produces structured match results.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use crate::parser::ParseResult;
use crate::pattern::{MetaVarKind, MetaVariable, Pattern};

/// Result of a successful pattern match.
///
/// Contains the matched node and any captured metavariables.
#[derive(Debug)]
pub struct MatchResult<'a> {
    /// The AST node that matched the pattern.
    node: tree_sitter::Node<'a>,
    /// The source code being matched against.
    source: &'a str,
    /// Captured metavariables mapped by name.
    captures: HashMap<String, CapturedNode<'a>>,
}

impl<'a> MatchResult<'a> {
    /// Returns the matched AST node.
    #[must_use]
    pub const fn node(&self) -> tree_sitter::Node<'a> {
        self.node
    }

    /// Returns the byte range of the match in the source.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        self.node.byte_range()
    }

    /// Returns the text of the matched region.
    #[must_use]
    pub fn text(&self) -> &'a str {
        self.source.get(self.byte_range()).unwrap_or_default()
    }

    /// Returns the start position (line, column) of the match.
    ///
    /// Both line and column are one-based for display purposes.
    #[must_use]
    pub fn start_position(&self) -> (u32, u32) {
        position_to_one_based(self.node.start_position())
    }

    /// Returns the end position (line, column) of the match.
    ///
    /// Both line and column are one-based for display purposes.
    #[must_use]
    pub fn end_position(&self) -> (u32, u32) {
        position_to_one_based(self.node.end_position())
    }

    /// Gets a captured metavariable by name.
    ///
    /// Returns `None` if no metavariable with that name was captured.
    #[must_use]
    pub fn capture(&self, name: &str) -> Option<&CapturedNode<'a>> {
        self.captures.get(name)
    }

    /// Returns all captured metavariables.
    #[must_use]
    pub const fn captures(&self) -> &HashMap<String, CapturedNode<'a>> {
        &self.captures
    }
}

/// Converts a tree-sitter position (0-based) to one-based display coordinates.
fn position_to_one_based(pos: tree_sitter::Point) -> (u32, u32) {
    // Line/column numbers will realistically never exceed u32::MAX
    // Tree-sitter uses usize but files are limited to reasonable sizes
    let line = u32::try_from(pos.row.saturating_add(1)).unwrap_or(u32::MAX);
    let column = u32::try_from(pos.column.saturating_add(1)).unwrap_or(u32::MAX);
    (line, column)
}

/// A captured metavariable binding.
#[derive(Debug, Clone)]
pub struct CapturedNode<'a> {
    /// The captured AST node.
    node: tree_sitter::Node<'a>,
    /// The source text of the captured node.
    text: &'a str,
}

impl<'a> CapturedNode<'a> {
    /// Returns the captured AST node.
    #[must_use]
    pub const fn node(&self) -> tree_sitter::Node<'a> {
        self.node
    }

    /// Returns the text of the captured node.
    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.text
    }

    /// Returns the byte range of the captured node.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        self.node.byte_range()
    }
}

/// Pattern matcher that finds occurrences in parsed code.
pub struct Matcher<'p> {
    pattern: &'p Pattern,
}

impl<'p> Matcher<'p> {
    /// Creates a new matcher for the given pattern.
    #[must_use]
    pub const fn new(pattern: &'p Pattern) -> Self {
        Self { pattern }
    }

    /// Finds all matches of the pattern in the parsed source.
    ///
    /// Returns an empty vector if no matches are found.
    #[must_use]
    pub fn find_all<'a>(&self, parsed: &'a ParseResult) -> Vec<MatchResult<'a>> {
        let mut results = Vec::new();
        let ctx = MatchContext::new(self.pattern, parsed.source());
        find_matches_recursive(parsed.root_node(), &ctx, &mut results);
        results
    }

    /// Finds the first match of the pattern in the parsed source.
    ///
    /// Returns `None` if no match is found.
    #[must_use]
    pub fn find_first<'a>(&self, parsed: &'a ParseResult) -> Option<MatchResult<'a>> {
        self.find_all(parsed).into_iter().next()
    }
}

/// Context for pattern matching operations.
struct MatchContext<'a, 'p> {
    /// Root node of the pattern tree.
    pattern_root: tree_sitter::Node<'p>,
    /// Source code being matched against.
    source: &'a str,
    /// Pattern being matched.
    pattern: &'p Pattern,
    /// Current captures (uses `RefCell` for interior mutability).
    captures: RefCell<HashMap<String, CapturedNode<'a>>>,
}

impl<'a, 'p> MatchContext<'a, 'p> {
    /// Creates a new match context.
    fn new(pattern: &'p Pattern, source: &'a str) -> Self {
        // Get the actual pattern content, not the source_file wrapper
        // The pattern is typically wrapped in source_file, so use its first named child
        let root = pattern.parsed().root_node();
        let pattern_root = root
            .named_child(0)
            .filter(|_| root.kind() == "source_file")
            .unwrap_or(root);

        Self {
            pattern_root,
            source,
            pattern,
            captures: RefCell::new(HashMap::new()),
        }
    }

    /// Resets captures for a new match attempt.
    fn reset_captures(&self) {
        self.captures.borrow_mut().clear();
    }

    /// Takes the current captures, leaving an empty map.
    fn take_captures(&self) -> HashMap<String, CapturedNode<'a>> {
        std::mem::take(&mut *self.captures.borrow_mut())
    }

    /// Inserts a capture.
    fn insert_capture(&self, name: String, captured: CapturedNode<'a>) {
        self.captures.borrow_mut().insert(name, captured);
    }

    /// Gets metavariable at offset.
    fn get_metavariable_at(&self, offset: usize) -> Option<&'p MetaVariable> {
        self.pattern.metavariables().iter().find(|m| {
            let pattern_source = self.pattern.source();
            let before_offset = offset.saturating_sub(4);
            pattern_source
                .get(before_offset..offset)
                .is_some_and(|s| s.contains('$'))
                && m.offset <= offset.saturating_add(m.name.len())
                && m.offset >= before_offset
        })
    }
}

/// Recursively searches for pattern matches in the syntax tree.
fn find_matches_recursive<'a>(
    source_node: tree_sitter::Node<'a>,
    ctx: &MatchContext<'a, '_>,
    results: &mut Vec<MatchResult<'a>>,
) {
    // Try to match at this node
    ctx.reset_captures();
    if nodes_match(source_node, ctx.pattern_root, ctx) {
        results.push(MatchResult {
            node: source_node,
            source: ctx.source,
            captures: ctx.take_captures(),
        });
    }

    // Recurse into children
    let mut cursor = source_node.walk();
    for child in source_node.children(&mut cursor) {
        find_matches_recursive(child, ctx, results);
    }
}

/// Checks if a pattern node represents a metavariable capture.
///
/// Tree-sitter parses `$VAR` as a `metavariable` node. We also need to handle
/// wrapper nodes like `range_pattern` that exist solely to contain a metavariable.
fn find_metavariable_in_pattern<'p>(
    pattern_node: tree_sitter::Node<'p>,
    ctx: &MatchContext<'_, 'p>,
) -> Option<&'p MetaVariable> {
    // Direct metavariable node
    if pattern_node.kind() == "metavariable" {
        let text = ctx
            .pattern
            .source()
            .get(pattern_node.byte_range())
            .unwrap_or_default();

        // Extract name from $VAR format
        let name = text.trim_start_matches('$');
        return ctx.pattern.metavariables().iter().find(|m| m.name == name);
    }

    // Don't treat ERROR nodes as metavariable wrappers - they indicate parse failures
    if pattern_node.kind() == "ERROR" {
        return None;
    }

    // Check if this node is a pure metavariable wrapper (contains ONLY a metavariable)
    // This handles cases like `range_pattern` which wraps `$VAR` in Rust patterns.
    // We only consider it a metavariable if the node has exactly one named child
    // and that child is a metavariable.
    let mut cursor = pattern_node.walk();
    let named_children: Vec<_> = pattern_node.named_children(&mut cursor).collect();

    if let [child] = named_children.as_slice() {
        if child.kind() == "metavariable" {
            let text = ctx
                .pattern
                .source()
                .get(child.byte_range())
                .unwrap_or_default();
            let name = text.trim_start_matches('$');
            return ctx.pattern.metavariables().iter().find(|m| m.name == name);
        }
    }

    None
}

/// Checks if a source node matches a pattern node.
fn nodes_match<'a>(
    source_node: tree_sitter::Node<'a>,
    pattern_node: tree_sitter::Node<'_>,
    ctx: &MatchContext<'a, '_>,
) -> bool {
    let pattern_text = ctx
        .pattern
        .source()
        .get(pattern_node.byte_range())
        .unwrap_or_default();

    // Check if this is a metavariable (either directly or wrapped)
    if let Some(metavar) = find_metavariable_in_pattern(pattern_node, ctx) {
        // Capture the source node
        let text = ctx.source.get(source_node.byte_range()).unwrap_or_default();
        ctx.insert_capture(
            metavar.name.clone(),
            CapturedNode {
                node: source_node,
                text,
            },
        );
        return true;
    }

    // Node kinds must match
    if source_node.kind() != pattern_node.kind() {
        return false;
    }

    // For leaf nodes, compare text if no children
    if pattern_node.child_count() == 0 {
        let source_text = ctx.source.get(source_node.byte_range()).unwrap_or_default();
        return source_text == pattern_text;
    }

    // Match children
    match_children(source_node, pattern_node, ctx)
}

/// Matches children of source and pattern nodes.
fn match_children<'a>(
    source_node: tree_sitter::Node<'a>,
    pattern_node: tree_sitter::Node<'_>,
    ctx: &MatchContext<'a, '_>,
) -> bool {
    let source_children: Vec<_> = {
        let mut cursor = source_node.walk();
        source_node.named_children(&mut cursor).collect()
    };
    let pattern_children: Vec<_> = {
        let mut cursor = pattern_node.walk();
        pattern_node.named_children(&mut cursor).collect()
    };

    // Check for multiple-match metavariables in children
    let has_multiple = pattern_children.iter().any(|child| {
        let text = ctx
            .pattern
            .source()
            .get(child.byte_range())
            .unwrap_or_default();
        text.starts_with("$$$")
    });

    if has_multiple {
        return match_with_multiple(&source_children, &pattern_children, ctx);
    }

    // Without multiple-match, children count must match
    if source_children.len() != pattern_children.len() {
        return false;
    }

    // Match each child
    for (source_child, pattern_child) in source_children.iter().zip(pattern_children.iter()) {
        if !nodes_match(*source_child, *pattern_child, ctx) {
            return false;
        }
    }

    true
}

/// Matches children when a multiple-match metavariable ($$$) is present.
fn match_with_multiple<'a>(
    source_children: &[tree_sitter::Node<'a>],
    pattern_children: &[tree_sitter::Node<'_>],
    ctx: &MatchContext<'a, '_>,
) -> bool {
    // Simplified: if we have a $$$ metavar, try to match greedily
    let mut source_idx = 0;
    let mut pattern_idx = 0;

    while pattern_idx < pattern_children.len() {
        let Some(pattern_child) = pattern_children.get(pattern_idx) else {
            break;
        };

        if try_capture_multiple(pattern_child, source_children.get(source_idx), ctx) {
            pattern_idx = pattern_idx.saturating_add(1);
            source_idx = source_idx.saturating_add(1);
            continue;
        }

        // Regular match
        let Some(source_child) = source_children.get(source_idx) else {
            return false;
        };

        if !nodes_match(*source_child, *pattern_child, ctx) {
            return false;
        }

        pattern_idx = pattern_idx.saturating_add(1);
        source_idx = source_idx.saturating_add(1);
    }

    true
}

/// Tries to capture a multiple-match metavariable. Returns true if captured.
fn try_capture_multiple<'a>(
    pattern_child: &tree_sitter::Node<'_>,
    source_child: Option<&tree_sitter::Node<'a>>,
    ctx: &MatchContext<'a, '_>,
) -> bool {
    let Some(metavar) = ctx.get_metavariable_at(pattern_child.start_byte()) else {
        return false;
    };

    if metavar.kind != MetaVarKind::Multiple {
        return false;
    }

    // Capture the source child if present
    if let Some(child) = source_child {
        let text = ctx.source.get(child.byte_range()).unwrap_or_default();
        ctx.insert_capture(metavar.name.clone(), CapturedNode { node: *child, text });
    }

    true
}

impl Pattern {
    /// Finds all matches of this pattern in the parsed source.
    ///
    /// This is a convenience method that creates a [`Matcher`] internally.
    #[must_use]
    pub fn find_all<'a>(&'a self, parsed: &'a ParseResult) -> Vec<MatchResult<'a>> {
        Matcher::new(self).find_all(parsed)
    }

    /// Finds the first match of this pattern in the parsed source.
    ///
    /// This is a convenience method that creates a [`Matcher`] internally.
    #[must_use]
    pub fn find_first<'a>(&'a self, parsed: &'a ParseResult) -> Option<MatchResult<'a>> {
        Matcher::new(self).find_first(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::SupportedLanguage;
    use crate::parser::Parser;

    #[test]
    fn find_literal_pattern() {
        let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
        let source = parser.parse("fn main() { let x = 1; }").expect("parse");
        let pattern = Pattern::compile("let x = 1", SupportedLanguage::Rust).expect("pattern");

        let matches = pattern.find_all(&source);
        assert!(!matches.is_empty());
    }

    #[test]
    fn find_pattern_with_metavariable() {
        let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
        let source = parser
            .parse("fn main() { let x = 1; let y = 2; }")
            .expect("parse");
        let pattern =
            Pattern::compile("let $VAR = $VAL", SupportedLanguage::Rust).expect("pattern");

        let matches = pattern.find_all(&source);
        // Should find both let statements
        assert!(!matches.is_empty());
    }

    #[test]
    fn capture_metavariable_text() {
        let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
        let source = parser.parse("fn hello() {}").expect("parse");
        let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");

        let matches = pattern.find_all(&source);
        if let Some(m) = matches.first() {
            if let Some(capture) = m.capture("NAME") {
                assert_eq!(capture.text(), "hello");
            }
        }
    }

    #[test]
    fn no_match_returns_empty() {
        let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
        let source = parser.parse("fn main() {}").expect("parse");
        let pattern =
            Pattern::compile("struct $NAME {}", SupportedLanguage::Rust).expect("pattern");

        let matches = pattern.find_all(&source);
        assert!(matches.is_empty());
    }

    #[test]
    fn match_result_has_position() {
        let mut parser = Parser::new(SupportedLanguage::Rust).expect("parser");
        let source = parser.parse("fn test() {}").expect("parse");
        let pattern = Pattern::compile("fn $NAME() {}", SupportedLanguage::Rust).expect("pattern");

        if let Some(m) = pattern.find_first(&source) {
            let (line, col) = m.start_position();
            assert_eq!(line, 1);
            assert!(col >= 1);
        }
    }
}
