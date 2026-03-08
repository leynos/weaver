//! Shared Tree-sitter extraction helpers across supported languages.

use tree_sitter::Node;
use weaver_syntax::SupportedLanguage;

use crate::{BranchInfo, CardSymbolKind, LocalInfo, ParamInfo, SourcePosition, SourceRange};

use super::super::{EntityCandidate, ImportBlock};

#[derive(Debug, Clone)]
pub(super) struct CallableMetadata {
    pub(super) container: Option<String>,
    pub(super) decorators: Vec<String>,
    pub(super) docstring: Option<String>,
}

impl CallableMetadata {
    pub(super) const fn new(
        container: Option<String>,
        decorators: Vec<String>,
        docstring: Option<String>,
    ) -> Self {
        Self {
            container,
            decorators,
            docstring,
        }
    }
}

struct StructureCollector<'a> {
    source: &'a str,
    body_root_id: usize,
    root_kind: &'a str,
    locals: Vec<LocalInfo>,
    branches: Vec<BranchInfo>,
}

impl<'a> StructureCollector<'a> {
    fn new(source: &'a str, body_root: Node<'_>, root_kind: &'a str) -> Self {
        Self {
            source,
            body_root_id: body_root.id(),
            root_kind,
            locals: Vec::new(),
            branches: Vec::new(),
        }
    }

    fn finish(self) -> (Vec<LocalInfo>, Vec<BranchInfo>) {
        (self.locals, self.branches)
    }

    fn visit(&mut self, node: Node<'_>) {
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

pub(super) fn callable_candidate(
    node: Node<'_>,
    source: &str,
    kind: CardSymbolKind,
    metadata: CallableMetadata,
) -> EntityCandidate {
    let body = node.child_by_field_name("body");
    let signature_end = body.map_or_else(|| node.end_byte(), |body_node| body_node.start_byte());
    let signature_source = source
        .get(node.start_byte()..signature_end)
        .unwrap_or_default();
    let range = to_range(node);
    let (locals, branches) = collect_structure(node, body, source);
    let params = node
        .child_by_field_name("parameters")
        .map_or_else(Vec::new, |param_node| {
            parse_parameters(source.get(param_node.byte_range()).unwrap_or_default())
        });
    let returns = node
        .child_by_field_name("return_type")
        .map_or_else(String::new, |return_node| {
            normalise_whitespace(source.get(return_node.byte_range()).unwrap_or_default())
        });

    EntityCandidate {
        kind,
        name: name_text(node, source),
        container: metadata.container,
        byte_range: node.byte_range(),
        range: range.clone(),
        signature_display: Some(normalise_whitespace(signature_source)),
        params,
        returns,
        locals,
        branches: branches.clone(),
        decorators: metadata.decorators,
        attachment_anchor: Some(node.start_byte()),
        docstring: metadata.docstring,
        lines: range.end.line.saturating_sub(range.start.line) + 1,
        structure_fingerprint: format!(
            "{}:{}:{}:{}",
            node.kind(),
            node.child_by_field_name("parameters")
                .map_or(0, |param_node| parse_parameters(
                    source.get(param_node.byte_range()).unwrap_or_default()
                )
                .len()),
            branches.len(),
            node.child_by_field_name("body").map_or(0, |_| 1)
        ),
        interstitial: None,
    }
}

pub(super) fn simple_candidate(
    node: Node<'_>,
    source: &str,
    kind: CardSymbolKind,
    container: Option<String>,
) -> EntityCandidate {
    let range = to_range(node);
    EntityCandidate {
        kind,
        name: name_text(node, source),
        container,
        byte_range: node.byte_range(),
        range: range.clone(),
        signature_display: None,
        params: Vec::new(),
        returns: String::new(),
        locals: Vec::new(),
        branches: Vec::new(),
        decorators: Vec::new(),
        attachment_anchor: Some(node.start_byte()),
        docstring: None,
        lines: range.end.line.saturating_sub(range.start.line) + 1,
        structure_fingerprint: String::from(node.kind()),
        interstitial: None,
    }
}

pub(super) fn top_level_imports(
    language: SupportedLanguage,
    root: Node<'_>,
    source: &str,
) -> Vec<ImportBlock> {
    let kinds: &[&str] = match language {
        SupportedLanguage::Rust => &["use_declaration", "extern_crate_declaration"],
        SupportedLanguage::Python => &["import_statement", "import_from_statement"],
        SupportedLanguage::TypeScript => &["import_statement"],
    };

    let mut cursor = root.walk();
    let nodes: Vec<Node<'_>> = root
        .named_children(&mut cursor)
        .filter(|child| kinds.contains(&child.kind()))
        .collect();

    group_consecutive_nodes(nodes)
        .into_iter()
        .filter_map(|group| import_block_from_group(language, &group, source))
        .collect()
}

pub(super) fn name_text(node: Node<'_>, source: &str) -> String {
    node.child_by_field_name("name")
        .and_then(|name| source.get(name.byte_range()))
        .map_or_else(
            || normalise_whitespace(source.get(node.byte_range()).unwrap_or_default()),
            normalise_whitespace,
        )
}

pub(super) fn impl_container_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .and_then(|ty| source.get(ty.byte_range()))
        .map(normalise_whitespace)
}

pub(super) fn decorator_texts(node: Node<'_>, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "decorator" {
            decorators.push(normalise_whitespace(
                source.get(child.byte_range()).unwrap_or_default(),
            ));
        }
    }
    decorators
}

