//! Matching context shared across recursive operations.

use crate::pattern::Pattern;

fn single_named_child(node: tree_sitter::Node<'_>) -> Option<tree_sitter::Node<'_>> {
    let mut cursor = node.walk();
    let mut named_children = node.named_children(&mut cursor);
    let first = named_children.next()?;
    named_children.next().is_none().then_some(first)
}

pub(super) struct MatchContext<'a, 'p> {
    pub(super) pattern_root: tree_sitter::Node<'p>,
    pub(super) source: &'a str,
    pub(super) pattern: &'p Pattern,
}

impl<'a, 'p> MatchContext<'a, 'p> {
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

    pub(super) fn pattern_text(&self, node: tree_sitter::Node<'_>) -> &'p str {
        self.pattern
            .parsed()
            .source()
            .get(node.byte_range())
            .unwrap_or_default()
    }
}
