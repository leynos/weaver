//! Recording language server used in tests.

use std::sync::{Arc, Mutex};

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Location, ReferenceParams, Uri,
};

use crate::server::{LanguageServer, LanguageServerError, ServerCapabilitySet};

/// Discriminates the kind of call recorded by the stub server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    /// `initialise` was invoked.
    Initialise,
    /// `textDocument/definition` was invoked.
    Definition,
    /// `textDocument/references` was invoked.
    References,
    /// Diagnostics were requested.
    Diagnostics,
    /// `textDocument/didOpen` was invoked.
    DidOpen,
    /// `textDocument/didChange` was invoked.
    DidChange,
    /// `textDocument/didClose` was invoked.
    DidClose,
    /// `textDocument/prepareCallHierarchy` was invoked.
    PrepareCallHierarchy,
    /// `callHierarchy/incomingCalls` was invoked.
    IncomingCalls,
    /// `callHierarchy/outgoingCalls` was invoked.
    OutgoingCalls,
}

/// Test double that records every request routed through it.
#[derive(Clone)]
pub struct RecordingLanguageServer {
    shared: Arc<Mutex<RecordingState>>,
}

impl RecordingLanguageServer {
    /// Creates a server that reports the provided capabilities and responses.
    pub fn new(capabilities: ServerCapabilitySet, responses: ResponseSet) -> Self {
        Self {
            shared: Arc::new(Mutex::new(RecordingState::new(
                capabilities,
                responses,
                None,
            ))),
        }
    }

    /// Creates a server that fails during initialisation.
    pub fn failing_initialize(
        capabilities: ServerCapabilitySet,
        message: impl Into<String>,
    ) -> Self {
        Self {
            shared: Arc::new(Mutex::new(RecordingState::new(
                capabilities,
                ResponseSet::default(),
                Some(message.into()),
            ))),
        }
    }

