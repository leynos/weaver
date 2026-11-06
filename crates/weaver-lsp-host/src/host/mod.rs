//! Host facade that mediates access to per-language servers.

use std::collections::HashMap;

use lsp_types::{
    CallHierarchyIncomingCall,
    CallHierarchyIncomingCallsParams,
    CallHierarchyItem,
    CallHierarchyOutgoingCall,
    CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams,
    DidChangeTextDocumentParams,
    DidCloseTextDocumentParams,
    DidOpenTextDocumentParams,
    GotoDefinitionParams,
    GotoDefinitionResponse,
    Hover,
    HoverParams,
    ReferenceParams,
    Uri,
};

mod call_context;

use self::call_context::{CallContext, CallSpec};
use crate::{
    capability::{CapabilityKind, CapabilitySummary, resolve_capabilities},
    errors::{HostOperation, LspHostError},
    language::Language,
    server::{LanguageServer, LanguageServerError},
};

/// A registered language server and the state of its initialisation.
struct Session {
    /// Server process handle used to dispatch requests.
    server: Box<dyn LanguageServer>,
    /// Whether capability negotiation has completed.
    state: SessionState,
}

/// Lifecycle state of a registered language server session.
enum SessionState {
    /// The server has been registered but not yet initialized.
    Pending,
    /// Capabilities have been negotiated with the server.
    Ready {
        /// Capabilities resolved for this language.
        summary: CapabilitySummary,
    },
}

/// Generates a capability-gated host method that forwards to a server method.
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
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// Returns [`LspHostError`] when the language is unknown, the server
        /// cannot be initialised, or the capability is disabled.
        $vis fn $name(
            &mut self,
            language: Language,
            $param: $pty,
        ) -> $ret {
            self.call_with_capability(
                language,
                &CallSpec {
                    capability: $cap,
                    operation: $op,
                },
                move |server| server.$server_method($param),
            )
        }
    };
}

/// Generates a host method that forwards a notification to a server method.
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
        $(#[$meta])*
        ///
        /// # Errors
        ///
        /// Returns [`LspHostError`] when the language is unknown, the server
        /// cannot be initialised, or the notification is rejected.
        $vis fn $name(
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
    /// Capability overrides applied on top of each server's own advertisement.
    overrides: weaver_config::CapabilityMatrix,
    /// Sessions keyed by the language they serve.
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
    ///
    /// # Errors
    ///
    /// Returns [`LspHostError`] when a server is already registered for
    /// `language`.
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
    ///
    /// # Errors
    ///
    /// Returns [`LspHostError`] when the language is unknown or the server fails
    /// to initialise.
    pub fn initialize(&mut self, language: Language) -> Result<CapabilitySummary, LspHostError> {
        let (session, overrides) = self.session_mut(language)?;
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
        #[doc = include_str!("../../docs/did_open.md")]
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
        #[doc = include_str!("../../docs/did_change.md")]
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
        #[doc = include_str!("../../docs/did_close.md")]
        pub fn did_close(
            &mut self,
            language: Language,
            params: DidCloseTextDocumentParams,
        ) -> Result<(), LspHostError> {
            HostOperation::DidClose,
            did_close
        }
    );

    lsp_method!(
        /// Prepares a call hierarchy request at the given position.
        ///
        /// Returns a list of call hierarchy items representing the callable symbols
        /// at the position. This is the first step in the call hierarchy protocol.
        pub fn prepare_call_hierarchy(
            &mut self,
            language: Language,
            params: CallHierarchyPrepareParams,
        ) -> Result<Option<Vec<CallHierarchyItem>>, LspHostError> {
            CapabilityKind::CallHierarchy,
            HostOperation::PrepareCallHierarchy,
            prepare_call_hierarchy
        }
    );

    lsp_method!(
        /// Returns the incoming calls (callers) for the given call hierarchy item.
        pub fn incoming_calls(
            &mut self,
            language: Language,
            params: CallHierarchyIncomingCallsParams,
        ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LspHostError> {
            CapabilityKind::CallHierarchy,
            HostOperation::IncomingCalls,
            incoming_calls
        }
    );

    lsp_method!(
        /// Returns the outgoing calls (callees) for the given call hierarchy item.
        pub fn outgoing_calls(
            &mut self,
            language: Language,
            params: CallHierarchyOutgoingCallsParams,
        ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LspHostError> {
            CapabilityKind::CallHierarchy,
            HostOperation::OutgoingCalls,
            outgoing_calls
        }
    );

    lsp_method!(
        /// Routes a hover request to the configured language server.
        pub fn hover(
            &mut self,
            language: Language,
            params: HoverParams,
        ) -> Result<Option<Hover>, LspHostError> {
            CapabilityKind::Hover,
            HostOperation::Hover,
            hover
        }
    );

    /// Dispatches a capability-gated call against the session for `language`.
    fn call_with_capability<F, T>(
        &mut self,
        language: Language,
        spec: &CallSpec,
        call: F,
    ) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        let context = CallContext::with_capability(language, spec.operation, spec.capability);
        self.call_with_session(&context, call)
    }

    /// Dispatches an ungated call against the session for `language`.
    fn call_on_server<F, T>(
        &mut self,
        language: Language,
        operation: HostOperation,
        call: F,
    ) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        let context = CallContext::for_operation(language, operation);
        self.call_with_session(&context, call)
    }

    /// Ensures the session is initialized, enforces capabilities, then calls it.
    fn call_with_session<F, T>(&mut self, context: &CallContext, call: F) -> Result<T, LspHostError>
    where
        F: FnOnce(&mut dyn LanguageServer) -> Result<T, LanguageServerError>,
    {
        let (session, overrides) = self.session_mut(context.language)?;
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

    /// Borrows the session for `language` and the override matrix together.
    ///
    /// A single lookup keeps both disjoint borrows inside one method.
    fn session_mut(
        &mut self,
        language: Language,
    ) -> Result<(&mut Session, &weaver_config::CapabilityMatrix), LspHostError> {
        let overrides = &self.overrides;
        let session = self
            .sessions
            .get_mut(&language)
            .ok_or_else(|| LspHostError::unknown(language))?;
        Ok((session, overrides))
    }

    /// Initializes `session` on first use and returns its capability summary.
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

                let summary = resolve_capabilities(language, &capabilities, overrides);
                session.state = SessionState::Ready {
                    summary: summary.clone(),
                };
                Ok(summary)
            }
        }
    }
}
