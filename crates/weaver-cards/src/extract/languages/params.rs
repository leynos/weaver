//! Parameter extraction helpers shared across supported languages.

use tree_sitter::Node;

use crate::ParamInfo;

use super::normalise_whitespace;

/// Extracts parameter names and type annotations from a parameters node.
///
/// `param_node` is the Tree-sitter parameters list to inspect and `source`
/// provides the backing text used for node slices. Returns one [`ParamInfo`]
/// per non-empty parameter name, skipping receivers such as `self_parameter`
/// and `receiver` plus separator nodes. Use this for callable signature
/// extraction rather than raw text splitting.
pub(super) fn parse_parameters(param_node: Node<'_>, source: &str) -> Vec<ParamInfo> {
    let mut cursor = param_node.walk();
    param_node
        .named_children(&mut cursor)
        .filter_map(|child| {
            let kind = child.kind();
            if matches!(
                kind,
                "self_parameter" | "receiver" | "positional_separator" | "keyword_separator"
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
