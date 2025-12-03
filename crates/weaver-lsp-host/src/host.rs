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

enum SessionState {
    Pending,
    Ready { summary: CapabilitySummary },
}

struct CallSpec {
    capability: CapabilityKind,
    operation: HostOperation,
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

    fn call_with_capability<F, T>(
        &mut self,
        language: Language,
        spec: CallSpec,
        call: F,
    ) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        let overrides = &self.overrides;
        let session = self
            .sessions
            .get_mut(&language)
            .ok_or_else(|| LspHostError::unknown(language))?;
        let summary = Self::ensure_initialized(language, session, overrides)?;
        let state = summary.state(spec.capability);
        if !state.enabled {
            return Err(LspHostError::capability_unavailable(
                language,
                spec.capability,
                state.source,
            ));
        }

        call(session.server.as_mut())
            .map_err(|source| LspHostError::server(language, spec.operation, source))
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
