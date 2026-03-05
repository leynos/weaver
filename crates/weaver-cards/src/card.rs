//! Symbol card data structures.
//!
//! A symbol card is a structured JSON object representing a single symbol
//! in a codebase. It supports progressive enrichment by detail level: the
//! `minimal` level carries only identity, while `full` includes dependency
//! edges and fan metrics.
//!
//! See `docs/jacquard-card-first-symbol-graph-design.md` §5.3 for the
//! canonical JSON shape.

use serde::{Deserialize, Serialize};

use crate::SymbolIdentity;

/// Signature parameter information.
///
/// # Example
///
/// ```
/// use weaver_cards::ParamInfo;
///
/// let param = ParamInfo {
///     name: String::from("x"),
///     type_annotation: String::from("int"),
/// };
/// assert_eq!(param.name, "x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamInfo {
    /// Parameter name.
    pub name: String,
    /// Type annotation as written in source.
    #[serde(rename = "type")]
    pub type_annotation: String,
}

/// Function or method signature information.
///
/// Present at `signature` detail level and above.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// Human-readable display string (e.g. `fn foo(x: i32) -> bool`).
    pub display: String,
    /// Positional parameters.
    pub params: Vec<ParamInfo>,
    /// Return type annotation.
    pub returns: String,
}

/// Documentation extracted from source.
///
/// Present at `structure` detail level and above.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocInfo {
    /// Full docstring or doc comment text.
    pub docstring: String,
    /// First-sentence summary (deterministic extraction, not LLM-generated).
    pub summary: String,
    /// Provenance source (e.g. `tree_sitter`).
    pub source: String,
}

/// A local variable or binding within a symbol body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalInfo {
    /// Variable name.
    pub name: String,
    /// Kind of binding (e.g. `variable`, `parameter`).
    pub kind: String,
    /// Declaration line number (zero-indexed).
    pub decl_line: u32,
}

/// A control-flow branch within a symbol body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    /// Branch kind (e.g. `if`, `for`, `match`, `while`).
    pub kind: String,
    /// Line number where the branch appears (zero-indexed).
    pub line: u32,
}

/// Structural analysis of a symbol body.
///
/// Present at `structure` detail level and above.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructureInfo {
    /// Local variables and bindings.
    pub locals: Vec<LocalInfo>,
    /// Control-flow branches.
    pub branches: Vec<BranchInfo>,
}

/// LSP-provided semantic information.
///
/// Present at `semantic` detail level and above.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspInfo {
    /// Hover documentation from the language server.
    pub hover: String,
    /// Resolved type annotation.
    #[serde(rename = "type")]
    pub type_info: String,
    /// Whether the symbol is marked as deprecated.
    pub deprecated: bool,
    /// Provenance source (e.g. `lsp_hover`).
    pub source: String,
}

/// Quantitative metrics for a symbol.
///
/// Basic metrics (`lines`, `cyclomatic`) are present at `structure` detail.
/// Fan metrics (`fan_in`, `fan_out`) are present at `full` detail only,
/// computed from the relational graph layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricsInfo {
    /// Number of source lines in the symbol body.
    pub lines: u32,
    /// Cyclomatic complexity.
    pub cyclomatic: u32,
    /// Number of incoming references (callers). Only at `full` detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_in: Option<u32>,
    /// Number of outgoing references (callees). Only at `full` detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fan_out: Option<u32>,
}

/// Dependency edges for a symbol.
///
/// Present at `full` detail level only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepsInfo {
    /// Symbol IDs of called functions or methods.
    pub calls: Vec<String>,
    /// Module identifiers for import dependencies.
    pub imports: Vec<String>,
    /// Configuration key identifiers.
    pub config: Vec<String>,
}

/// Provenance metadata recording how and when a card was extracted.
///
/// # Example
///
/// ```
/// use weaver_cards::Provenance;
///
/// let prov = Provenance {
///     extracted_at: String::from("2026-03-03T12:34:56Z"),
///     sources: vec![String::from("tree_sitter")],
/// };
/// assert_eq!(prov.sources.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// ISO 8601 timestamp of when the card was extracted.
    pub extracted_at: String,
    /// List of extraction sources (e.g. `tree_sitter`, `lsp_hover`).
    pub sources: Vec<String>,
}

/// A structured symbol card containing identity, signature, documentation,
/// structure, semantic, metrics, and dependency information.
///
/// Fields beyond identity are optional to support progressive detail levels.
/// When serialized, absent fields are omitted from the JSON output.
///
/// # Example
///
/// ```
/// use weaver_cards::{
///     CardLanguage, CardSymbolKind, Provenance, SourcePosition,
///     SourceRange, SymbolCard, SymbolIdentity, SymbolRef,
/// };
///
/// let card = SymbolCard {
///     card_version: 1,
///     symbol: SymbolIdentity {
///         symbol_id: String::from("sym_abc"),
///         symbol_ref: SymbolRef {
///             uri: String::from("file:///src/main.rs"),
///             range: SourceRange {
///                 start: SourcePosition { line: 0, column: 0 },
///                 end: SourcePosition { line: 10, column: 0 },
///             },
///             language: CardLanguage::Rust,
///             kind: CardSymbolKind::Function,
///             name: String::from("main"),
///             container: None,
///         },
///     },
///     signature: None,
///     doc: None,
///     structure: None,
///     lsp: None,
///     metrics: None,
///     deps: None,
///     provenance: Provenance {
///         extracted_at: String::from("2026-03-03T12:34:56Z"),
///         sources: vec![String::from("tree_sitter")],
///     },
///     etag: None,
/// };
/// assert_eq!(card.card_version, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolCard {
    /// Schema version for forward compatibility.
    pub card_version: u32,
    /// Symbol identity (ID + location reference).
    pub symbol: SymbolIdentity,
    /// Signature information (present at `signature` detail and above).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<SignatureInfo>,
    /// Documentation (present at `structure` detail and above).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<DocInfo>,
    /// Structural analysis (present at `structure` detail and above).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structure: Option<StructureInfo>,
    /// LSP semantic data (present at `semantic` detail and above).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<LspInfo>,
    /// Quantitative metrics (present at `structure` detail and above;
    /// fan metrics at `full` detail only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsInfo>,
    /// Dependency edges (present at `full` detail only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deps: Option<DepsInfo>,
    /// Provenance metadata (always present).
    pub provenance: Provenance,
    /// Content hash of the canonical JSON encoding for cache checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}
