//! Call graph node representation.

use camino::Utf8PathBuf;

/// Unique identifier for a node in the call graph.
///
/// Node IDs are constructed from the symbol's location to ensure uniqueness
/// across the codebase. The format is `{path}:{line}:{column}:{name}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Creates a new node ID from its components.
    #[must_use]
    pub fn new(path: &Utf8PathBuf, line: u32, column: u32, name: &str) -> Self {
        Self(format!("{path}:{line}:{column}:{name}"))
    }

    /// Returns the string representation of this node ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Position in source code (line and column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// Zero-based line number.
    pub line: u32,
    /// Zero-based column number (UTF-16 code units).
    pub column: u32,
}

impl Position {
    /// Creates a new source position.
    #[must_use]
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// Kind of symbol represented by a call graph node.
///
/// This mirrors LSP's `SymbolKind` but only includes callable symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// A function definition.
    Function,
    /// A method on a class or struct.
    Method,
    /// A class constructor.
    Constructor,
    /// A property getter or setter (when callable).
    Property,
    /// Unknown or unclassified callable.
    Unknown,
}

impl SymbolKind {
    /// Converts from LSP `SymbolKind` to our domain type.
    #[must_use]
    pub const fn from_lsp(kind: lsp_types::SymbolKind) -> Self {
        match kind {
            lsp_types::SymbolKind::FUNCTION => Self::Function,
            lsp_types::SymbolKind::METHOD => Self::Method,
            lsp_types::SymbolKind::CONSTRUCTOR => Self::Constructor,
            lsp_types::SymbolKind::PROPERTY => Self::Property,
            _ => Self::Unknown,
        }
    }
}

/// A node in the call graph representing a callable symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallNode {
    /// Unique identifier for this node.
    id: NodeId,
    /// Human-readable name of the symbol.
    name: String,
    /// Kind of symbol (function, method, etc.).
    kind: SymbolKind,
    /// Path to the file containing this symbol.
    path: Utf8PathBuf,
    /// Line number where the symbol is defined (0-based).
    line: u32,
    /// Column number where the symbol is defined (0-based).
    column: u32,
    /// Optional container name (e.g., class name for methods).
    container: Option<String>,
}

impl CallNode {
    /// Creates a new call node.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        kind: SymbolKind,
        path: impl Into<Utf8PathBuf>,
        position: Position,
    ) -> Self {
        let name_str = name.into();
        let path_buf = path.into();
        let id = NodeId::new(&path_buf, position.line, position.column, &name_str);
        Self {
            id,
            name: name_str,
            kind,
            path: path_buf,
            line: position.line,
            column: position.column,
            container: None,
        }
    }

    /// Creates a new call node with a container.
    #[must_use]
    pub fn with_container(mut self, container: impl Into<String>) -> Self {
        self.container = Some(container.into());
        self
    }

    /// Returns the unique identifier for this node.
    #[must_use]
    pub const fn id(&self) -> &NodeId {
        &self.id
    }

    /// Returns the name of the symbol.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the kind of symbol.
    #[must_use]
    pub const fn kind(&self) -> SymbolKind {
        self.kind
    }

    /// Returns the path to the file containing this symbol.
    #[must_use]
    pub const fn path(&self) -> &Utf8PathBuf {
        &self.path
    }

    /// Returns the line number where the symbol is defined.
    #[must_use]
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// Returns the column number where the symbol is defined.
    #[must_use]
    pub const fn column(&self) -> u32 {
        self.column
    }

    /// Returns the container name if present.
    #[must_use]
    pub fn container(&self) -> Option<&str> {
        self.container.as_deref()
    }

    /// Returns the fully qualified name including the container.
    #[must_use]
    pub fn qualified_name(&self) -> String {
        self.container.as_ref().map_or_else(
            || self.name.clone(),
            |container| format!("{container}.{}", self.name),
        )
    }
}
