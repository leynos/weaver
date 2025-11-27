//! Host facade that mediates access to per-language servers.

use std::collections::HashMap;

use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, ReferenceParams, Uri};

use crate::capability::{CapabilityKind, CapabilitySummary, resolve_capabilities};
use crate::errors::{HostOperation, LspHostError};
use crate::language::Language;
use crate::server::{LanguageServer, LanguageServerError};

struct Session {
    server: Box<dyn LanguageServer>,
    state: SessionState,
}

struct CallContext {
    language: Language,
    capability: CapabilityKind,
    operation: HostOperation,
}

enum SessionState {
    Pending,
    Ready { summary: CapabilitySummary },
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

    /// Initialises the language server and returns the resolved capability summary.
    pub fn initialise(&mut self, language: Language) -> Result<CapabilitySummary, LspHostError> {
        let overrides = self.overrides.clone();
        let session = self.session_mut(language)?;
        Self::ensure_initialised(language, session, &overrides)
    }

    /// Returns the resolved capabilities when the language is already initialised.
    #[must_use]
    pub fn capabilities(&self, language: Language) -> Option<CapabilitySummary> {
        self.sessions
            .get(&language)
            .and_then(|session| match &session.state {
                SessionState::Ready { summary } => Some(summary.clone()),
                SessionState::Pending => None,
            })
    }

    /// Routes a definition request to the configured language server.
    pub fn goto_definition(
        &mut self,
        language: Language,
        params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LspHostError> {
        let context = CallContext {
            language,
            capability: CapabilityKind::Definition,
            operation: HostOperation::Definition,
        };
        self.call_with_capability(context, move |server| server.goto_definition(params))
    }

    /// Routes a references request to the configured language server.
    pub fn references(
        &mut self,
        language: Language,
        params: ReferenceParams,
    ) -> Result<Vec<lsp_types::Location>, LspHostError> {
        let context = CallContext {
            language,
            capability: CapabilityKind::References,
            operation: HostOperation::References,
        };
        self.call_with_capability(context, move |server| server.references(params))
    }

    /// Retrieves diagnostics for the supplied document.
    pub fn diagnostics(
        &mut self,
        language: Language,
        uri: Uri,
    ) -> Result<Vec<lsp_types::Diagnostic>, LspHostError> {
        let context = CallContext {
            language,
            capability: CapabilityKind::Diagnostics,
            operation: HostOperation::Diagnostics,
        };
        self.call_with_capability(context, move |server| server.diagnostics(uri))
    }

    fn call_with_capability<F, T>(
        &mut self,
        context: CallContext,
        call: F,
    ) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        let overrides = self.overrides.clone();
        let session = self.session_mut(context.language)?;
        let summary = Self::ensure_initialised(context.language, session, &overrides)?;
        let state = summary.state(context.capability);
        if !state.enabled {
            return Err(LspHostError::capability_unavailable(
                context.language,
                context.capability,
                state.source,
            ));
        }

        call(session.server.as_mut())
            .map_err(|source| LspHostError::server(context.language, context.operation, source))
    }

    fn ensure_initialised(
        language: Language,
        session: &mut Session,
        overrides: &weaver_config::CapabilityMatrix,
    ) -> Result<CapabilitySummary, LspHostError> {
        match &session.state {
            SessionState::Ready { summary } => Ok(summary.clone()),
            SessionState::Pending => {
                let capabilities = session.server.initialise().map_err(|source| {
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

    fn session_mut(&mut self, language: Language) -> Result<&mut Session, LspHostError> {
        self.sessions
            .get_mut(&language)
            .ok_or_else(|| LspHostError::unknown(language))
    }
}