    /// Returns a handle that can be used to assert recorded calls.
    pub fn handle(&self) -> RecordingServerHandle {
        RecordingServerHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    fn validate_and_execute<R>(
        &mut self,
        call_kind: CallKind,
        operation: &str,
        action: impl FnOnce(&ResponseSet) -> Result<R, LanguageServerError>,
    ) -> Result<R, LanguageServerError> {
        with_state(&self.shared, |state| {
            state.record_call(call_kind);
            if !state.initialised {
                return Err(LanguageServerError::new(format!(
                    "{operation} requested before initialisation",
                )));
            }
            action(&state.responses)
        })
    }

    fn handle_request<R>(
        &mut self,
        call_kind: CallKind,
        operation: &str,
        extract_response: impl FnOnce(&ResponseSet) -> R,
    ) -> Result<R, LanguageServerError> {
        self.validate_and_execute(call_kind, operation, |responses| {
            Ok(extract_response(responses))
        })
    }

    fn handle_notification(
        &mut self,
        call_kind: CallKind,
        operation: &str,
        extract_error: impl FnOnce(&ResponseSet) -> Option<String>,
    ) -> Result<(), LanguageServerError> {
        self.validate_and_execute(call_kind, operation, |responses| {
            if let Some(message) = extract_error(responses) {
                return Err(LanguageServerError::new(message));
            }
            Ok(())
        })
    }
}

impl LanguageServer for RecordingLanguageServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        with_state(&self.shared, |state| {
            state.record_call(CallKind::Initialise);
            if let Some(message) = &state.fail_initialise {
                return Err(LanguageServerError::new(message.clone()));
            }
            state.initialised = true;
            Ok(state.capabilities)
        })
    }

    fn goto_definition(
        &mut self,
        _params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LanguageServerError> {
        self.handle_request(CallKind::Definition, "definition", |responses| {
            responses.definition.clone()
        })
    }

    fn references(
        &mut self,
        _params: ReferenceParams,
    ) -> Result<Vec<Location>, LanguageServerError> {
        self.handle_request(CallKind::References, "references", |responses| {
            responses.references.clone()
        })
    }

    fn diagnostics(&mut self, _uri: Uri) -> Result<Vec<Diagnostic>, LanguageServerError> {
        self.handle_request(CallKind::Diagnostics, "diagnostics", |responses| {
            responses.diagnostics.clone()
        })
    }

    fn did_open(&mut self, _params: DidOpenTextDocumentParams) -> Result<(), LanguageServerError> {
        self.handle_notification(CallKind::DidOpen, "didOpen", |responses| {
            responses.document_sync.did_open_error.clone()
        })
    }

    fn did_change(
        &mut self,
        _params: DidChangeTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        self.handle_notification(CallKind::DidChange, "didChange", |responses| {
            responses.document_sync.did_change_error.clone()
        })
    }

    fn did_close(
        &mut self,
        _params: DidCloseTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        self.handle_notification(CallKind::DidClose, "didClose", |responses| {
            responses.document_sync.did_close_error.clone()
        })
    }

    fn prepare_call_hierarchy(
        &mut self,
        _params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, LanguageServerError> {
        self.handle_request(
            CallKind::PrepareCallHierarchy,
            "prepareCallHierarchy",
            |responses| responses.call_hierarchy.prepare.clone(),
        )
    }

    fn incoming_calls(
        &mut self,
        _params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LanguageServerError> {
        self.handle_request(CallKind::IncomingCalls, "incomingCalls", |responses| {
            responses.call_hierarchy.incoming.clone()
        })
    }

    fn outgoing_calls(
        &mut self,
        _params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LanguageServerError> {
        self.handle_request(CallKind::OutgoingCalls, "outgoingCalls", |responses| {
            responses.call_hierarchy.outgoing.clone()
        })
    }
}

/// Handle that exposes recorded state for assertions.
#[derive(Clone)]
pub struct RecordingServerHandle {
    shared: Arc<Mutex<RecordingState>>,
}

impl RecordingServerHandle {
    /// Returns the ordered list of calls the server observed.
    pub fn calls(&self) -> Vec<CallKind> {
        with_state(&self.shared, |state| state.calls.clone())
    }
}

fn with_state<R, F>(shared: &Arc<Mutex<RecordingState>>, action: F) -> R
where
    F: FnOnce(&mut RecordingState) -> R,
{
    let mut guard = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    action(&mut guard)
}

/// Static responses returned by the stub server.
#[derive(Debug, Clone)]
pub struct ResponseSet {
    /// Response returned for definition requests.
    pub definition: GotoDefinitionResponse,
    /// Response returned for reference requests.
    pub references: Vec<Location>,
    /// Response returned for diagnostics requests.
    pub diagnostics: Vec<Diagnostic>,
    /// Errors returned for document sync notifications.
    pub document_sync: DocumentSyncErrors,
    /// Responses for call hierarchy requests.
    pub call_hierarchy: CallHierarchyResponses,
}

impl Default for ResponseSet {
    fn default() -> Self {
        Self {
            definition: GotoDefinitionResponse::Array(Vec::new()),
            references: Vec::new(),
            diagnostics: Vec::new(),
            document_sync: DocumentSyncErrors::default(),
            call_hierarchy: CallHierarchyResponses::default(),
        }
    }
}

/// Static responses for call hierarchy requests.
#[derive(Debug, Clone, Default)]
pub struct CallHierarchyResponses {
    /// Response for `textDocument/prepareCallHierarchy`.
    pub prepare: Option<Vec<CallHierarchyItem>>,
    /// Response for `callHierarchy/incomingCalls`.
    pub incoming: Option<Vec<CallHierarchyIncomingCall>>,
    /// Response for `callHierarchy/outgoingCalls`.
    pub outgoing: Option<Vec<CallHierarchyOutgoingCall>>,
}

/// Document sync failures for notifications.
#[derive(Debug, Clone, Default)]
pub struct DocumentSyncErrors {
    /// Error raised when `did_open` is called.
    pub did_open_error: Option<String>,
    /// Error raised when `did_change` is called.
    pub did_change_error: Option<String>,
    /// Error raised when `did_close` is called.
    pub did_close_error: Option<String>,
}

#[derive(Debug)]
struct RecordingState {
    capabilities: ServerCapabilitySet,
    responses: ResponseSet,
    calls: Vec<CallKind>,
    initialised: bool,
    fail_initialise: Option<String>,
}

impl RecordingState {
    fn new(
        capabilities: ServerCapabilitySet,
        responses: ResponseSet,
        fail_initialise: Option<String>,
    ) -> Self {
        Self {
            capabilities,
            responses,
            calls: Vec::new(),
            initialised: false,
            fail_initialise,
        }
    }

    fn record_call(&mut self, kind: CallKind) {
        self.calls.push(kind);
    }
}
