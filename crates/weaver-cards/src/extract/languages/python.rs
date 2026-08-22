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

/// Metadata threaded through class-entity construction so that a decorated
/// class and a plain one can share the same builder logic.
#[derive(Clone, Copy)]
struct ClassMetadata<'a> {
    /// Raw decorator texts to attach to the class candidate; empty for a
    /// class with no `decorated_definition` wrapper.
    decorators: &'a [String],
    /// Byte offset used as the class candidate's attachment anchor. For a
    /// decorated class this is the `decorated_definition` node's start, not
    /// the `class_definition` node's, so attachments (e.g. leading comments)
    /// bind to the outermost span including the decorators.
    anchor: usize,
}

/// Parameters threaded through [`build_callable`] to describe how a
/// function or method entity should be classified and labelled.
#[derive(Clone)]
struct CallableSpec<'a> {
    /// Whether the callable should be recorded as a function or a method.
    kind: CardSymbolKind,
    /// Owning class name for a method, or `None` for a module-level function.
    container: Option<&'a str>,
    /// Raw decorator texts collected from any enclosing `decorated_definition`.
    decorators: Vec<String>,
}

/// Collects top-level Python entities from `root` using slices from `source`.
///
/// Returns one [`EntityCandidate`] per top-level function or class
/// (including their nested methods), covering both plain definitions and
/// definitions wrapped in a `decorated_definition` node.
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

/// Handles a top-level `decorated_definition` node, pushing the resulting
/// function or class entity (and, for a class, its methods) onto `entities`.
///
/// The decorator texts are read once from `node` and passed down so both the
/// function and class branches attribute them to the correct candidate.
/// Does nothing when `node` has no `definition` field or the definition kind
/// is neither a function nor a class.
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

/// Builds an [`EntityCandidate`] for a `function_definition` node, attaching
/// its docstring (if any) alongside the container and decorator metadata
/// supplied in `spec`.
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

/// Builds a class [`EntityCandidate`] plus its nested method candidates and
/// appends them all to `entities`.
///
/// `metadata` supplies the decorators and attachment anchor so a decorated
/// class reuses the same construction path as a plain one.
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

/// Collects method entities from a class body, covering both plain
/// `function_definition` methods and methods wrapped in a
/// `decorated_definition`. Returns an empty vector when `class_node` has no
/// body.
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

/// Builds a method [`EntityCandidate`] from a `decorated_definition` node
/// found inside a class body, or `None` when the wrapped definition is not a
/// `function_definition` (e.g. a decorated nested class).
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
