//! Parameter extraction helpers shared across supported languages.

use tree_sitter::Node;

use crate::ParamInfo;

use super::normalise_whitespace;

pub(super) fn parse_parameters(param_node: Node<'_>, source: &str) -> Vec<ParamInfo> {
    let mut cursor = param_node.walk();
    param_node
        .named_children(&mut cursor)
        .filter_map(|child| {
            let kind = child.kind();
            if matches!(
                kind,
                "self_parameter"
                    | "receiver"
                    | "list_splat_pattern"
                    | "dictionary_splat_pattern"
                    | "positional_separator"
                    | "keyword_separator"
            ) {
                return None;
            }

            let name = child
                .child_by_field_name("name")
                .or_else(|| child.child_by_field_name("pattern"))
                .or_else(|| {
                    let mut child_cursor = child.walk();
                    child.named_children(&mut child_cursor).next()
                })
                .or(Some(child))
                .and_then(|node| source.get(node.byte_range()))
                .map(normalise_whitespace)
                .unwrap_or_default();
            if name.is_empty() {
                return None;
            }

            let type_annotation = child
                .child_by_field_name("type")
                .or_else(|| child.child_by_field_name("annotation"))
                .and_then(|node| source.get(node.byte_range()))
                .map(normalise_whitespace)
                .unwrap_or_default();
            Some(ParamInfo {
                name,
                type_annotation,
            })
        })
        .collect()
}