pub(super) fn python_docstring(node: Node<'_>, source: &str) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();
    let first_statement = body.named_children(&mut cursor).next()?;
    if first_statement.kind() != "expression_statement" {
        return None;
    }

    let mut statement_cursor = first_statement.walk();
    let string_node = first_statement
        .named_children(&mut statement_cursor)
        .find(|child| child.kind() == "string")?;
    let raw = source.get(string_node.byte_range())?;
    let trimmed = raw.trim_matches('"').trim_matches('\'').trim();
    (!trimmed.is_empty()).then(|| String::from(trimmed))
}

pub(super) const fn extract_rust_docstring(_source: &str, _node: Node<'_>) -> Option<String> {
    None
}

pub(super) fn normalise_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn to_range(node: Node<'_>) -> SourceRange {
    let start = node.start_position();
    let end = node.end_position();
    SourceRange {
        start: SourcePosition {
            line: to_u32(start.row),
            column: to_u32(start.column),
        },
        end: SourcePosition {
            line: to_u32(end.row),
            column: to_u32(end.column),
        },
    }
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn collect_structure(
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

fn is_nested_entity(kind: &str, root_kind: &str) -> bool {
    kind == "function_definition"
        || kind == "lambda"
        || (kind == "function_item" && root_kind != "trait_item")
        || kind == "function_declaration"
        || kind == "method_definition"
}

fn local_info(node: Node<'_>, source: &str) -> Option<LocalInfo> {
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

fn branch_info(node: Node<'_>) -> Option<BranchInfo> {
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

fn normalise_import(language: SupportedLanguage, raw: &str) -> String {
    let trimmed = raw.trim();
    match language {
        SupportedLanguage::Rust => trimmed
            .trim_start_matches("pub ")
            .trim_start_matches("use ")
            .trim_start_matches("extern crate ")
            .trim_end_matches(';')
            .trim()
            .to_owned(),
        SupportedLanguage::Python => trimmed
            .trim_start_matches("from ")
            .trim_start_matches("import ")
            .trim()
            .to_owned(),
        SupportedLanguage::TypeScript => trimmed
            .trim_start_matches("import ")
            .trim_end_matches(';')
            .trim()
            .to_owned(),
    }
}

fn group_consecutive_nodes(nodes: Vec<Node<'_>>) -> Vec<Vec<Node<'_>>> {
    let mut groups: Vec<Vec<Node<'_>>> = Vec::new();
    for node in nodes {
        if let Some(group) = groups.last_mut() {
            let previous_end = group.last().map_or(0, |n| n.end_position().row);
            if node.start_position().row <= previous_end + 1 {
                group.push(node);
                continue;
            }
        }
        groups.push(vec![node]);
    }
    groups
}

fn import_block_from_group(
    language: SupportedLanguage,
    group: &[Node<'_>],
    source: &str,
) -> Option<ImportBlock> {
    let start = group.first().map(Node::start_byte)?;
    let end = group.last().map(Node::end_byte)?;
    source.get(start..end)?;
    let normalized = group
        .iter()
        .map(|node| normalise_import(language, source.get(node.byte_range()).unwrap_or_default()))
        .collect();
    Some(ImportBlock {
        byte_start: start,
        byte_end: end,
        normalized,
    })
}

fn is_skippable_param(s: &str) -> bool {
    const SKIPPABLE: &[&str] = &["", "self", "&self"];
    SKIPPABLE.contains(&s)
}

fn parse_parameters(raw: &str) -> Vec<ParamInfo> {
    raw.trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .map(str::trim)
        .filter(|p| !is_skippable_param(p))
        .map(|trimmed| {
            let (name, ty) = trimmed
                .split_once(':')
                .map_or((trimmed, ""), |(name, ty)| (name.trim(), ty.trim()));
            ParamInfo {
                name: String::from(name),
                type_annotation: String::from(ty),
            }
        })
        .collect()
}

fn binding_name(node: Node<'_>, source: &str) -> String {
    node.child_by_field_name("pattern")
        .and_then(|pattern| source.get(pattern.byte_range()))
        .map_or_else(
            || normalise_whitespace(source.get(node.byte_range()).unwrap_or_default()),
            normalise_whitespace,
        )
}

fn assignment_name(node: Node<'_>, source: &str) -> String {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .next()
        .and_then(|target| source.get(target.byte_range()))
        .map_or_else(|| String::from("assignment"), normalise_whitespace)
}

fn lexical_name(node: Node<'_>, source: &str) -> String {
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
