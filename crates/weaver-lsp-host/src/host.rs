//! Host facade that mediates access to per-language servers.

use std::collections::HashMap;

use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, ReferenceParams, Uri,
};

use crate::capability::{CapabilityKind, CapabilitySummary, resolve_capabilities};
use crate::errors::{HostOperation, LspHostError};
use crate::language::Language;
use crate::server::{LanguageServer, LanguageServerError};

struct Session {
    server: Box<dyn LanguageServer>,
    state: SessionState,
}

enum SessionState {
    Pending,
    Ready { summary: CapabilitySummary },
}

struct CallSpec {
    capability: CapabilityKind,
    operation: HostOperation,
}

struct CallContext {
    language: Language,
    operation: HostOperation,
    capability: Option<CapabilityKind>,
}

macro_rules! lsp_method {
    (
        $(#[$meta:meta])* $vis:vis fn $name:ident(
            &mut self,
            language: Language,
            $param:ident : $pty:ty $(,)?
        ) -> $ret:ty {
            $cap:expr,
            $op:expr,
            $server_method:ident
        }
    ) => {
        $(#[$meta])* $vis fn $name(
            &mut self,
            language: Language,
            $param: $pty,
        ) -> $ret {
            self.call_with_capability(
                language,
                CallSpec {
                    capability: $cap,
                    operation: $op,
                },
                move |server| server.$server_method($param),
            )
        }
    };
}

macro_rules! lsp_notification {
    (
        $(#[$meta:meta])* $vis:vis fn $name:ident(
            &mut self,
            language: Language,
            $param:ident : $pty:ty $(,)?
        ) -> $ret:ty {
            $op:expr,
            $server_method:ident
        }
    ) => {
        $(#[$meta])* $vis fn $name(
            &mut self,
            language: Language,
            $param: $pty,
        ) -> $ret {
            self.call_on_server(language, $op, move |server| server.$server_method($param))
        }
    };
}

/// Orchestrates multiple language servers and applies capability overrides.
pub struct LspHost {
    overrides: weaver_config::CapabilityMatrix,
    sessions: HashMap<Language, Session>,
}

impl LspHost {
    /// Builds an empty host with the supplied capability overrides.
    #[must_use]
    pub fn new(overrides: weaver_config::CapabilityMatrix) -> Self {
        Self {
            overrides,
            sessions: HashMap::new(),
        }
    }

    /// Registers a server for the given language.
    pub fn register_language(
        &mut self,
        language: Language,
        server: Box<dyn LanguageServer>,
    ) -> Result<(), LspHostError> {
        if self.sessions.contains_key(&language) {
            return Err(LspHostError::duplicate(language));
        }

        self.sessions.insert(
            language,
            Session {
                server,
                state: SessionState::Pending,
            },
        );
        Ok(())
    }

    /// Initializes the language server and returns the resolved capability summary.
    pub fn initialize(&mut self, language: Language) -> Result<CapabilitySummary, LspHostError> {
        let overrides = &self.overrides;
        let session = self
            .sessions
            .get_mut(&language)
            .ok_or_else(|| LspHostError::unknown(language))?;
        Self::ensure_initialized(language, session, overrides)
    }

    /// Returns the resolved capabilities when the language is already initialized.
    #[must_use]
    pub fn capabilities(&self, language: Language) -> Option<CapabilitySummary> {
        self.sessions
            .get(&language)
            .and_then(|session| match &session.state {
                SessionState::Ready { summary } => Some(summary.clone()),
                SessionState::Pending => None,
            })
    }

    lsp_method!(
        /// Routes a definition request to the configured language server.
        pub fn goto_definition(
            &mut self,
            language: Language,
            params: GotoDefinitionParams,
        ) -> Result<GotoDefinitionResponse, LspHostError> {
            CapabilityKind::Definition,
            HostOperation::Definition,
            goto_definition
        }
    );

    lsp_method!(
        /// Routes a references request to the configured language server.
        pub fn references(
            &mut self,
            language: Language,
            params: ReferenceParams,
        ) -> Result<Vec<lsp_types::Location>, LspHostError> {
            CapabilityKind::References,
            HostOperation::References,
            references
        }
    );

    lsp_method!(
        /// Retrieves diagnostics for the supplied document.
        pub fn diagnostics(
            &mut self,
            language: Language,
            uri: Uri,
        ) -> Result<Vec<lsp_types::Diagnostic>, LspHostError> {
            CapabilityKind::Diagnostics,
            HostOperation::Diagnostics,
            diagnostics
        }
    );

    lsp_notification!(
        /// Notifies the server that a document has been opened with in-memory content.
        #[doc = include_str!("../docs/did_open.md")]
        pub fn did_open(
            &mut self,
            language: Language,
            params: DidOpenTextDocumentParams,
        ) -> Result<(), LspHostError> {
            HostOperation::DidOpen,
            did_open
        }
    );

    lsp_notification!(
        /// Notifies the server that a document has changed with in-memory content.
        #[doc = include_str!("../docs/did_change.md")]
        pub fn did_change(
            &mut self,
            language: Language,
            params: DidChangeTextDocumentParams,
        ) -> Result<(), LspHostError> {
            HostOperation::DidChange,
            did_change
        }
    );

    lsp_notification!(
        /// Notifies the server that a document has been closed.
        #[doc = include_str!("../docs/did_close.md")]
        pub fn did_close(
            &mut self,
            language: Language,
            params: DidCloseTextDocumentParams,
        ) -> Result<(), LspHostError> {
            HostOperation::DidClose,
            did_close
        }
    );

    // Clippy: keep explicit arguments to make call sites self-descriptive.
    #[allow(clippy::too_many_arguments)]
    fn call_with_context<F, T>(
        &mut self,
        language: Language,
        operation: HostOperation,
        capability: Option<CapabilityKind>,
        call: F,
    ) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        self.call_with_session(
            CallContext {
                language,
                operation,
                capability,
            },
            call,
        )
    }

    fn call_with_capability<F, T>(
        &mut self,
        language: Language,
        spec: CallSpec,
        call: F,
    ) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        self.call_with_context(language, spec.operation, Some(spec.capability), call)
    }

    fn call_on_server<F, T>(
        &mut self,
        language: Language,
        operation: HostOperation,
        call: F,
    ) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        self.call_with_context(language, operation, None, call)
    }

    fn call_with_session<F, T>(&mut self, context: CallContext, call: F) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        let overrides = &self.overrides;
        let session = self
            .sessions
            .get_mut(&context.language)
            .ok_or_else(|| LspHostError::unknown(context.language))?;
        let summary = Self::ensure_initialized(context.language, session, overrides)?;
        if let Some(capability) = context.capability {
            let state = summary.state(capability);
            if !state.enabled {
                return Err(LspHostError::capability_unavailable(
                    context.language,
                    capability,
                    state.source,
                ));
            }
        }

        call(session.server.as_mut())
            .map_err(|source| LspHostError::server(context.language, context.operation, source))
    }

    fn ensure_initialized(
        language: Language,
        session: &mut Session,
        overrides: &weaver_config::CapabilityMatrix,
    ) -> Result<CapabilitySummary, LspHostError> {
        match &session.state {
            SessionState::Ready { summary } => Ok(summary.clone()),
            SessionState::Pending => {
                let capabilities = session.server.initialize().map_err(|source| {
                    LspHostError::server(language, HostOperation::Initialise, source)
                })?;

                let summary = resolve_capabilities(language, capabilities, overrides);
                session.state = SessionState::Ready {
                    summary: summary.clone(),
                };
                Ok(summary)
            }
        }
    }
}
