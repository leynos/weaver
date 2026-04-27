//! Rust entity extraction rules.

use tree_sitter::Node;

use super::{
    EntityCandidate,
    common::{
        CallableMetadata,
        callable_candidate,
        extract_rust_docstring,
        impl_container_name,
        name_text,
        simple_candidate,
    },
};
use crate::CardSymbolKind;

/// Collects top-level Rust entities from `root` using slices from `source`.
///
/// `root` is expected to be the parsed file root for the current source text.
/// Returns one [`EntityCandidate`] per supported top-level item, plus methods
/// nested under trait and impl items.
pub(super) fn collect(root: Node<'_>, source: &str) -> Vec<EntityCandidate> {
    let mut entities = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "function_item" => entities.push(callable_candidate(
                child,
                source,
                CardSymbolKind::Function,
                CallableMetadata::new(None, Vec::new(), extract_rust_docstring(child)),
            )),
            "struct_item" | "enum_item" | "type_item" | "union_item" => {
                entities.push(simple_candidate(child, source, CardSymbolKind::Type, None));
            }
            "trait_item" => {
                let name = name_text(child, source);
                entities.push(simple_candidate(
                    child,
                    source,
                    CardSymbolKind::Interface,
                    None,
                ));
                entities.extend(impl_like_methods(child, source, Some(name.as_str())));
            }
            "mod_item" => entities.push(simple_candidate(
                child,
                source,
                CardSymbolKind::Module,
                None,
            )),
            "impl_item" => entities.extend(impl_like_methods(
                child,
                source,
                impl_container_name(child, source).as_deref(),
            )),
            _ => {}
        }
    }
    entities
}

fn impl_like_methods(
    node: Node<'_>,
    source: &str,
    container: Option<&str>,
) -> Vec<EntityCandidate> {
    let Some(body) = node.child_by_field_name("body") else {
        return Vec::new();
    };

    let mut methods = Vec::new();
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        if child.kind() == "function_item" {
            methods.push(callable_candidate(
                child,
                source,
                CardSymbolKind::Method,
                CallableMetadata::new(
                    container.map(str::to_owned),
                    Vec::new(),
                    extract_rust_docstring(child),
                ),
            ));
        }
    }
    methods
}
