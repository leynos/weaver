//! Error types for call graph operations.

use std::sync::Arc;

use camino::Utf8PathBuf;
use thiserror::Error;

use weaver_lsp_host::LspHostError;

/// Errors returned by call graph operations.
#[derive(Debug, Error)]
pub enum GraphError {
    /// The underlying LSP host returned an error.
    #[error("LSP host error: {0}")]
    LspHost(#[from] LspHostError),

    /// Call hierarchy is not supported for the target language.
    #[error("call hierarchy not supported for language '{language}'")]
    CallHierarchyUnsupported {
        /// Language that lacks call hierarchy support.
        language: String,
    },

    /// Failed to resolve the symbol at the given position.
    #[error("no symbol found at {path}:{line}:{column}")]
    SymbolNotFound {
        /// Path to the file containing the position.
        path: Utf8PathBuf,
        /// Line number (0-based).
        line: u32,
        /// Column number (0-based).
        column: u32,
    },

    /// An IO error occurred during graph operations.
    #[error("IO error: {message}")]
    Io {
        /// Description of the IO error.
        message: String,
        /// Underlying error wrapped in Arc for Clone support.
        #[source]
        source: Arc<std::io::Error>,
    },

    /// The requested node was not found in the graph.
    #[error("node not found: {0}")]
    NodeNotFound(String),
}

impl GraphError {
    /// Creates a new `CallHierarchyUnsupported` error.
    #[must_use]
    pub fn call_hierarchy_unsupported(language: impl Into<String>) -> Self {
        Self::CallHierarchyUnsupported {
            language: language.into(),
        }
    }

    /// Creates a new `SymbolNotFound` error.
    #[must_use]
    pub fn symbol_not_found(path: impl Into<Utf8PathBuf>, line: u32, column: u32) -> Self {
        Self::SymbolNotFound {
            path: path.into(),
            line,
            column,
        }
    }

    /// Creates a new `Io` error.
    #[must_use]
    pub fn io(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            message: message.into(),
            source: Arc::new(source),
        }
    }

    /// Creates a new `NodeNotFound` error.
    #[must_use]
    pub fn node_not_found(node_id: impl Into<String>) -> Self {
        Self::NodeNotFound(node_id.into())
    }
}
