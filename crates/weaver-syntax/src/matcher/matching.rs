//! Matching algorithms for the [`Matcher`] implementation.

use crate::matcher::MatchResult;
use crate::matcher::capture::Captures;
use crate::matcher::context::MatchContext;
use crate::metavariables::metavar_name_from_placeholder;
use crate::parser::ParseResult;
use crate::pattern::{MetaVarKind, MetaVariable, Pattern};

/// Returns true if the node kind represents a block construct that should not
/// be traversed when searching for metavariables.
fn is_block_node_kind(kind: &str) -> bool {
    matches!(kind, "block" | "statement_block" | "compound_statement")
}

/// Finds all matches of `pattern` in `parsed` via depth-first traversal.
///
/// Returns matches in traversal order (pre-order) and borrows from `parsed`.
pub(super) fn find_all<'a>(pattern: &Pattern, parsed: &'a ParseResult) -> Vec<MatchResult<'a>> {
    let ctx = MatchContext::new(pattern, parsed.source());
    let mut results = Vec::new();
    find_matches_recursive(parsed.root_node(), &ctx, &mut results);
    results
}

/// Finds the first match of `pattern` in `parsed` via depth-first traversal.
///
/// Returns the earliest match in traversal order (pre-order) and borrows from
/// `parsed`.
pub(super) fn find_first<'a>(
    pattern: &Pattern,
    parsed: &'a ParseResult,
) -> Option<MatchResult<'a>> {
    let ctx = MatchContext::new(pattern, parsed.source());
    find_first_recursive(parsed.root_node(), &ctx)
}

/// Recursively traverses the source AST in pre-order, collecting all matches
/// of the pattern. Creates a fresh capture state for each candidate node.
fn find_matches_recursive<'a>(
    source_node: tree_sitter::Node<'a>,
    ctx: &MatchContext<'a, '_>,
    results: &mut Vec<MatchResult<'a>>,
) {
    let mut captures = Captures::new(ctx.source);
    if nodes_match(source_node, ctx.pattern_root, ctx, &mut captures) {
        results.push(MatchResult {
            node: source_node,
            source: ctx.source,
            captures: captures.into_inner(),
        });
    }

    let mut cursor = source_node.walk();
    for child in source_node.children(&mut cursor) {
        find_matches_recursive(child, ctx, results);
    }
}

/// Recursively traverses the source AST in pre-order, returning the first match
/// of the pattern. Creates a fresh capture state for each candidate node.
fn find_first_recursive<'a>(
    source_node: tree_sitter::Node<'a>,
    ctx: &MatchContext<'a, '_>,
) -> Option<MatchResult<'a>> {
    let mut captures = Captures::new(ctx.source);
    if nodes_match(source_node, ctx.pattern_root, ctx, &mut captures) {
        return Some(MatchResult {
            node: source_node,
            source: ctx.source,
            captures: captures.into_inner(),
        });
    }

    let mut cursor = source_node.walk();
    for child in source_node.children(&mut cursor) {
        if let Some(found) = find_first_recursive(child, ctx) {
            return Some(found);
        }
    }

    None
}

/// Extracts a metavariable reference from a pattern node by checking placeholder
/// text, recursing through single-child wrapper nodes, and skipping ERROR/block
/// nodes.
fn find_metavariable_in_pattern<'p>(
    pattern_node: tree_sitter::Node<'p>,
    ctx: &MatchContext<'_, 'p>,
) -> Option<&'p MetaVariable> {
    let text = ctx.pattern_text(pattern_node);
    if let Some(name) = metavar_name_from_placeholder(text) {
        return ctx.pattern.metavariables().iter().find(|m| m.name == name);
    }

    if pattern_node.kind() == "ERROR" {
        return None;
    }

    if is_block_node_kind(pattern_node.kind()) {
        return None;
    }

    let mut cursor = pattern_node.walk();
    let mut children = pattern_node.named_children(&mut cursor);
    let child = children.next()?;
    if children.next().is_none() {
        return find_metavariable_in_pattern(child, ctx);
    }

    None
}

/// Checks whether `source_node` matches `pattern_node`, handling metavariables,
/// kind comparison, leaf text comparison, and delegating to child matching.
/// Updates `captures` if the match succeeds.
fn nodes_match<'a>(
    source_node: tree_sitter::Node<'a>,
    pattern_node: tree_sitter::Node<'_>,
    ctx: &MatchContext<'a, '_>,
    captures: &mut Captures<'a>,
) -> bool {
    if let Some(metavar) = find_metavariable_in_pattern(pattern_node, ctx) {
        return match metavar.kind {
            MetaVarKind::Single => captures.capture_single(&metavar.name, source_node),
            MetaVarKind::Multiple => {
                captures.capture_multiple(&metavar.name, &[source_node], source_node.start_byte())
            }
        };
    }

    if source_node.kind() != pattern_node.kind() {
        return false;
    }

    let pattern_text = ctx.pattern_text(pattern_node);
    if pattern_node.child_count() == 0 {
        let range = source_node.byte_range();
        let Some(source_text) = ctx.source.get(range.clone()) else {
            debug_assert!(
                false,
                "tree-sitter node byte range {:?} is not valid for source length {}",
                range,
                ctx.source.len()
            );
            return false;
        };
        return source_text == pattern_text;
    }

    match_children(source_node, pattern_node, ctx, captures)
}

