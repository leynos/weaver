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
    #[case(Language::Rust, "rust-analyzer", &[] as &[&str])]
    #[case(Language::Python, "pyrefly", &["lsp"])]
    #[case(Language::TypeScript, "tsgo", &["--lsp"])]
    fn default_config_for_language(
        #[case] language: Language,
        #[case] expected_command: &str,
        #[case] expected_args: &[&str],
    ) {
        let config = LspServerConfig::for_language(language);

        assert_eq!(config.command, PathBuf::from(expected_command));
        assert_eq!(
            config.args,
            expected_args
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[rstest]
    fn builder_methods_work() {
        let config = LspServerConfig::rust_default().with_working_dir("/workspace");

        assert_eq!(config.working_dir, Some(PathBuf::from("/workspace")));
    }
}
