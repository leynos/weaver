//! Error types surfaced by the LSP host facade.

use std::fmt;

use thiserror::Error;

use crate::capability::{CapabilityKind, CapabilitySource};
use crate::language::Language;
use crate::server::LanguageServerError;

/// Operation being executed when an error occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOperation {
    /// Server initialisation handshake.
    Initialise,
    /// `textDocument/definition` handling.
    Definition,
    /// `textDocument/references` handling.
    References,
    /// Diagnostic retrieval.
    Diagnostics,
    /// `textDocument/didOpen` notification.
    DidOpen,
    /// `textDocument/didChange` notification.
    DidChange,
    /// `textDocument/didClose` notification.
    DidClose,
    /// `textDocument/prepareCallHierarchy` request.
    PrepareCallHierarchy,
    /// `callHierarchy/incomingCalls` request.
    IncomingCalls,
    /// `callHierarchy/outgoingCalls` request.
    OutgoingCalls,
}

impl fmt::Display for HostOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Initialise => "initialisation",
            Self::Definition => "definition",
            Self::References => "references",
            Self::Diagnostics => "diagnostics",
            Self::DidOpen => "didOpen",
            Self::DidChange => "didChange",
            Self::DidClose => "didClose",
            Self::PrepareCallHierarchy => "prepareCallHierarchy",
            Self::IncomingCalls => "incomingCalls",
            Self::OutgoingCalls => "outgoingCalls",
        };
        formatter.write_str(label)
    }
}

/// Errors returned by [`crate::LspHost`].
#[derive(Debug, Error)]
pub enum LspHostError {
    /// The requested language has not been registered.
    ///
    /// This typically occurs when no language server adapter has been
    /// configured for the language. Process-based language server adapters
    /// (for rust-analyzer, pyright, etc.) are not yet implemented.
    #[error(
        "no language server registered for '{language}'; \
         process-based language server adapters are not yet implemented"
    )]
    UnknownLanguage {
        /// Language requested by the caller.
        language: Language,
    },

    /// The language has already been registered.
    #[error("language '{language}' already has a registered server")]
    DuplicateLanguage {
        /// Language for which a duplicate server was registered.
        language: Language,
    },

    /// A capability is disabled by overrides or missing server support.
    #[error("capability {capability:?} for {language} is unavailable: {reason}")]
    CapabilityUnavailable {
        /// Language associated with the capability.
        language: Language,
        /// Capability that was requested.
        capability: CapabilityKind,
        /// Why the capability is not available.
        reason: CapabilitySource,
    },

    /// Underlying language server returned an error.
    #[error("language server for {language} failed during {operation}: {source}")]
    Server {
        /// Language associated with the server.
        language: Language,
        /// Operation that failed.
        operation: HostOperation,
        /// Underlying error.
        #[source]
        source: LanguageServerError,
    },
}

impl LspHostError {
    /// Builds an `UnknownLanguage` error for the supplied language.
    pub(crate) fn unknown(language: Language) -> Self {
        Self::UnknownLanguage { language }
    }

    /// Builds a `DuplicateLanguage` error.
    pub(crate) fn duplicate(language: Language) -> Self {
        Self::DuplicateLanguage { language }
    }

    /// Builds a `CapabilityUnavailable` error with the provided reason.
    pub(crate) fn capability_unavailable(
        language: Language,
        capability: CapabilityKind,
        reason: CapabilitySource,
    ) -> Self {
        Self::CapabilityUnavailable {
            language,
            capability,
            reason,
        }
    }

    /// Wraps an underlying language server failure.
    pub(crate) fn server(
        language: Language,
        operation: HostOperation,
        source: LanguageServerError,
    ) -> Self {
        Self::Server {
            language,
            operation,
            source,
        }
    }
}
