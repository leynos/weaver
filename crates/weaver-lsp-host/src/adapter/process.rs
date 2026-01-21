//! Process-based language server adapter implementing the `LanguageServer` trait.

use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentDiagnosticParams, DocumentDiagnosticReport, GotoDefinitionParams,
    GotoDefinitionResponse, InitializeParams, InitializeResult, InitializedParams, ReferenceParams,
    TextDocumentIdentifier, Uri,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tracing::{debug, warn};

use super::config::LspServerConfig;
use super::error::AdapterError;
use super::jsonrpc::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use super::transport::StdioTransport;
use crate::Language;
use crate::server::{LanguageServer, LanguageServerError, ServerCapabilitySet};

/// Log target for adapter operations.
const ADAPTER_TARGET: &str = "weaver_lsp_host::adapter";

/// A language server adapter that spawns and communicates with an external process.
///
/// This adapter implements the [`LanguageServer`] trait by spawning a child process
/// and communicating via JSON-RPC 2.0 over stdin/stdout with LSP header framing.
///
/// # Example
///
/// ```ignore
/// use weaver_lsp_host::adapter::ProcessLanguageServer;
/// use weaver_lsp_host::Language;
///
/// let mut server = ProcessLanguageServer::new(Language::Rust);
/// let capabilities = server.initialize()?;
/// ```
pub struct ProcessLanguageServer {
    language: Language,
    config: LspServerConfig,
    state: Mutex<ProcessState>,
}

/// Internal state of the language server process.
enum ProcessState {
    /// Process has not been started.
    NotStarted,
    /// Process is running and ready for communication.
    Running {
        /// The child process handle.
        child: Child,
        /// The transport for JSON-RPC communication.
        transport: StdioTransport,
    },
    /// Process has been stopped.
    Stopped,
}

impl ProcessLanguageServer {
    /// Creates a new adapter for the given language using default configuration.
    #[must_use]
    pub fn new(language: Language) -> Self {
        Self {
            language,
            config: LspServerConfig::for_language(language),
            state: Mutex::new(ProcessState::NotStarted),
        }
    }

    /// Creates a new adapter with custom configuration.
    #[must_use]
    pub fn with_config(language: Language, config: LspServerConfig) -> Self {
        Self {
            language,
            config,
            state: Mutex::new(ProcessState::NotStarted),
        }
    }

    /// Returns the language this adapter serves.
    #[must_use]
    pub fn language(&self) -> Language {
        self.language
    }

    /// Spawns the language server process.
    fn spawn_process(&self) -> Result<(Child, StdioTransport), AdapterError> {
        debug!(
            target: ADAPTER_TARGET,
            language = %self.language,
            command = %self.config.command.display(),
            args = ?self.config.args,
            "spawning language server process"
        );

        let mut command = Command::new(&self.config.command);
        command
            .args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Some(dir) = &self.config.working_dir {
            command.current_dir(dir);
        }

        let mut child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AdapterError::BinaryNotFound {
                    command: self.config.command.display().to_string(),
                    source: e,
                }
            } else {
                AdapterError::SpawnFailed {
                    message: format!("failed to start {}", self.config.command.display()),
                    source: e,
                }
            }
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AdapterError::SpawnFailed {
                message: "failed to capture stdin".to_string(),
                source: std::io::Error::other("no stdin"),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdapterError::SpawnFailed {
                message: "failed to capture stdout".to_string(),
                source: std::io::Error::other("no stdout"),
            })?;

        let transport = StdioTransport::new(stdout, stdin);

        debug!(
            target: ADAPTER_TARGET,
            language = %self.language,
            pid = child.id(),
            "language server process spawned"
        );

