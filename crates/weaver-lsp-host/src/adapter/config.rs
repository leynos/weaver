//! Configuration for process-based language server adapters.

use std::path::PathBuf;
use std::time::Duration;

use crate::Language;

/// Default timeout for initialization handshake.
const DEFAULT_INIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default timeout for individual requests.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Default timeout for graceful shutdown.
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for spawning a language server process.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// The executable path or command name.
    pub command: PathBuf,
    /// Arguments to pass to the language server.
    pub args: Vec<String>,
    /// Working directory for the spawned process.
    pub working_dir: Option<PathBuf>,
    /// Timeout for the initialization handshake.
    pub init_timeout: Duration,
    /// Timeout for individual requests.
    pub request_timeout: Duration,
    /// Timeout for graceful shutdown.
    pub shutdown_timeout: Duration,
}

impl LspServerConfig {
    /// Default configuration for Rust (`rust-analyzer`).
    ///
    /// Expects `rust-analyzer` to be available in PATH.
    #[must_use]
    pub fn rust_default() -> Self {
        Self {
            command: PathBuf::from("rust-analyzer"),
            args: Vec::new(),
            working_dir: None,
            init_timeout: DEFAULT_INIT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
        }
    }

    /// Default configuration for Python (`pyrefly lsp`).
    ///
    /// Expects `pyrefly` to be available in PATH.
    #[must_use]
    pub fn python_default() -> Self {
        Self {
            command: PathBuf::from("pyrefly"),
            args: vec!["lsp".to_string()],
            working_dir: None,
            init_timeout: DEFAULT_INIT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
        }
    }

    /// Default configuration for TypeScript (`tsgo --lsp`).
    ///
    /// Expects `tsgo` to be available in PATH.
    #[must_use]
    pub fn typescript_default() -> Self {
        Self {
            command: PathBuf::from("tsgo"),
            args: vec!["--lsp".to_string()],
            working_dir: None,
            init_timeout: DEFAULT_INIT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
        }
    }

    /// Returns the default configuration for a given language.
    #[must_use]
    pub fn for_language(language: Language) -> Self {
        match language {
            Language::Rust => Self::rust_default(),
            Language::Python => Self::python_default(),
            Language::TypeScript => Self::typescript_default(),
        }
    }

    /// Sets a custom working directory.
    #[must_use]
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Sets a custom initialization timeout.
    #[must_use]
    pub const fn with_init_timeout(mut self, timeout: Duration) -> Self {
        self.init_timeout = timeout;
        self
    }

    /// Sets a custom request timeout.
    #[must_use]
    pub const fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Sets a custom shutdown timeout.
    #[must_use]
    pub const fn with_shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn rust_default_uses_rust_analyzer() {
        let config = LspServerConfig::rust_default();

        assert_eq!(config.command, PathBuf::from("rust-analyzer"));
        assert!(config.args.is_empty());
    }

    #[rstest]
    fn python_default_uses_pyrefly_lsp() {
        let config = LspServerConfig::python_default();

        assert_eq!(config.command, PathBuf::from("pyrefly"));
        assert_eq!(config.args, vec!["lsp"]);
    }

    #[rstest]
    fn typescript_default_uses_tsgo_lsp() {
        let config = LspServerConfig::typescript_default();

        assert_eq!(config.command, PathBuf::from("tsgo"));
        assert_eq!(config.args, vec!["--lsp"]);
    }

    #[rstest]
    #[case(Language::Rust, "rust-analyzer")]
    #[case(Language::Python, "pyrefly")]
    #[case(Language::TypeScript, "tsgo")]
    fn for_language_returns_correct_command(#[case] language: Language, #[case] expected: &str) {
        let config = LspServerConfig::for_language(language);

        assert_eq!(config.command, PathBuf::from(expected));
    }

    #[rstest]
    fn builder_methods_work() {
        let config = LspServerConfig::rust_default()
            .with_working_dir("/workspace")
            .with_init_timeout(Duration::from_secs(60))
            .with_request_timeout(Duration::from_secs(20))
            .with_shutdown_timeout(Duration::from_secs(10));

        assert_eq!(config.working_dir, Some(PathBuf::from("/workspace")));
        assert_eq!(config.init_timeout, Duration::from_secs(60));
        assert_eq!(config.request_timeout, Duration::from_secs(20));
        assert_eq!(config.shutdown_timeout, Duration::from_secs(10));
    }
}
