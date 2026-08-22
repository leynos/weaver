//! TypeScript entity extraction rules.

use tree_sitter::Node;

use super::{
    EntityCandidate,
    common::{CallableMetadata, callable_candidate, decorator_texts, name_text, simple_candidate},
};
use crate::CardSymbolKind;

/// Collects top-level TypeScript entities from `root` using slices from
/// `source`.
///
/// Returns one [`EntityCandidate`] per supported top-level declaration
/// (functions, classes, interfaces, type aliases), plus methods nested under
/// each class body.
pub(super) fn collect(root: Node<'_>, source: &str) -> Vec<EntityCandidate> {
    let mut entities = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "function_declaration" => entities.push(callable_candidate(
                child,
                source,
                CardSymbolKind::Function,
                CallableMetadata::new(None, decorator_texts(child, source), None),
            )),
            "class_declaration" => {
                let name = name_text(child, source);
                let mut class_candidate =
                    simple_candidate(child, source, CardSymbolKind::Class, None);
                class_candidate.decorators = decorator_texts(child, source);
                entities.push(class_candidate);
                entities.extend(class_methods(child, source, Some(name.as_str())));
            }
            "interface_declaration" => {
                entities.push(simple_candidate(
                    child,
                    source,
                    CardSymbolKind::Interface,
                    None,
                ));
            }
            "type_alias_declaration" => {
                entities.push(simple_candidate(child, source, CardSymbolKind::Type, None));
            }
            _ => {}
        }
    }
    entities
}

/// Collects `method_definition` entities from a class body, tagging each
/// with `container` as its owning class name. Returns an empty vector when
/// `class_node` has no body.
fn class_methods(
    class_node: Node<'_>,
    source: &str,
    container: Option<&str>,
) -> Vec<EntityCandidate> {
    let Some(body) = class_node.child_by_field_name("body") else {
        return Vec::new();
    };

    let mut methods = Vec::new();
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        if child.kind() == "method_definition" {
            methods.push(callable_candidate(
                child,
                source,
                CardSymbolKind::Method,
                CallableMetadata::new(
                    container.map(str::to_owned),
                    decorator_texts(child, source),
                    None,
                ),
            ));
        }
    }
    methods
}
