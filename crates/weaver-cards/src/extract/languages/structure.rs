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

        if let Some(local) = local_info(node, self.source) {
            self.locals.push(local);
        }
        if let Some(branch) = branch_info(node) {
            self.branches.push(branch);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(child);
        }
    }
}

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

pub(super) fn is_nested_entity(kind: &str, root_kind: &str) -> bool {
    kind == "function_definition"
        || kind == "lambda"
        || (kind == "function_item" && root_kind != "trait_item")
        || kind == "function_declaration"
        || kind == "method_definition"
}

pub(super) fn local_info(node: Node<'_>, source: &str) -> Option<LocalInfo> {
    match node.kind() {
        "let_declaration" => Some(LocalInfo {
            name: binding_name(node, source),
            kind: String::from("variable"),
            decl_line: to_u32(node.start_position().row),
        }),
        "assignment" => Some(LocalInfo {
            name: assignment_name(node, source),
            kind: String::from("variable"),
            decl_line: to_u32(node.start_position().row),
        }),
        "lexical_declaration" => Some(LocalInfo {
            name: lexical_name(node, source),
            kind: String::from("variable"),
            decl_line: to_u32(node.start_position().row),
        }),
        _ => None,
    }
}

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

pub(super) fn binding_name(node: Node<'_>, source: &str) -> String {
    node.child_by_field_name("pattern")
        .and_then(|pattern| source.get(pattern.byte_range()))
        .map_or_else(
            || normalise_whitespace(source.get(node.byte_range()).unwrap_or_default()),
            normalise_whitespace,
        )
}

pub(super) fn assignment_name(node: Node<'_>, source: &str) -> String {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .next()
        .and_then(|target| source.get(target.byte_range()))
        .map_or_else(|| String::from("assignment"), normalise_whitespace)
}

pub(super) fn lexical_name(node: Node<'_>, source: &str) -> String {
    let raw = source.get(node.byte_range()).unwrap_or_default().trim();
    let trimmed = raw
        .trim_start_matches("const ")
        .trim_start_matches("let ")
        .trim_start_matches("var ");
    trimmed
        .split_once('=')
        .map_or(trimmed, |(name, _)| name)
        .split_once(':')
        .map_or(trimmed, |(name, _)| name)
        .trim()
        .to_owned()
}
