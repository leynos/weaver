//! Process-based language server adapter.

use serde::de::DeserializeOwned;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tracing::{debug, warn};

use super::config::LspServerConfig;
use super::error::AdapterError;
use super::lifecycle::{ADAPTER_TARGET, terminate_child};
use super::messaging;
use super::state::ProcessState;
use super::transport::StdioTransport;
use crate::Language;

/// A language server adapter that spawns and communicates with an external process.
///
/// This adapter spawns a child process and communicates via JSON-RPC 2.0
/// over stdin/stdout with LSP header framing.
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

    /// Sends a request and waits for a response.
    pub(super) fn send_request<P, R>(&self, method: &str, params: P) -> Result<R, AdapterError>
    where
        P: serde::Serialize,
        R: DeserializeOwned,
    {
        self.with_running_transport(|transport| messaging::send_request(transport, method, params))
    }

    /// Sends a notification (no response expected).
    pub(super) fn send_notification<P>(&self, method: &str, params: P) -> Result<(), AdapterError>
    where
        P: serde::Serialize,
    {
        self.with_running_transport(|transport| {
            messaging::send_notification(transport, method, params)
        })
    }

    /// Sends a request that may return null as a valid response.
    pub(super) fn send_request_optional<P, R>(
        &self,
        method: &str,
        params: P,
    ) -> Result<Option<R>, AdapterError>
    where
        P: serde::Serialize,
        R: DeserializeOwned,
    {
        self.with_running_transport(|transport| {
            messaging::send_request_optional(transport, method, params)
        })
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
            terminate_child(&mut child, self.language);
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
