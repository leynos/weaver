//! Matching context shared across recursive operations.

use crate::pattern::Pattern;

/// Returns `node`'s only named child, or `None` if it has zero or more than
/// one — used to unwrap a synthetic single-statement wrapper down to the
/// node the pattern actually intends to match.
fn single_named_child(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut cursor = node.walk();
    let mut named_children = node.named_children(&mut cursor);
    let first = named_children.next()?;
    named_children.next().is_none().then_some(first)
}

/// State threaded through a single matching pass: the pattern's effective
/// root node (unwrapped from any surrounding function-wrapper syntax) and
/// the source text being matched against.
pub(super) struct MatchContext<'a, 'p> {
    /// The pattern AST node that matching actually starts from, after
    /// unwrapping any wrapper function or single-statement block introduced
    /// so the pattern could be parsed as valid syntax.
    pub(super) pattern_root: tree_sitter::Node<'p>,
    /// The candidate source text being matched.
    pub(super) source: &'a str,
    /// The pattern being matched, kept for access to its parsed tree and text.
    pub(super) pattern: &'p Pattern,
}

impl<'a, 'p> MatchContext<'a, 'p> {
    /// Builds a match context, unwrapping `pattern`'s parse tree down to the
    /// node that should be compared against candidate source nodes.
    pub(super) fn new(pattern: &'p Pattern, source: &'a str) -> Self {
        let root = pattern.parsed().root_node();
        let pattern_root = if pattern.wrapped_in_function() {
            let wrapper = root.named_child(0).unwrap_or(root);

            let wrapper_body = wrapper.child_by_field_name("body").or_else(|| {
                let mut cursor = wrapper.walk();
                wrapper
                    .named_children(&mut cursor)
                    .find(|child| child.kind().contains("block"))
            });

            wrapper_body.map_or(wrapper, |body_node| {
                single_named_child(body_node).unwrap_or(body_node)
            })
        } else {
            single_named_child(root).unwrap_or(root)
        };

        Self {
            pattern_root,
            source,
            pattern,
        }
    }

    /// Returns the pattern source text spanned by `node`, or `""` if the
    /// range is out of bounds for the pattern's source.
    pub(super) fn pattern_text(&self, node: tree_sitter::Node<'_>) -> &'p str {
        self.pattern
            .parsed()
            .source()
            .get(node.byte_range())
            .unwrap_or_default()
    }
}
