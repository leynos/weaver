//! Process-based language server adapter.

use std::{
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::de::DeserializeOwned;
use tracing::{debug, warn};

use super::{
    config::LspServerConfig,
    error::AdapterError,
    lifecycle::{ADAPTER_TARGET, terminate_child},
    messaging,
    state::ProcessState,
    transport::StdioTransport,
};
use crate::Language;

/// How long a server that stopped answering is left alone before the adapter
/// will start a fresh process for it.
///
/// Without a cooldown, every request in a batch (for example one hover per card
/// in a graph slice) would start a new process and wait out its own deadline,
/// multiplying the stall by the batch size.
const RESTART_COOLDOWN: Duration = Duration::from_secs(60);

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
    /// When the process was last stopped for not answering in time.
    unresponsive_since: Mutex<Option<Instant>>,
}

impl ProcessLanguageServer {
    /// Creates a new adapter for the given language using default configuration.
    #[must_use]
    pub fn new(language: Language) -> Self {
        Self {
            language,
            config: LspServerConfig::for_language(language),
            state: Mutex::new(ProcessState::NotStarted),
            unresponsive_since: Mutex::new(None),
        }
    }

    /// Creates a new adapter with custom configuration.
    #[must_use]
    pub fn with_config(language: Language, config: LspServerConfig) -> Self {
        Self {
            language,
            config,
            state: Mutex::new(ProcessState::NotStarted),
            unresponsive_since: Mutex::new(None),
        }
    }

    /// Returns the language this adapter serves.
    #[must_use]
    pub fn language(&self) -> Language { self.language }

    /// Spawns the language server process.
    pub(super) fn spawn_process(&self) -> Result<(Child, StdioTransport), AdapterError> {
        self.refuse_while_cooling_down()?;
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

        let transport =
            StdioTransport::new(stdout, stdin).with_response_timeout(self.config.response_timeout);

        debug!(
            target: ADAPTER_TARGET,
            language = %self.language,
            pid = child.id(),
            "language server process spawned"
        );

        Ok((child, transport))
    }

    /// Fails fast while a server that stopped answering is still cooling down.
    fn refuse_while_cooling_down(&self) -> Result<(), AdapterError> {
        let since = *self
            .unresponsive_since
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match since {
            Some(at) if at.elapsed() < RESTART_COOLDOWN => Err(AdapterError::Unresponsive {
                cooldown_secs: RESTART_COOLDOWN.as_secs(),
            }),
            _ => Ok(()),
        }
    }

    /// Kills a server that missed its deadline and starts the restart cooldown.
    ///
    /// The stream cannot be trusted afterwards: a late reply would be read as
    /// the answer to the next request. Killing the process also closes its
    /// stdout, which ends the transport's reader thread.
    fn stop_unresponsive(&self) {
        warn!(
            target: ADAPTER_TARGET,
            language = %self.language,
            timeout_secs = self.config.response_timeout.as_secs_f32(),
            "language server did not respond in time; stopping it"
        );
        *self
            .unresponsive_since
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(Instant::now());
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let ProcessState::Running { mut child, .. } =
            std::mem::replace(&mut *state, ProcessState::Stopped)
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Stops the server if `result` shows it missed its deadline, then returns `result`.
    fn stop_if_timed_out<T>(&self, result: Result<T, AdapterError>) -> Result<T, AdapterError> {
        if result.as_ref().is_err_and(AdapterError::is_timeout) {
            self.stop_unresponsive();
        }
        result
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

    /// Generic helper to execute a messaging operation with running transport.
    fn execute_messaging_operation<P, R, F>(
        &self,
        method: &str,
        params: P,
        operation: F,
    ) -> Result<R, AdapterError>
    where
        P: serde::Serialize,
        F: FnOnce(&mut StdioTransport, &str, P) -> Result<R, AdapterError>,
    {
        self.with_running_transport(|transport| operation(transport, method, params))
    }

    /// Sends a request and waits for a response.
    pub(super) fn send_request<P, R>(&self, method: &str, params: P) -> Result<R, AdapterError>
    where
        P: serde::Serialize,
        R: DeserializeOwned,
    {
        let result = self.execute_messaging_operation(method, params, messaging::send_request);
        self.stop_if_timed_out(result)
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
        let result =
            self.execute_messaging_operation(method, params, messaging::send_request_optional);
        self.stop_if_timed_out(result)
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
