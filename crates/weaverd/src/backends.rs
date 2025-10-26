//! Lazy orchestration of Semantic Fusion backends.
//!
//! The daemon owns a set of backends that contribute to the Semantic Fusion
//! engine. Each backend is started on demand the first time it is requested.
//! This minimises boot latency and avoids paying the cost of services that are
//! not required for a given command sequence.

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use thiserror::Error;

use weaver_config::Config;

/// Semantic Fusion backends managed by the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    /// The Language Server Protocol layer.
    Semantic,
    /// The Tree-sitter syntax layer.
    Syntactic,
    /// Relational intelligence and call-graph analysis.
    Relational,
}

impl fmt::Display for BackendKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Semantic => "semantic",
            Self::Syntactic => "syntactic",
            Self::Relational => "relational",
        };
        formatter.write_str(label)
    }
}

/// Error returned when parsing a backend kind fails.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("unsupported backend kind: {0}")]
pub struct BackendKindParseError(String);

impl BackendKindParseError {
    /// Creates a parse error describing the unsupported value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the offending value that could not be parsed.
    #[must_use]
    pub fn value(&self) -> &str {
        self.0.as_str()
    }
}

impl FromStr for BackendKind {
    type Err = BackendKindParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "semantic" => Ok(Self::Semantic),
            "syntactic" => Ok(Self::Syntactic),
            "relational" => Ok(Self::Relational),
            other => Err(BackendKindParseError::new(other)),
        }
    }
}

/// Errors surfaced when a backend fails to start.
#[derive(Debug, Error)]
#[error("backend {kind} failed to start: {message}")]
pub struct BackendStartupError {
    /// Kind of backend that failed.
    pub kind: BackendKind,
    message: String,
    /// Optional source error reported by the backend implementation.
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl BackendStartupError {
    /// Builds an error without an underlying source.
    #[must_use]
    pub fn new(kind: BackendKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Builds an error that wraps an underlying source.
    #[must_use]
    pub fn with_source(
        kind: BackendKind,
        message: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Human-readable message describing the failure.
    #[must_use]
    pub fn message(&self) -> &str {
        self.message.as_str()
    }
}

/// Trait implemented by types capable of starting a backend.
pub trait BackendProvider {
    /// Starts the specified backend using the resolved configuration.
    fn start_backend(&self, kind: BackendKind, config: &Config) -> Result<(), BackendStartupError>;
}

/// Registry that tracks which backends have already been started.
#[derive(Debug)]
pub struct FusionBackends<P> {
    config: Config,
    provider: P,
    started: HashSet<BackendKind>,
}

impl<P> FusionBackends<P> {
    /// Builds a new registry over the supplied provider.
    #[must_use]
    pub fn new(config: Config, provider: P) -> Self {
        Self {
            config,
            provider,
            started: HashSet::new(),
        }
    }

    /// Returns a reference to the resolved configuration.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns a mutable reference to the backend provider.
    #[must_use]
    pub fn provider_mut(&mut self) -> &mut P {
        &mut self.provider
    }

    /// Ensures the specified backend has been started.
    ///
    /// Successful calls are idempotent: once a backend starts, subsequent
    /// invocations return immediately without consulting the provider again.
    /// When the provider reports an error, the backend remains outside the
    /// started set, so later calls retry `BackendProvider::start_backend`.
    /// This favours eventual success for transient faults at the cost of
    /// possible retries and assumes providers classify identical failures
    /// deterministically.
    pub fn ensure_started(&mut self, kind: BackendKind) -> Result<(), BackendStartupError>
    where
        P: BackendProvider,
    {
        if self.started.contains(&kind) {
            return Ok(());
        }

        self.provider.start_backend(kind, &self.config)?;
        self.started.insert(kind);
        Ok(())
    }

    /// Returns `true` when the backend has already been started.
    #[must_use]
    pub fn is_started(&self, kind: BackendKind) -> bool {
        self.started.contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use weaver_config::SocketEndpoint;

    #[derive(Clone, Debug, Default)]
    struct RecordingProvider {
        calls: std::sync::Arc<std::sync::Mutex<Vec<BackendKind>>>,
    }

    impl RecordingProvider {
        fn calls(&self) -> Vec<BackendKind> {
            self.calls
                .lock()
                .expect("recording provider mutex poisoned")
                .clone()
        }
    }

    impl BackendProvider for RecordingProvider {
        fn start_backend(
            &self,
            kind: BackendKind,
            _config: &Config,
        ) -> Result<(), BackendStartupError> {
            let mut calls = self
                .calls
                .lock()
                .expect("recording provider mutex poisoned");
            calls.push(kind);
            Ok(())
        }
    }

    #[fixture]
    fn config() -> Config {
        Config {
            daemon_socket: SocketEndpoint::unix("/tmp/weaver-tests/socket.sock"),
            ..Config::default()
        }
    }

    #[fixture]
    fn provider() -> RecordingProvider {
        RecordingProvider::default()
    }

    #[fixture]
    fn fusion_backends(
        config: Config,
        provider: RecordingProvider,
    ) -> (FusionBackends<RecordingProvider>, RecordingProvider) {
        let inspector = provider.clone();
        (FusionBackends::new(config, provider), inspector)
    }

    #[rstest]
    fn ensures_backend_starts_only_once(
        fusion_backends: (FusionBackends<RecordingProvider>, RecordingProvider),
    ) {
        let (mut backends, inspector) = fusion_backends;
        backends
            .ensure_started(BackendKind::Semantic)
            .expect("start backend");
        backends
            .ensure_started(BackendKind::Semantic)
            .expect("start backend");

        assert_eq!(inspector.calls().as_slice(), &[BackendKind::Semantic]);
    }
}
