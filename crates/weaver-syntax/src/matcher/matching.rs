//! Matching algorithms for the [`Matcher`] implementation.

use crate::matcher::capture::Captures;
use crate::matcher::context::MatchContext;
use crate::matcher::MatchResult;
use crate::metavariables::metavar_name_from_placeholder;
use crate::parser::ParseResult;
use crate::pattern::{MetaVarKind, MetaVariable, Pattern};

pub(super) fn find_all<'a>(pattern: &Pattern, parsed: &'a ParseResult) -> Vec<MatchResult<'a>> {
    let ctx = MatchContext::new(pattern, parsed.source());
    let mut results = Vec::new();
    find_matches_recursive(parsed.root_node(), &ctx, &mut results);
    results
}

pub(super) fn find_first<'a>(
    pattern: &Pattern,
    parsed: &'a ParseResult,
) -> Option<MatchResult<'a>> {
    let ctx = MatchContext::new(pattern, parsed.source());
    find_first_recursive(parsed.root_node(), &ctx)
}

fn find_matches_recursive<'a>(
    source_node: tree_sitter::Node<'a>,
    ctx: &MatchContext<'a, '_>,
    results: &mut Vec<MatchResult<'a>>,
) {
    let mut captures = Captures::default();
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

fn find_first_recursive<'a>(
    source_node: tree_sitter::Node<'a>,
    ctx: &MatchContext<'a, '_>,
) -> Option<MatchResult<'a>> {
    let mut captures = Captures::default();
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

    let mut cursor = pattern_node.walk();
    let children: Vec<_> = pattern_node.named_children(&mut cursor).collect();
    if let [child] = children.as_slice() {
        return find_metavariable_in_pattern(*child, ctx);
    }

    None
}

fn nodes_match<'a>(
    source_node: tree_sitter::Node<'a>,
    pattern_node: tree_sitter::Node<'_>,
    ctx: &MatchContext<'a, '_>,
    captures: &mut Captures<'a>,
) -> bool {
    if let Some(metavar) = find_metavariable_in_pattern(pattern_node, ctx) {
        return match metavar.kind {
            MetaVarKind::Single => captures.capture_single(&metavar.name, source_node, ctx.source),
            MetaVarKind::Multiple => {
                captures.capture_multiple(&metavar.name, &[source_node], ctx.source)
            }
        };
    }

    if source_node.kind() != pattern_node.kind() {
        return false;
    }

    let pattern_text = ctx.pattern_text(pattern_node);
    if pattern_node.child_count() == 0 {
        let source_text = ctx.source.get(source_node.byte_range()).unwrap_or_default();
        return source_text == pattern_text;
    }

    match_children(source_node, pattern_node, ctx, captures)
}

fn node_children<'a>(node: tree_sitter::Node<'a>) -> Vec<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

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
        return match_with_multiple(&source_children, &pattern_children, ctx, captures);
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

fn match_with_multiple<'a>(
    source_children: &[tree_sitter::Node<'a>],
    pattern_children: &[tree_sitter::Node<'_>],
    ctx: &MatchContext<'a, '_>,
    captures: &mut Captures<'a>,
) -> bool {
    fn match_seq<'a>(
        source_children: &[tree_sitter::Node<'a>],
        pattern_children: &[tree_sitter::Node<'_>],
        source_idx: usize,
        pattern_idx: usize,
        ctx: &MatchContext<'a, '_>,
        captures: &mut Captures<'a>,
    ) -> bool {
        if pattern_idx == pattern_children.len() {
            return source_idx == source_children.len();
        }

        let pattern_child = pattern_children[pattern_idx];
        if let Some(metavar) = find_metavariable_in_pattern(pattern_child, ctx)
            .filter(|metavar| metavar.kind == MetaVarKind::Multiple)
        {
            for k in source_idx..=source_children.len() {
                let mut trial = captures.clone();
                if !trial.capture_multiple(&metavar.name, &source_children[source_idx..k], ctx.source)
                {
                    continue;
                }
                if match_seq(
                    source_children,
                    pattern_children,
                    k,
                    pattern_idx.saturating_add(1),
                    ctx,
                    &mut trial,
                ) {
                    *captures = trial;
                    return true;
                }
            }
            return false;
        }

        let Some(source_child) = source_children.get(source_idx).copied() else {
            return false;
        };

        let mut trial = captures.clone();
        if !nodes_match(source_child, pattern_child, ctx, &mut trial) {
            return false;
        }

        if match_seq(
            source_children,
            pattern_children,
            source_idx.saturating_add(1),
            pattern_idx.saturating_add(1),
            ctx,
            &mut trial,
        ) {
            *captures = trial;
            return true;
        }

        false
    }

    match_seq(
        source_children,
        pattern_children,
        0,
        0,
        ctx,
        captures,
    )
}
