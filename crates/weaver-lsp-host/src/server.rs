//! Abstractions over concrete language server implementations.

use std::error::Error;
use std::fmt;

use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, ReferenceParams, Uri,
};
use thiserror::Error;

/// Minimal set of capabilities the host inspects during negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerCapabilitySet {
    definition: bool,
    references: bool,
    diagnostics: bool,
}

impl ServerCapabilitySet {
    /// Builds a capability set describing the server's advertised support.
    #[must_use]
    pub fn new(definition: bool, references: bool, diagnostics: bool) -> Self {
        Self {
            definition,
            references,
            diagnostics,
        }
    }

    /// Whether the server reports support for `textDocument/definition`.
    #[must_use]
    pub fn supports_definition(self) -> bool {
        self.definition
    }

    /// Whether the server reports support for `textDocument/references`.
    #[must_use]
    pub fn supports_references(self) -> bool {
        self.references
    }

    /// Whether the server reports support for diagnostics.
    #[must_use]
    pub fn supports_diagnostics(self) -> bool {
        self.diagnostics
    }
}

/// Errors reported by language server implementations.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct LanguageServerError {
    message: String,
    #[source]
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl LanguageServerError {
    /// Builds an error without an underlying source.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    /// Builds an error that wraps an underlying source.
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl Into<Box<dyn Error + Send + Sync>>,
    ) -> Self {
        Self {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Human-friendly description without the optional source.
    #[must_use]
    pub fn message(&self) -> &str {
        self.message.as_str()
    }
}

/// Behaviour required from concrete language server bindings.
pub trait LanguageServer: Send {
    /// Runs the server initialization handshake and returns advertised capabilities.
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError>;

    /// Handles a `textDocument/definition` request.
    fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LanguageServerError>;

    /// Handles a `textDocument/references` request.
    fn references(
        &mut self,
        params: ReferenceParams,
    ) -> Result<Vec<lsp_types::Location>, LanguageServerError>;

    /// Returns diagnostics for the supplied URI.
    fn diagnostics(&mut self, uri: Uri) -> Result<Vec<Diagnostic>, LanguageServerError>;

    /// Notifies the server that a document has been opened with in-memory content.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::str::FromStr;
    /// # use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Uri};
    /// # use weaver_lsp_host::{LanguageServer, LanguageServerError, ServerCapabilitySet};
    /// # struct StubServer;
    /// # impl LanguageServer for StubServer {
    /// #     fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
    /// #         Ok(ServerCapabilitySet::new(false, false, false))
    /// #     }
    /// #     fn goto_definition(
    /// #         &mut self,
    /// #         _params: lsp_types::GotoDefinitionParams,
    /// #     ) -> Result<lsp_types::GotoDefinitionResponse, LanguageServerError> {
    /// #         Ok(lsp_types::GotoDefinitionResponse::Array(Vec::new()))
    /// #     }
    /// #     fn references(
    /// #         &mut self,
    /// #         _params: lsp_types::ReferenceParams,
    /// #     ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
    /// #         Ok(Vec::new())
    /// #     }
    /// #     fn diagnostics(
    /// #         &mut self,
    /// #         _uri: lsp_types::Uri,
    /// #     ) -> Result<Vec<lsp_types::Diagnostic>, LanguageServerError> {
    /// #         Ok(Vec::new())
    /// #     }
    /// #     fn did_open(
    /// #         &mut self,
    /// #         _params: DidOpenTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// #     fn did_change(
    /// #         &mut self,
    /// #         _params: lsp_types::DidChangeTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// #     fn did_close(
    /// #         &mut self,
    /// #         _params: lsp_types::DidCloseTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let mut server = StubServer;
    /// let uri = Uri::from_str("file:///workspace/main.rs")?;
    /// let params = DidOpenTextDocumentParams {
    ///     text_document: TextDocumentItem {
    ///         uri,
    ///         language_id: "rust".to_string(),
    ///         version: 1,
    ///         text: "fn main() {}".to_string(),
    ///     },
    /// };
    /// server.did_open(params)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<(), LanguageServerError>;

    /// Notifies the server that a document has changed with updated in-memory content.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::str::FromStr;
    /// # use lsp_types::{DidChangeTextDocumentParams, TextDocumentContentChangeEvent};
    /// # use lsp_types::{Uri, VersionedTextDocumentIdentifier};
    /// # use weaver_lsp_host::{LanguageServer, LanguageServerError, ServerCapabilitySet};
    /// # struct StubServer;
    /// # impl LanguageServer for StubServer {
    /// #     fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
    /// #         Ok(ServerCapabilitySet::new(false, false, false))
    /// #     }
    /// #     fn goto_definition(
    /// #         &mut self,
    /// #         _params: lsp_types::GotoDefinitionParams,
    /// #     ) -> Result<lsp_types::GotoDefinitionResponse, LanguageServerError> {
    /// #         Ok(lsp_types::GotoDefinitionResponse::Array(Vec::new()))
    /// #     }
    /// #     fn references(
    /// #         &mut self,
    /// #         _params: lsp_types::ReferenceParams,
    /// #     ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
    /// #         Ok(Vec::new())
    /// #     }
    /// #     fn diagnostics(
    /// #         &mut self,
    /// #         _uri: lsp_types::Uri,
    /// #     ) -> Result<Vec<lsp_types::Diagnostic>, LanguageServerError> {
    /// #         Ok(Vec::new())
    /// #     }
    /// #     fn did_open(
    /// #         &mut self,
    /// #         _params: lsp_types::DidOpenTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// #     fn did_change(
    /// #         &mut self,
    /// #         _params: DidChangeTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// #     fn did_close(
    /// #         &mut self,
    /// #         _params: lsp_types::DidCloseTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let mut server = StubServer;
    /// let uri = Uri::from_str("file:///workspace/main.rs")?;
    /// let params = DidChangeTextDocumentParams {
    ///     text_document: VersionedTextDocumentIdentifier { uri, version: 2 },
    ///     content_changes: vec![TextDocumentContentChangeEvent {
    ///         range: None,
    ///         range_length: None,
    ///         text: "fn main() { println!(\"hi\"); }".to_string(),
    ///     }],
    /// };
    /// server.did_change(params)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> Result<(), LanguageServerError>;

    /// Notifies the server that a document has been closed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::str::FromStr;
    /// # use lsp_types::{DidCloseTextDocumentParams, TextDocumentIdentifier, Uri};
    /// # use weaver_lsp_host::{LanguageServer, LanguageServerError, ServerCapabilitySet};
    /// # struct StubServer;
    /// # impl LanguageServer for StubServer {
    /// #     fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
    /// #         Ok(ServerCapabilitySet::new(false, false, false))
    /// #     }
    /// #     fn goto_definition(
    /// #         &mut self,
    /// #         _params: lsp_types::GotoDefinitionParams,
    /// #     ) -> Result<lsp_types::GotoDefinitionResponse, LanguageServerError> {
    /// #         Ok(lsp_types::GotoDefinitionResponse::Array(Vec::new()))
    /// #     }
    /// #     fn references(
    /// #         &mut self,
    /// #         _params: lsp_types::ReferenceParams,
    /// #     ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
    /// #         Ok(Vec::new())
    /// #     }
    /// #     fn diagnostics(
    /// #         &mut self,
    /// #         _uri: lsp_types::Uri,
    /// #     ) -> Result<Vec<lsp_types::Diagnostic>, LanguageServerError> {
    /// #         Ok(Vec::new())
    /// #     }
    /// #     fn did_open(
    /// #         &mut self,
    /// #         _params: lsp_types::DidOpenTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// #     fn did_change(
    /// #         &mut self,
    /// #         _params: lsp_types::DidChangeTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// #     fn did_close(
    /// #         &mut self,
    /// #         _params: DidCloseTextDocumentParams,
    /// #     ) -> Result<(), LanguageServerError> {
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let mut server = StubServer;
    /// let uri = Uri::from_str("file:///workspace/main.rs")?;
    /// let params = DidCloseTextDocumentParams {
    ///     text_document: TextDocumentIdentifier { uri },
    /// };
    /// server.did_close(params)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<(), LanguageServerError>;
}

impl fmt::Debug for dyn LanguageServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LanguageServer")
    }
}
