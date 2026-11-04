//! Abstractions over concrete language server implementations.

use std::{collections::BTreeSet, error::Error, fmt};

use lsp_types::{
    CallHierarchyIncomingCall,
    CallHierarchyIncomingCallsParams,
    CallHierarchyItem,
    CallHierarchyOutgoingCall,
    CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams,
    Diagnostic,
    DidChangeTextDocumentParams,
    DidCloseTextDocumentParams,
    DidOpenTextDocumentParams,
    GotoDefinitionParams,
    GotoDefinitionResponse,
    Hover,
    HoverParams,
    PositionEncodingKind,
    ReferenceParams,
    Uri,
};
use thiserror::Error;

use crate::capability::CapabilityKind;

/// Minimal set of capabilities the host inspects during negotiation.
///
/// Support is modelled as a set rather than a bank of boolean fields, so adding
/// a capability does not change the shape of this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCapabilitySet {
    /// Capabilities the server advertised during the initialize handshake.
    pub(crate) supported: BTreeSet<CapabilityKind>,
    /// Position encoding the server agreed to, when it named one.
    pub(crate) position_encoding: Option<PositionEncodingKind>,
}

impl ServerCapabilitySet {
    /// Builds a capability set describing the server's advertised support.
    #[must_use]
    pub fn new(definition: bool, references: bool, diagnostics: bool) -> Self {
        let mut set = Self {
            supported: BTreeSet::new(),
            position_encoding: None,
        };
        set.set(CapabilityKind::Definition, definition);
        set.set(CapabilityKind::References, references);
        set.set(CapabilityKind::Diagnostics, diagnostics);
        set
    }

    /// Builds a capability set with call hierarchy support.
    #[must_use]
    pub fn with_call_hierarchy(mut self, supported: bool) -> Self {
        self.set(CapabilityKind::CallHierarchy, supported);
        self
    }

    /// Builds a capability set with hover support.
    #[must_use]
    pub fn with_hover(mut self, supported: bool) -> Self {
        self.set(CapabilityKind::Hover, supported);
        self
    }

    /// Builds a capability set with position encoding.
    #[must_use]
    pub fn with_position_encoding(mut self, encoding: Option<PositionEncodingKind>) -> Self {
        self.position_encoding = encoding;
        self
    }

    /// Whether the server reports support for `capability`.
    #[must_use]
    pub fn supports(&self, capability: CapabilityKind) -> bool {
        self.supported.contains(&capability)
    }

    /// Records or clears support for `capability`.
    fn set(&mut self, capability: CapabilityKind, is_supported: bool) {
        if is_supported {
            self.supported.insert(capability);
        } else {
            self.supported.remove(&capability);
        }
    }

    /// Whether the server reports support for `textDocument/definition`.
    #[must_use]
    pub fn supports_definition(&self) -> bool { self.supports(CapabilityKind::Definition) }

    /// Whether the server reports support for `textDocument/references`.
    #[must_use]
    pub fn supports_references(&self) -> bool { self.supports(CapabilityKind::References) }

    /// Whether the server reports support for diagnostics.
    #[must_use]
    pub fn supports_diagnostics(&self) -> bool { self.supports(CapabilityKind::Diagnostics) }

    /// Whether the server reports support for `textDocument/prepareCallHierarchy`.
    #[must_use]
    pub fn supports_call_hierarchy(&self) -> bool { self.supports(CapabilityKind::CallHierarchy) }

    /// Whether the server reports support for `textDocument/hover`.
    #[must_use]
    pub fn supports_hover(&self) -> bool { self.supports(CapabilityKind::Hover) }

    /// Returns the negotiated position encoding.
    ///
    /// When `Some(PositionEncodingKind::UTF8)`, Tree-sitter byte offsets can be
    /// used directly. Otherwise, UTF-16 code unit conversion is required.
    #[must_use]
    pub const fn position_encoding(&self) -> Option<&PositionEncodingKind> {
        self.position_encoding.as_ref()
    }
}

/// Errors reported by language server implementations.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct LanguageServerError {
    /// Human-readable description rendered by the [`Display`] impl.
    ///
    /// [`Display`]: std::fmt::Display
    message: String,
    /// Underlying failure, when the error wraps one.
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
    pub const fn message(&self) -> &str { self.message.as_str() }
}

/// Behaviour required from concrete language server bindings.
#[doc = include_str!("../docs/language_server_trait.md")]
pub trait LanguageServer: Send {
    /// Runs the server initialization handshake and returns advertised capabilities.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the server cannot be spawned, the
    /// handshake fails, or the capabilities payload cannot be decoded.
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError>;

    /// Handles a `textDocument/definition` request.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the request cannot be delivered or
    /// the response cannot be decoded.
    fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LanguageServerError>;

    /// Handles a `textDocument/references` request.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the request cannot be delivered or
    /// the response cannot be decoded.
    fn references(
        &mut self,
        params: ReferenceParams,
    ) -> Result<Vec<lsp_types::Location>, LanguageServerError>;

    /// Returns diagnostics for the supplied URI.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the pull-diagnostics request fails
    /// or the report cannot be decoded.
    fn diagnostics(&mut self, uri: Uri) -> Result<Vec<Diagnostic>, LanguageServerError>;

    /// Notifies the server that a document has been opened with in-memory content.
    #[doc = include_str!("../docs/language_server_did_open.md")]
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the notification cannot be sent.
    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<(), LanguageServerError>;

    /// Notifies the server that a document has changed with updated in-memory content.
    #[doc = include_str!("../docs/language_server_did_change.md")]
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the notification cannot be sent.
    fn did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> Result<(), LanguageServerError>;

    /// Notifies the server that a document has been closed.
    #[doc = include_str!("../docs/language_server_did_close.md")]
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the notification cannot be sent.
    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<(), LanguageServerError>;

    /// Prepares a call hierarchy request at the given position.
    ///
    /// This is the first step in the call hierarchy protocol. It returns a list
    /// of call hierarchy items representing the functions/methods at the position.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the request cannot be delivered or
    /// the response cannot be decoded.
    fn prepare_call_hierarchy(
        &mut self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, LanguageServerError>;

    /// Returns the incoming calls (callers) for the given call hierarchy item.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the request cannot be delivered or
    /// the response cannot be decoded.
    fn incoming_calls(
        &mut self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LanguageServerError>;

    /// Returns the outgoing calls (callees) for the given call hierarchy item.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the request cannot be delivered or
    /// the response cannot be decoded.
    fn outgoing_calls(
        &mut self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LanguageServerError>;

    /// Handles a `textDocument/hover` request.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageServerError`] when the request cannot be delivered or
    /// the response cannot be decoded.
    fn hover(&mut self, params: HoverParams) -> Result<Option<Hover>, LanguageServerError>;
}

impl fmt::Debug for dyn LanguageServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LanguageServer")
    }
}
