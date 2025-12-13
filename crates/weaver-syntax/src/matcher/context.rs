//! Matching context shared across recursive operations.

use crate::pattern::Pattern;

pub(super) struct MatchContext<'a, 'p> {
    pub(super) pattern_root: tree_sitter::Node<'p>,
    pub(super) source: &'a str,
    pub(super) pattern: &'p Pattern,
}

impl<'a, 'p> MatchContext<'a, 'p> {
    pub(super) fn new(pattern: &'p Pattern, source: &'a str) -> Self {
        let root = pattern.parsed().root_node();
        let pattern_root = if pattern.wrapped_in_function() {
            let wrapper = root
                .named_child(0)
                .filter(|_| root.kind() == "source_file")
                .unwrap_or(root);

            let body = wrapper.child_by_field_name("body").or_else(|| {
                let mut cursor = wrapper.walk();
                wrapper
                    .named_children(&mut cursor)
                    .find(|child| child.kind().contains("block"))
            });

            if let Some(body) = body {
                let mut cursor = body.walk();
                let named_children: Vec<_> = body.named_children(&mut cursor).collect();
                if let [child] = named_children.as_slice() {
                    *child
                } else {
                    body
                }
            } else {
                wrapper
            }
        } else {
            root.named_child(0)
                .filter(|_| root.kind() == "source_file")
                .unwrap_or(root)
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
