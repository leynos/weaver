//! Python entity extraction rules.

use tree_sitter::Node;

use super::{
    EntityCandidate,
    common::{
        CallableMetadata,
        callable_candidate,
        decorator_texts,
        name_text,
        python_docstring,
        simple_candidate,
    },
};
use crate::CardSymbolKind;

#[derive(Clone, Copy)]
struct ClassMetadata<'a> {
    decorators: &'a [String],
    anchor: usize,
}

#[derive(Clone)]
struct CallableSpec<'a> {
    kind: CardSymbolKind,
    container: Option<&'a str>,
    decorators: Vec<String>,
}

pub(super) fn collect(root: Node<'_>, source: &str) -> Vec<EntityCandidate> {
    let mut entities = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "function_definition" => entities.push(build_callable(
                child,
                source,
                CallableSpec {
                    kind: CardSymbolKind::Function,
                    container: None,
                    decorators: Vec::new(),
                },
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
            let mut candidate = build_callable(
                definition,
                source,
                CallableSpec {
                    kind: CardSymbolKind::Function,
                    container: None,
                    decorators,
                },
            );
            candidate.attachment_anchor = Some(node.start_byte());
            entities.push(candidate);
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

fn build_callable(node: Node<'_>, source: &str, spec: CallableSpec<'_>) -> EntityCandidate {
    callable_candidate(
        node,
        source,
        spec.kind,
        CallableMetadata::new(
            spec.container.map(str::to_owned),
            spec.decorators,
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
                    CallableSpec {
                        kind: CardSymbolKind::Method,
                        container,
                        decorators: Vec::new(),
                    },
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
        let mut candidate = build_callable(
            definition,
            source,
            CallableSpec {
                kind: CardSymbolKind::Method,
                container,
                decorators: decorator_texts(node, source),
            },
        );
        candidate.attachment_anchor = Some(node.start_byte());
        candidate
    })
}
