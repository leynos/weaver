//! Python entity extraction rules.

use tree_sitter::Node;

use crate::CardSymbolKind;

use super::EntityCandidate;
use super::common::{
    CallableMetadata, callable_candidate, decorator_texts, name_text, python_docstring,
    simple_candidate,
};

#[derive(Clone, Copy)]
struct ClassMetadata<'a> {
    decorators: &'a [String],
    anchor: usize,
}

pub(super) fn collect(root: Node<'_>, source: &str) -> Vec<EntityCandidate> {
    let mut entities = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "function_definition" => entities.push(build_callable(
                child,
                source,
                CardSymbolKind::Function,
                None,
                Vec::new(),
            )),
            "class_definition" => push_class_entities(
                &mut entities,
                child,
                source,
                ClassMetadata {
                    decorators: &[],
                    anchor: child.start_byte(),
                },
            ),
            "decorated_definition" => push_decorated_entities(&mut entities, child, source),
            _ => {}
        }
    }
    entities
}

fn push_decorated_entities(entities: &mut Vec<EntityCandidate>, node: Node<'_>, source: &str) {
    let Some(definition) = node.child_by_field_name("definition") else {
        return;
    };
    let decorators = decorator_texts(node, source);
    match definition.kind() {
        "function_definition" => {
            entities.push(build_callable(
                definition,
                source,
                CardSymbolKind::Function,
                None,
                decorators,
            ));
        }
        "class_definition" => {
            push_class_entities(
                entities,
                definition,
                source,
                ClassMetadata {
                    decorators: &decorators,
                    anchor: node.start_byte(),
                },
            );
        }
        _ => {}
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "requested helper keeps the callable construction contract explicit"
)]
fn build_callable(
    node: Node<'_>,
    source: &str,
    kind: CardSymbolKind,
    container: Option<&str>,
    decorators: Vec<String>,
) -> EntityCandidate {
    callable_candidate(
        node,
        source,
        kind,
        CallableMetadata::new(
            container.map(str::to_owned),
            decorators,
            python_docstring(node, source),
        ),
    )
}

fn push_class_entities(
    entities: &mut Vec<EntityCandidate>,
    class_node: Node<'_>,
    source: &str,
    metadata: ClassMetadata<'_>,
) {
    let name = name_text(class_node, source);
    let mut class_candidate = simple_candidate(class_node, source, CardSymbolKind::Class, None);
    class_candidate.decorators = metadata.decorators.to_vec();
    class_candidate.attachment_anchor = Some(metadata.anchor);
    entities.push(class_candidate);
    entities.extend(class_methods(class_node, source, Some(name.as_str())));
}

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
        match child.kind() {
            "function_definition" => {
                methods.push(build_callable(
                    child,
                    source,
                    CardSymbolKind::Method,
                    container,
                    Vec::new(),
                ));
            }
            "decorated_definition" => {
                if let Some(method) = decorated_method(child, source, container) {
                    methods.push(method);
                }
            }
            _ => {}
        }
    }
    methods
}

fn decorated_method(
    node: Node<'_>,
    source: &str,
    container: Option<&str>,
) -> Option<EntityCandidate> {
    let definition = node.child_by_field_name("definition")?;
    (definition.kind() == "function_definition").then(|| {
        build_callable(
            definition,
            source,
            CardSymbolKind::Method,
            container,
            decorator_texts(node, source),
        )
    })
}
