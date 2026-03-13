//! Structure extraction helpers for locals and control-flow branches.

use tree_sitter::Node;

use crate::{BranchInfo, LocalInfo};

use super::{normalise_whitespace, to_u32};

pub(super) struct StructureCollector<'a> {
    source: &'a str,
    body_root_id: usize,
    root_kind: &'a str,
    locals: Vec<LocalInfo>,
    branches: Vec<BranchInfo>,
}

impl<'a> StructureCollector<'a> {
    pub(super) fn new(source: &'a str, body_root: Node<'_>, root_kind: &'a str) -> Self {
        Self {
            source,
            body_root_id: body_root.id(),
            root_kind,
            locals: Vec::new(),
            branches: Vec::new(),
        }
    }

    pub(super) fn finish(self) -> (Vec<LocalInfo>, Vec<BranchInfo>) {
        (self.locals, self.branches)
    }

    pub(super) fn visit(&mut self, node: Node<'_>) {
        if node.id() != self.body_root_id && is_nested_entity(node.kind(), self.root_kind) {
            return;
        }

        self.locals.extend(local_info(node, self.source));
        if let Some(branch) = branch_info(node) {
            self.branches.push(branch);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(child);
        }
    }
}

/// Collects local bindings and branch markers for a callable body subtree.
///
/// `root` is the callable node that owns the body, `body` is the optional
/// body node to traverse, and `source` provides text slices for extracted
/// names. Returns `(locals, branches)` and yields empty vectors when no body is
/// present.
pub(super) fn collect_structure(
    root: Node<'_>,
    body: Option<Node<'_>>,
    source: &str,
) -> (Vec<LocalInfo>, Vec<BranchInfo>) {
    let Some(body_node) = body else {
        return (Vec::new(), Vec::new());
    };
    let mut collector = StructureCollector::new(source, body_node, root.kind());
    collector.visit(body_node);
    collector.finish()
}

/// Reports whether a node kind should stop local traversal as a nested entity.
///
/// `kind` is the candidate nested node and `root_kind` is the surrounding
/// callable or container kind, used for Rust trait-item exceptions.
pub(super) fn is_nested_entity(kind: &str, root_kind: &str) -> bool {
    const ALWAYS_NESTED: &[&str] = &[
        "function_definition",
        "lambda",
        "function_declaration",
        "method_definition",
        "class_definition",
        "class_declaration",
        "interface_declaration",
        "type_alias_declaration",
        "struct_item",
        "enum_item",
        "type_item",
        "trait_item",
    ];
    if kind == "function_item" {
        return root_kind != "trait_item";
    }
    ALWAYS_NESTED.contains(&kind)
}

/// Extracts local variable declarations represented by the given node.
///
/// `node` is a declaration or assignment candidate and `source` provides text
/// for bound-name extraction. Returns one [`LocalInfo`] per discovered binding,
/// or an empty vector when the node does not declare locals.
pub(super) fn local_info(node: Node<'_>, source: &str) -> Vec<LocalInfo> {
    let names = match node.kind() {
        "let_declaration" => {
            let names = binding_names(node, source);
            if names.is_empty() {
                vec![binding_name(node, source)]
            } else {
                names
            }
        }
        "assignment" => {
            let names = assignment_names(node, source);
            if names.is_empty() {
                vec![assignment_name(node, source)]
            } else {
                names
            }
        }
        "lexical_declaration" => {
            let names = lexical_names(node, source);
            if names.is_empty() {
                vec![lexical_name(node, source)]
            } else {
                names
            }
        }
        _ => return Vec::new(),
    };

    names
        .into_iter()
        .map(|name| LocalInfo {
            name,
            kind: String::from("variable"),
            decl_line: to_u32(node.start_position().row),
        })
        .collect()
}

/// Maps branch-like syntax nodes to the corresponding [`BranchInfo`].
///
/// Returns `None` for non-branch nodes and otherwise records the normalized
/// branch kind plus the node's starting line.
pub(super) fn branch_info(node: Node<'_>) -> Option<BranchInfo> {
    let kind = match node.kind() {
        "if_expression" | "if_statement" => "if",
        "for_expression" | "for_statement" | "for_in_statement" | "for_of_statement" => "for",
        "while_expression" | "while_statement" => "while",
        "match_expression" | "match_statement" => "match",
        "switch_statement" => "switch",
        _ => return None,
    };
    Some(BranchInfo {
        kind: String::from(kind),
        line: to_u32(node.start_position().row),
    })
}

/// Returns the first binding name extracted from a Rust-style let declaration.
///
/// `node` is expected to expose a `pattern` field. Falls back to the raw node
/// text when no more specific binding name can be derived.
pub(super) fn binding_name(node: Node<'_>, source: &str) -> String {
    binding_names(node, source)
        .into_iter()
        .next()
        .unwrap_or_else(|| normalise_whitespace(source.get(node.byte_range()).unwrap_or_default()))
}

/// Returns the first assignment target name extracted from a binding node.
///
/// `node` is expected to expose a `left` field. Returns `"assignment"` when no
/// explicit target name can be resolved.
pub(super) fn assignment_name(node: Node<'_>, source: &str) -> String {
    assignment_names(node, source)
        .into_iter()
        .next()
        .unwrap_or_else(|| String::from("assignment"))
}

/// Returns the first lexical declaration name extracted from a declaration.
///
/// `node` is usually a JavaScript or TypeScript lexical declaration. Falls
/// back to normalized node text when bound-name traversal finds nothing more
/// specific.
pub(super) fn lexical_name(node: Node<'_>, source: &str) -> String {
    lexical_names(node, source)
        .into_iter()
        .next()
        .unwrap_or_else(|| normalise_whitespace(source.get(node.byte_range()).unwrap_or_default()))
}

fn binding_names(node: Node<'_>, source: &str) -> Vec<String> {
    node.child_by_field_name("pattern")
        .map_or_else(Vec::new, |pattern| bound_names(pattern, source))
}

fn assignment_names(node: Node<'_>, source: &str) -> Vec<String> {
    node.child_by_field_name("left")
        .map_or_else(Vec::new, |target| bound_names(target, source))
}

fn lexical_names(node: Node<'_>, source: &str) -> Vec<String> {
    let mut cursor = node.walk();
    let names: Vec<String> = node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "variable_declarator")
        .flat_map(|declarator| {
            declarator
                .child_by_field_name("name")
                .map_or_else(Vec::new, |name| bound_names(name, source))
        })
        .collect();
    if names.is_empty() {
        let mut fallback_cursor = node.walk();
        node.named_children(&mut fallback_cursor)
            .flat_map(|child| bound_names(child, source))
            .collect()
    } else {
        names
    }
}

fn bound_names(node: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    collect_bound_names(node, source, &mut names);
    names
}

fn push_identifier_name(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    let name = normalise_whitespace(source.get(node.byte_range()).unwrap_or_default());
    if !name.is_empty() {
        names.push(name);
    }
}

fn find_named_field_child<'a>(node: Node<'a>, fields: &[&str]) -> Option<Node<'a>> {
    fields.iter().find_map(|f| node.child_by_field_name(f))
}

fn collect_bound_names(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    match node.kind() {
        "identifier"
        | "property_identifier"
        | "shorthand_property_identifier"
        | "shorthand_property_identifier_pattern" => {
            push_identifier_name(node, source, names);
            return;
        }
        "member_expression" | "field_expression" | "subscript_expression" => return,
        _ => {}
    }

    if let Some(child) = find_named_field_child(node, &["name", "pattern", "left", "value"]) {
        collect_bound_names(child, source, names);
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_bound_names(child, source, names);
    }
}
