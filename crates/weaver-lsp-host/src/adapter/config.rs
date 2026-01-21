//! Configuration for process-based language server adapters.

use std::path::PathBuf;

use crate::Language;

/// Configuration for spawning a language server process.
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// The executable path or command name.
    pub command: PathBuf,
    /// Arguments to pass to the language server.
    pub args: Vec<String>,
    /// Working directory for the spawned process.
    pub working_dir: Option<PathBuf>,
}

impl LspServerConfig {
    fn default_config(command: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            working_dir: None,
        }
    }

    /// Default configuration for Rust (`rust-analyzer`).
    ///
    /// Expects `rust-analyzer` to be available in PATH.
    #[must_use]
    pub fn rust_default() -> Self {
        Self::default_config("rust-analyzer", Vec::new())
    }

    /// Default configuration for Python (`pyrefly lsp`).
    ///
    /// Expects `pyrefly` to be available in PATH.
    #[must_use]
    pub fn python_default() -> Self {
        Self::default_config("pyrefly", vec!["lsp".to_string()])
    }

    /// Default configuration for TypeScript (`tsgo --lsp`).
    ///
    /// Expects `tsgo` to be available in PATH.
    #[must_use]
    pub fn typescript_default() -> Self {
        Self::default_config("tsgo", vec!["--lsp".to_string()])
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
        let config = LspServerConfig::rust_default().with_working_dir("/workspace");

        assert_eq!(config.working_dir, Some(PathBuf::from("/workspace")));
    }
}