        Ok((child, transport))
    }

    /// Sends a request and waits for a response.
    fn send_request<P, R>(&self, method: &str, params: P) -> Result<R, AdapterError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        let transport = match &mut *state {
            ProcessState::Running { transport, .. } => transport,
            ProcessState::NotStarted | ProcessState::Stopped => {
                return Err(AdapterError::ProcessExited);
            }
        };

        let params_value = serde_json::to_value(params)?;
        let request = JsonRpcRequest::new(method, Some(params_value));
        let request_id = request.id;
        let payload = serde_json::to_vec(&request)?;

        debug!(
            target: ADAPTER_TARGET,
            method = method,
            id = request_id,
            "sending request"
        );

        transport.send(&payload)?;
        let response_bytes = transport.receive()?;
        let response: JsonRpcResponse = serde_json::from_slice(&response_bytes)?;

        // Validate response ID matches
        if response.id != Some(request_id) {
            return Err(AdapterError::InitializationFailed {
                message: format!(
                    "response ID mismatch: expected {}, got {:?}",
                    request_id, response.id
                ),
            });
        }

        if let Some(error) = response.error {
            return Err(AdapterError::from_jsonrpc(error));
        }

        let result = response
            .result
            .ok_or_else(|| AdapterError::InitializationFailed {
                message: "empty result in response".to_string(),
            })?;

        serde_json::from_value(result).map_err(AdapterError::from)
    }

    /// Sends a notification (no response expected).
    fn send_notification<P>(&self, method: &str, params: P) -> Result<(), AdapterError>
    where
        P: Serialize,
    {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        let transport = match &mut *state {
            ProcessState::Running { transport, .. } => transport,
            ProcessState::NotStarted | ProcessState::Stopped => {
                return Err(AdapterError::ProcessExited);
            }
        };

        let params_value = serde_json::to_value(params)?;
        let notification = JsonRpcNotification::new(method, Some(params_value));
        let payload = serde_json::to_vec(&notification)?;

        debug!(
            target: ADAPTER_TARGET,
            method = method,
            "sending notification"
        );

        transport.send(&payload)?;
        Ok(())
    }

    /// Sends a request that may return null as a valid response.
    fn send_request_optional<P, R>(
        &self,
        method: &str,
        params: P,
    ) -> Result<Option<R>, AdapterError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        let transport = match &mut *state {
            ProcessState::Running { transport, .. } => transport,
            ProcessState::NotStarted | ProcessState::Stopped => {
                return Err(AdapterError::ProcessExited);
            }
        };

        let params_value = serde_json::to_value(params)?;
        let request = JsonRpcRequest::new(method, Some(params_value));
        let request_id = request.id;
        let payload = serde_json::to_vec(&request)?;

        debug!(
            target: ADAPTER_TARGET,
            method = method,
            id = request_id,
            "sending request (optional result)"
        );

        transport.send(&payload)?;
        let response_bytes = transport.receive()?;
        let response: JsonRpcResponse = serde_json::from_slice(&response_bytes)?;

        if response.id != Some(request_id) {
            return Err(AdapterError::InitializationFailed {
                message: format!(
                    "response ID mismatch: expected {}, got {:?}",
                    request_id, response.id
                ),
            });
        }

        if let Some(error) = response.error {
            return Err(AdapterError::from_jsonrpc(error));
        }

        match response.result {
            Some(Value::Null) | None => Ok(None),
            Some(value) => {
                let result = serde_json::from_value(value)?;
                Ok(Some(result))
            }
        }
    }

    /// Performs graceful shutdown of the language server.
    ///
    /// Sends a `shutdown` request followed by an `exit` notification,
    /// then waits for the process to terminate (with timeout).
    pub fn shutdown(&self) -> Result<(), AdapterError> {
        debug!(
            target: ADAPTER_TARGET,
            language = %self.language,
            "initiating graceful shutdown"
        );

        // Try to send shutdown request
        let shutdown_result: Result<Value, _> = self.send_request("shutdown", ());

        if let Err(e) = shutdown_result {
            debug!(
                target: ADAPTER_TARGET,
                language = %self.language,
                error = %e,
                "shutdown request failed, proceeding with termination"
            );
        }

        // Send exit notification (best effort)
        let _ = self.send_notification("exit", ());

        // Wait for process to exit
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        if let ProcessState::Running { mut child, .. } =
            std::mem::replace(&mut *state, ProcessState::Stopped)
        {
            self.wait_for_exit(&mut child);
        }

        Ok(())
    }

    /// Waits for the child process to exit, killing it if necessary.
    fn wait_for_exit(&self, child: &mut Child) {
        match child.try_wait() {
            Ok(Some(status)) => {
                debug!(
                    target: ADAPTER_TARGET,
                    language = %self.language,
                    ?status,
                    "language server exited"
                );
            }
            Ok(None) => {
                self.wait_with_timeout_then_kill(child);
            }
            Err(e) => {
                warn!(
                    target: ADAPTER_TARGET,
                    language = %self.language,
                    error = %e,
                    "failed to check process status, killing"
                );
                let _ = child.kill();
            }
        }
    }

    /// Waits for the shutdown timeout then kills the process if still running.
    fn wait_with_timeout_then_kill(&self, child: &mut Child) {
        std::thread::sleep(self.config.shutdown_timeout);
        match child.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                warn!(
                    target: ADAPTER_TARGET,
                    language = %self.language,
                    "language server did not exit gracefully, killing"
                );
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

// SAFETY: ProcessLanguageServer is Send because:
// - The inner state is protected by Mutex
// - Child process handles are Send
// - All fields are Send
//
// Note: The LanguageServer trait requires Send, and our implementation
// uses Mutex to protect all mutable state.
unsafe impl Send for ProcessLanguageServer {}

impl LanguageServer for ProcessLanguageServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        debug!(
            target: ADAPTER_TARGET,
            language = %self.language,
            "initializing language server"
        );

        // Spawn process
        let (child, transport) = self.spawn_process().map_err(|e| {
            LanguageServerError::with_source(
                format!("failed to spawn {} language server", self.language),
                e,
            )
        })?;

        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            *state = ProcessState::Running { child, transport };
        }

        // Send initialize request
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            capabilities: lsp_types::ClientCapabilities::default(),
            ..Default::default()
        };

        let result: InitializeResult = self
            .send_request("initialize", params)
            .map_err(|e| LanguageServerError::with_source("initialization handshake failed", e))?;

        // Send initialized notification
        self.send_notification("initialized", InitializedParams {})
            .map_err(|e| {
                LanguageServerError::with_source("failed to send initialized notification", e)
            })?;

        // Extract capabilities
        let caps = &result.capabilities;

        let definition_supported = caps.definition_provider.is_some();
        let references_supported = caps.references_provider.is_some();
        let diagnostics_supported = caps.diagnostic_provider.is_some();
        let call_hierarchy_supported = caps.call_hierarchy_provider.is_some();

        debug!(
            target: ADAPTER_TARGET,
            language = %self.language,
            definition = definition_supported,
            references = references_supported,
            diagnostics = diagnostics_supported,
            call_hierarchy = call_hierarchy_supported,
            "language server initialized with capabilities"
        );

        Ok(ServerCapabilitySet::new(
            definition_supported,
            references_supported,
            diagnostics_supported,
        )
        .with_call_hierarchy(call_hierarchy_supported))
    }

    fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LanguageServerError> {
        self.send_request_optional("textDocument/definition", params)
            .map(|opt| opt.unwrap_or(GotoDefinitionResponse::Array(vec![])))
            .map_err(|e| LanguageServerError::with_source("definition request failed", e))
    }

    fn references(
        &mut self,
        params: ReferenceParams,
    ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
        self.send_request_optional("textDocument/references", params)
            .map(|opt| opt.unwrap_or_default())
            .map_err(|e| LanguageServerError::with_source("references request failed", e))
    }

    fn diagnostics(&mut self, uri: Uri) -> Result<Vec<Diagnostic>, LanguageServerError> {
        // Use pull-based diagnostics (textDocument/diagnostic)
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result: DocumentDiagnosticReport = self
            .send_request("textDocument/diagnostic", params)
            .map_err(|e| LanguageServerError::with_source("diagnostics request failed", e))?;

        // Extract diagnostics from the report
        let diagnostics = match result {
            DocumentDiagnosticReport::Full(full) => full.full_document_diagnostic_report.items,
            DocumentDiagnosticReport::Unchanged(_) => Vec::new(),
        };

        Ok(diagnostics)
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<(), LanguageServerError> {
        self.send_notification("textDocument/didOpen", params)
            .map_err(|e| LanguageServerError::with_source("didOpen notification failed", e))
    }

    fn did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        self.send_notification("textDocument/didChange", params)
            .map_err(|e| LanguageServerError::with_source("didChange notification failed", e))
    }

    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<(), LanguageServerError> {
        self.send_notification("textDocument/didClose", params)
            .map_err(|e| LanguageServerError::with_source("didClose notification failed", e))
    }

    fn prepare_call_hierarchy(
        &mut self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, LanguageServerError> {
        self.send_request_optional("textDocument/prepareCallHierarchy", params)
            .map_err(|e| LanguageServerError::with_source("prepareCallHierarchy request failed", e))
    }

    fn incoming_calls(
        &mut self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LanguageServerError> {
        self.send_request_optional("callHierarchy/incomingCalls", params)
            .map_err(|e| LanguageServerError::with_source("incomingCalls request failed", e))
    }

    fn outgoing_calls(
        &mut self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LanguageServerError> {
        self.send_request_optional("callHierarchy/outgoingCalls", params)
            .map_err(|e| LanguageServerError::with_source("outgoingCalls request failed", e))
    }
}

impl Drop for ProcessLanguageServer {
    fn drop(&mut self) {
        // Best-effort graceful shutdown
        if let Err(e) = self.shutdown() {
            warn!(
                target: ADAPTER_TARGET,
                language = %self.language,
                error = %e,
                "failed to gracefully shut down language server"
            );
        }
    }
}

impl std::fmt::Debug for ProcessLanguageServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_desc = match self.state.lock() {
            Ok(guard) => match &*guard {
                ProcessState::NotStarted => "not_started",
                ProcessState::Running { child, .. } => {
                    return f
                        .debug_struct("ProcessLanguageServer")
                        .field("language", &self.language)
                        .field("state", &format!("running (pid: {})", child.id()))
                        .finish();
                }
                ProcessState::Stopped => "stopped",
            },
            Err(_) => "poisoned",
        };

        f.debug_struct("ProcessLanguageServer")
            .field("language", &self.language)
            .field("state", &state_desc)
            .finish()
    }
}