/// Collects all children of `node` into a Vec.
/// Used by `match_children` and `SequenceMatcher` for backtracking over child
/// sequences.
fn node_children(node: tree_sitter::Node<'_>) -> Vec<tree_sitter::Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

/// Matches children of `source_node` against children of `pattern_node`.
/// If the pattern contains Multiple metavariables, delegates to
/// `SequenceMatcher` for backtracking; otherwise performs pairwise matching.
fn match_children<'a>(
    source_node: tree_sitter::Node<'a>,
    pattern_node: tree_sitter::Node<'_>,
    ctx: &MatchContext<'a, '_>,
    captures: &mut Captures<'a>,
) -> bool {
    let source_children = node_children(source_node);
    let pattern_children = node_children(pattern_node);

    let has_multiple = pattern_children.iter().any(|child| {
        find_metavariable_in_pattern(*child, ctx)
            .is_some_and(|metavar| metavar.kind == MetaVarKind::Multiple)
    });

    if has_multiple {
        return SequenceMatcher {
            source_parent: source_node,
            source_children: &source_children,
            pattern_children: &pattern_children,
            ctx,
        }
        .matches(0, 0, captures);
    }

    if source_children.len() != pattern_children.len() {
        return false;
    }

    for (source_child, pattern_child) in source_children.iter().zip(pattern_children.iter()) {
        if !nodes_match(*source_child, *pattern_child, ctx, captures) {
            return false;
        }
    }

    true
}

/// Implements backtracking-based matching for child sequences containing
/// Multiple metavariables (`$$$VAR`), trying all possible bindings to find a
/// valid match.
struct SequenceMatcher<'a, 'p, 'c> {
    source_parent: tree_sitter::Node<'a>,
    source_children: &'c [tree_sitter::Node<'a>],
    pattern_children: &'c [tree_sitter::Node<'p>],
    ctx: &'c MatchContext<'a, 'p>,
}

/// Tracks current positions in source and pattern child sequences during
/// backtracking.
#[derive(Clone, Copy)]
struct MatchIndices {
    source_idx: usize,
    pattern_idx: usize,
}

impl<'a, 'p> SequenceMatcher<'a, 'p, '_> {
    /// Computes the byte position anchor for empty Multiple metavariable
    /// captures.
    ///
    /// Returns the start byte of the next source child, the end byte of the
    /// last source child, or the parent's start byte if no children exist.
    fn empty_anchor_byte(&self, source_idx: usize) -> usize {
        if let Some(next) = self.source_children.get(source_idx) {
            return next.start_byte();
        }

        if let Some(last) = self.source_children.last() {
            return last.end_byte();
        }

        self.source_parent.start_byte()
    }

    /// Recursively matches child sequences, dispatching to `matches_multiple`
    /// or `matches_single`.
    fn matches(&self, source_idx: usize, pattern_idx: usize, captures: &mut Captures<'a>) -> bool {
        if pattern_idx == self.pattern_children.len() {
            return source_idx == self.source_children.len();
        }

        let Some(pattern_child) = self.pattern_children.get(pattern_idx).copied() else {
            return false;
        };

        if let Some(metavar) = find_metavariable_in_pattern(pattern_child, self.ctx)
            .filter(|metavar| metavar.kind == MetaVarKind::Multiple)
        {
            return self.matches_multiple(
                MatchIndices {
                    source_idx,
                    pattern_idx,
                },
                metavar,
                captures,
            );
        }

        self.matches_single(
            MatchIndices {
                source_idx,
                pattern_idx,
            },
            pattern_child,
            captures,
        )
    }

    /// Tries all possible capture ranges for a Multiple metavariable via
    /// backtracking.
    fn matches_multiple(
        &self,
        indices: MatchIndices,
        metavar: &MetaVariable,
        captures: &mut Captures<'a>,
    ) -> bool {
        let next_pattern_idx = indices.pattern_idx + 1;
        let empty_anchor_byte = self.empty_anchor_byte(indices.source_idx);
        for k in indices.source_idx..=self.source_children.len() {
            let Some(candidate) = self.source_children.get(indices.source_idx..k) else {
                continue;
            };

            let mut trial = captures.clone();
            if !trial.capture_multiple(&metavar.name, candidate, empty_anchor_byte) {
                continue;
            }

            if self.matches(k, next_pattern_idx, &mut trial) {
                *captures = trial;
                return true;
            }
        }

        false
    }

    /// Matches a single pattern child against a single source child.
    ///
    /// Clones captures to preserve state in case the subsequent sequence fails
    /// to match.
    fn matches_single(
        &self,
        indices: MatchIndices,
        pattern_child: tree_sitter::Node<'p>,
        captures: &mut Captures<'a>,
    ) -> bool {
        let Some(source_child) = self.source_children.get(indices.source_idx).copied() else {
            return false;
        };

        let mut trial = captures.clone();
        if !nodes_match(source_child, pattern_child, self.ctx, &mut trial) {
            return false;
        }

        if self.matches(indices.source_idx + 1, indices.pattern_idx + 1, &mut trial) {
            *captures = trial;
            return true;
        }

        false
    }
}

// `match_with_multiple` is implemented by `SequenceMatcher::matches` to keep
// the matching logic local to the sequence matcher.
