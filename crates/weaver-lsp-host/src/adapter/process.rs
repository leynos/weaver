//! Process-based language server adapter implementing the `LanguageServer` trait.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

use super::config::LspServerConfig;
use super::error::AdapterError;
use super::jsonrpc::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use super::state::ProcessState;
use super::transport::StdioTransport;
use crate::Language;

/// Log target for adapter operations.
pub(super) const ADAPTER_TARGET: &str = "weaver_lsp_host::adapter";

/// Maximum number of iterations to wait for a matching JSON-RPC response.
const MAX_RESPONSE_ITERATIONS: usize = 100;

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
    pub(super) fn spawn_process(&self) -> Result<(Child, StdioTransport), AdapterError> {
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
                    source: std::sync::Arc::new(e),
                }
            } else {
                AdapterError::SpawnFailed {
                    message: format!("failed to start {}", self.config.command.display()),
                    source: std::sync::Arc::new(e),
                }
            }
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AdapterError::SpawnFailed {
                message: "failed to capture stdin".to_string(),
                source: std::sync::Arc::new(std::io::Error::other("no stdin")),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdapterError::SpawnFailed {
                message: "failed to capture stdout".to_string(),
                source: std::sync::Arc::new(std::io::Error::other("no stdout")),
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

    /// Accesses the running transport with the state lock held.
    pub(super) fn with_running_transport<F, T>(&self, f: F) -> Result<T, AdapterError>
    where
        F: FnOnce(&mut StdioTransport) -> Result<T, AdapterError>,
    {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        // Recover from poisoning to attempt graceful shutdown even after a panic

        let transport = match &mut *state {
            ProcessState::Running { transport, .. } => transport,
            ProcessState::NotStarted | ProcessState::Stopped => {
                return Err(AdapterError::ProcessExited);
            }
        };

        f(transport)
    }

    /// Receives messages from transport until a matching response is found.
    ///
    /// Handles interleaved JSON-RPC messages (notifications, server requests, and responses)
    /// by looping and processing each message until a response with matching ID is found.
    ///
    /// Uses a bounded iteration limit to prevent blocking indefinitely on interleaved messages.
    #[expect(
        clippy::excessive_nesting,
        reason = "nested match arms required to handle multiple JSON-RPC message types"
    )]
    pub(super) fn receive_response_for_request(
        transport: &mut StdioTransport,
        request_id: i64,
    ) -> Result<JsonRpcResponse, AdapterError> {
        let mut iteration_count = 0;
        loop {
            if iteration_count >= MAX_RESPONSE_ITERATIONS {
                warn!(
                    target: ADAPTER_TARGET,
                    request_id,
                    max_iterations = MAX_RESPONSE_ITERATIONS,
                    "giving up on response after reaching maximum iterations"
                );
                return Err(AdapterError::MaxResponseIterations { request_id });
            }
            iteration_count += 1;

            let message_bytes = transport.receive()?;

            match JsonRpcMessage::from_bytes(&message_bytes)? {
                JsonRpcMessage::Response(resp) => {
                    if resp.id == Some(request_id) {
                        return Ok(resp);
                    }
                    warn!(
                        target: ADAPTER_TARGET,
                        expected = request_id,
                        received = ?resp.id,
                        "skipping response with non-matching ID"
                    );
                }
                JsonRpcMessage::ServerRequest(req) => {
                    warn!(
                        target: ADAPTER_TARGET,
                        method = %req.method,
                        id = req.id,
                        "ignoring server-initiated request (not yet implemented)"
                    );
                }
                JsonRpcMessage::Notification(notif) => {
                    debug!(
                        target: ADAPTER_TARGET,
                        method = %notif.method,
                        "skipping server notification"
                    );
                }
            }
        }
    }

    /// Sends a request and receives the raw JSON-RPC response.
    pub(super) fn send_request_raw<P>(
        &self,
        method: &str,
        params: P,
    ) -> Result<JsonRpcResponse, AdapterError>
    where
        P: Serialize,
    {
        self.with_running_transport(|transport| {
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
            let response = Self::receive_response_for_request(transport, request_id)?;

            if let Some(error) = response.error {
                return Err(AdapterError::from_jsonrpc(error));
            }

            Ok(response)
        })
    }

    /// Sends a request and waits for a response.
    pub(super) fn send_request<P, R>(&self, method: &str, params: P) -> Result<R, AdapterError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let response = self.send_request_raw(method, params)?;
        let result = response
            .result
            .ok_or_else(|| AdapterError::InitializationFailed {
                message: "empty result in response".to_string(),
            })?;
        serde_json::from_value(result).map_err(AdapterError::from)
    }

    /// Sends a notification (no response expected).
    pub(super) fn send_notification<P>(&self, method: &str, params: P) -> Result<(), AdapterError>
    where
        P: Serialize,
    {
        self.with_running_transport(|transport| {
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
        })
    }

    /// Sends a request that may return null as a valid response.
    pub(super) fn send_request_optional<P, R>(
        &self,
        method: &str,
        params: P,
    ) -> Result<Option<R>, AdapterError>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let response = self.send_request_raw(method, params)?;
        match response.result {
            Some(Value::Null) | None => Ok(None),
            Some(value) => Ok(Some(serde_json::from_value(value)?)),
        }
    }

    /// Performs graceful shutdown of the language server.
    ///
    /// Sends a `shutdown` request followed by an `exit` notification,
    /// then waits for the process to terminate.
    pub fn shutdown(&self) -> Result<(), AdapterError> {
        debug!(
            target: ADAPTER_TARGET,
            language = %self.language,
            "initiating graceful shutdown"
        );

        if let Err(e) = self.send_request::<_, serde_json::Value>("shutdown", ()) {
            debug!(
                target: ADAPTER_TARGET,
                language = %self.language,
                operation = "shutdown",
                error = ?e,
                "shutdown request failed"
            );
        }

        if let Err(e) = self.send_notification("exit", ()) {
            debug!(
                target: ADAPTER_TARGET,
                language = %self.language,
                operation = "exit",
                error = ?e,
                "exit notification failed"
            );
        }

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        // Recover from poisoning to attempt graceful shutdown even after a panic
        if let ProcessState::Running { mut child, .. } =
            std::mem::replace(&mut *state, ProcessState::Stopped)
        {
            self.terminate_child(&mut child);
        }

        Ok(())
    }

    /// Sets the process to running state with given child and transport.
    pub(super) fn set_running_state(&self, child: Child, transport: StdioTransport) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        *state = ProcessState::Running { child, transport };
    }

    /// Waits for the child process to exit, killing it if necessary.
    fn terminate_child(&self, child: &mut Child) {
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
                warn!(
                    target: ADAPTER_TARGET,
                    language = %self.language,
                    "language server did not exit gracefully, waiting before killing"
                );
                thread::sleep(Duration::from_millis(200));
                match child.try_wait() {
                    Ok(Some(status)) => {
                        debug!(
                            target: ADAPTER_TARGET,
                            language = %self.language,
                            ?status,
                            "language server exited during grace period"
                        );
                    }
                    Ok(None) | Err(_) => {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            }
            Err(e) => {
                warn!(
                    target: ADAPTER_TARGET,
                    language = %self.language,
                    error = %e,
                    "failed to check process status, waiting before killing"
                );
                thread::sleep(Duration::from_millis(200));
                match child.try_wait() {
                    Ok(Some(status)) => {
                        debug!(
                            target: ADAPTER_TARGET,
                            language = %self.language,
                            ?status,
                            "language server exited during grace period"
                        );
                    }
                    Ok(None) | Err(_) => {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            }
        }
    }
}

impl Drop for ProcessLanguageServer {
    fn drop(&mut self) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(poison) => poison.into_inner(),
        };

        // Recover from poisoning to attempt graceful shutdown even after a panic
        if let ProcessState::Running { mut child, .. } =
            std::mem::replace(&mut *state, ProcessState::Stopped)
        {
            if let Err(e) = child.kill() {
                warn!(
                    target: ADAPTER_TARGET,
                    language = %self.language,
                    error = %e,
                    "failed to kill language server process on drop"
                );
            } else {
                let _ = child.wait();
            }
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
