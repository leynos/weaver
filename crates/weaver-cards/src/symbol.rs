//! Symbol identity types for card-based symbol context.
//!
//! This module defines the types used to identify and reference symbols
//! within the Jacquard card-first symbol graph system. A symbol has two
//! complementary identities:
//!
//! - A [`SymbolRef`] anchored to a source location (file, range, language).
//! - A [`SymbolId`] derived from content for version-scoped caching and
//!   matching.
//!
//! Together, these form a [`SymbolIdentity`] that the rest of the card
//! pipeline uses to look up, cache, and compare symbols across commits.

use serde::{Deserialize, Serialize};

/// Position within a source file.
///
/// Both `line` and `column` are zero-indexed, matching the internal
/// representation used by Tree-sitter and LSP.
///
/// # Example
///
/// ```
/// use weaver_cards::SourcePosition;
///
/// let pos = SourcePosition { line: 10, column: 0 };
/// assert_eq!(pos.line, 10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourcePosition {
    /// Zero-indexed line number.
    pub line: u32,
    /// Zero-indexed column number.
    pub column: u32,
}

/// Range within a source file defined by start and end positions.
///
/// # Example
///
/// ```
/// use weaver_cards::{SourcePosition, SourceRange};
///
/// let range = SourceRange {
///     start: SourcePosition { line: 10, column: 0 },
///     end: SourcePosition { line: 42, column: 1 },
/// };
/// assert_eq!(range.start.line, 10);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceRange {
    /// Inclusive start position.
    pub start: SourcePosition,
    /// Exclusive end position.
    pub end: SourcePosition,
}

/// Kind of symbol represented in a symbol card.
///
/// These kinds align with the symbol taxonomy defined in
/// `docs/jacquard-card-first-symbol-graph-design.md` §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CardSymbolKind {
    /// A function definition.
    Function,
    /// A method on a class or struct.
    Method,
    /// A class definition.
    Class,
    /// An interface or trait definition.
    Interface,
    /// A type alias or type definition.
    Type,
    /// A variable binding.
    Variable,
    /// A module or namespace.
    Module,
    /// A field of a struct or class.
    Field,
}

/// Language of the source file containing the symbol.
///
/// Matches Weaver's supported language set for Tree-sitter and LSP backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CardLanguage {
    /// Rust source.
    Rust,
    /// Python source.
    Python,
    /// TypeScript source.
    #[serde(rename = "typescript")]
    TypeScript,
}

/// Location-based reference to a symbol.
///
/// Used as the primary input handle for symbol lookups and as part of
/// the identity section of a [`SymbolCard`](crate::SymbolCard).
///
/// # Example
///
/// ```
/// use weaver_cards::{
///     CardLanguage, CardSymbolKind, SourcePosition, SourceRange, SymbolRef,
/// };
///
/// let sym_ref = SymbolRef {
///     uri: String::from("file:///src/main.rs"),
///     range: SourceRange {
///         start: SourcePosition { line: 10, column: 0 },
///         end: SourcePosition { line: 42, column: 1 },
///     },
///     language: CardLanguage::Rust,
///     kind: CardSymbolKind::Function,
///     name: String::from("process_request"),
///     container: None,
/// };
/// assert_eq!(sym_ref.name, "process_request");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRef {
    /// File URI (e.g. `file:///src/main.rs`).
    pub uri: String,
    /// Range of the symbol definition in the source file.
    pub range: SourceRange,
    /// Programming language of the source file.
    pub language: CardLanguage,
    /// Kind of symbol (function, method, class, etc.).
    pub kind: CardSymbolKind,
    /// Symbol name as written in source.
    pub name: String,
    /// Optional container or namespace (class, module).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
}

/// Content-derived, version-scoped symbol identifier.
///
/// The `symbol_id` is a base64url-encoded hash derived from the symbol's
/// language, kind, canonical name, signature fingerprint, syntactic
/// fingerprint, and file path hint. It remains stable under
/// whitespace-only edits but is not expected to survive semantic changes.
///
/// # Example
///
/// ```
/// use weaver_cards::SymbolId;
///
/// let id = SymbolId {
///     symbol_id: String::from("sym_abc123"),
/// };
/// assert_eq!(id.symbol_id, "sym_abc123");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId {
    /// The base64url-encoded hash identifying this symbol.
    pub symbol_id: String,
}

/// Combined symbol identity, pairing the content-derived ID with the
/// location-based reference.
///
/// # Example
///
/// ```
/// use weaver_cards::{
///     CardLanguage, CardSymbolKind, SourcePosition, SourceRange,
///     SymbolIdentity, SymbolRef,
/// };
///
/// let identity = SymbolIdentity {
///     symbol_id: String::from("sym_abc123"),
///     symbol_ref: SymbolRef {
///         uri: String::from("file:///src/main.rs"),
///         range: SourceRange {
///             start: SourcePosition { line: 10, column: 0 },
///             end: SourcePosition { line: 42, column: 1 },
///         },
///         language: CardLanguage::Rust,
///         kind: CardSymbolKind::Function,
///         name: String::from("process_request"),
///         container: None,
///     },
/// };
/// assert_eq!(identity.symbol_id, "sym_abc123");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolIdentity {
    /// Content-derived stable identifier.
    pub symbol_id: String,
    /// Location-based reference for bootstrapping and recovery.
    #[serde(rename = "ref")]
    pub symbol_ref: SymbolRef,
}
