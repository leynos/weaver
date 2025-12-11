//! Tree-sitter based syntactic validation for the Double-Lock harness.
//!
//! This module provides [`TreeSitterSyntacticLock`], which validates that
//! modified files produce valid syntax trees. It integrates with the
//! safety harness in `weaverd` to prevent syntactically invalid code from
//! being committed.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::error::SyntaxError;
use crate::language::SupportedLanguage;
use crate::parser::Parser;

/// Tree-sitter based syntactic validation.
///
/// This validator parses modified files using Tree-sitter and reports any
/// syntax errors found. Files with unrecognised extensions are skipped
/// (passed through), allowing non-code files to coexist in the codebase.
///
/// # Thread Safety
///
/// This type is thread-safe and can be shared across threads. Internal
/// parser state is protected by a mutex.
pub struct TreeSitterSyntacticLock {
    /// Cached parsers for each language.
    parsers: Mutex<HashMap<SupportedLanguage, Parser>>,
}

impl TreeSitterSyntacticLock {
    /// Creates a new syntactic lock.
    ///
    /// Parsers for each language are created lazily on first use.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parsers: Mutex::new(HashMap::new()),
        }
    }

    /// Validates a single file's content.
    ///
    /// Returns a list of syntax errors found in the file. An empty list
    /// indicates the file is syntactically valid.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path (used for language detection)
    /// * `content` - The file content to validate
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<...>)` - List of validation failures (empty if valid)
    /// * `Err(...)` - If language detection or parser initialisation fails
    ///
    /// # Errors
    ///
    /// Returns an error if the parser for the detected language cannot be
    /// initialised, or if the internal parser lock is poisoned.
    pub fn validate_file(
        &self,
        path: &Path,
        content: &str,
    ) -> Result<Vec<ValidationFailure>, SyntaxError> {
        // Detect language from file extension
        let Some(language) = SupportedLanguage::from_path(path) else {
            // Unknown file type - skip validation (pass through)
            return Ok(Vec::new());
        };

        // Get or create parser for this language
        let mut parsers = self
            .parsers
            .lock()
            .map_err(|_| SyntaxError::parser_init(language, "lock poisoned"))?;

        let parser = if let Some(p) = parsers.get_mut(&language) {
            p
        } else {
            let new_parser = Parser::new(language)?;
            parsers.insert(language, new_parser);
            parsers.get_mut(&language).ok_or_else(|| {
                SyntaxError::parser_init(language, "parser not found after insert")
            })?
        };

        // Parse the content
        let result = parser.parse(content)?;

        // Collect errors
        let failures: Vec<ValidationFailure> = result
            .errors()
            .into_iter()
            .map(|e| ValidationFailure {
                path: path.to_path_buf(),
                line: e.line,
                column: e.column,
                message: e.message,
            })
            .collect();

        Ok(failures)
    }

    /// Validates multiple files.
    ///
    /// Returns all validation failures across all files.
    ///
    /// # Arguments
    ///
    /// * `files` - Iterator of (path, content) pairs to validate
    ///
    /// # Errors
    ///
    /// Returns an error if any file's parser cannot be initialised.
    pub fn validate_files<'a, I>(&self, files: I) -> Result<Vec<ValidationFailure>, SyntaxError>
    where
        I: IntoIterator<Item = (&'a Path, &'a str)>,
    {
        let mut all_failures = Vec::new();

        for (path, content) in files {
            let failures = self.validate_file(path, content)?;
            all_failures.extend(failures);
        }

        Ok(all_failures)
    }

    /// Checks if a file would be validated by this lock.
    ///
    /// Returns `true` if the file has a recognised extension that maps
    /// to a supported language.
    #[must_use]
    pub fn supports_file(path: &Path) -> bool {
        SupportedLanguage::from_path(path).is_some()
    }
}

impl Default for TreeSitterSyntacticLock {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TreeSitterSyntacticLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeSitterSyntacticLock")
            .field("languages", &SupportedLanguage::all())
            .finish_non_exhaustive()
    }
}

/// A validation failure from the syntactic lock.
///
/// This struct is designed to be compatible with `VerificationFailure` in
/// `weaverd::safety_harness` for easy integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFailure {
    /// Path to the affected file.
    pub path: std::path::PathBuf,
    /// Line number (one-based).
    pub line: u32,
    /// Column number (one-based).
    pub column: u32,
    /// Human-readable description of the problem.
    pub message: String,
}

impl std::fmt::Display for ValidationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.path.display(),
            self.line,
            self.column,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn validates_valid_rust() {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from("test.rs");
        let content = "fn main() { println!(\"hello\"); }";

        let failures = lock.validate_file(&path, content).expect("validate");
        assert!(failures.is_empty());
    }

    #[test]
    fn detects_invalid_rust() {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from("test.rs");
        let content = "fn broken() {";

        let failures = lock.validate_file(&path, content).expect("validate");
        assert!(!failures.is_empty());
    }

    #[test]
    fn skips_unknown_extensions() {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from("data.json");
        let content = "{invalid json without quotes}";

        let failures = lock.validate_file(&path, content).expect("validate");
        // Unknown extension should pass through
        assert!(failures.is_empty());
    }

    #[test]
    fn validates_valid_python() {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from("script.py");
        let content = "def hello():\n    print('hello')";

        let failures = lock.validate_file(&path, content).expect("validate");
        assert!(failures.is_empty());
    }

    #[test]
    fn detects_invalid_python() {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from("script.py");
        let content = "def broken(";

        let failures = lock.validate_file(&path, content).expect("validate");
        assert!(!failures.is_empty());
    }

    #[test]
    fn validates_valid_typescript() {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from("app.ts");
        let content = "function greet(name: string): void { console.log(name); }";

        let failures = lock.validate_file(&path, content).expect("validate");
        assert!(failures.is_empty());
    }

    #[test]
    fn failure_has_location_info() {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from("test.rs");
        let content = "fn test() {\n    let x = \n}";

        let failures = lock.validate_file(&path, content).expect("validate");
        assert!(!failures.is_empty());

        let first = failures.first().expect("failure");
        assert!(first.line >= 1);
        assert!(first.column >= 1);
        assert!(!first.message.is_empty());
    }

    #[test]
    fn validates_multiple_files() {
        let lock = TreeSitterSyntacticLock::new();

        let files: Vec<(PathBuf, &str)> = vec![
            (PathBuf::from("valid.rs"), "fn main() {}"),
            (PathBuf::from("invalid.rs"), "fn broken() {"),
            (PathBuf::from("data.json"), "{not validated}"),
        ];

        let file_refs: Vec<(&Path, &str)> = files.iter().map(|(p, c)| (p.as_path(), *c)).collect();

        let failures = lock.validate_files(file_refs).expect("validate");

        // Should have failures only from invalid.rs
        assert!(!failures.is_empty());
        assert!(
            failures
                .iter()
                .all(|f| f.path.to_string_lossy().contains("invalid"))
        );
    }

    #[test]
    fn supports_file_detects_extensions() {
        assert!(TreeSitterSyntacticLock::supports_file(Path::new("main.rs")));
        assert!(TreeSitterSyntacticLock::supports_file(Path::new(
            "script.py"
        )));
        assert!(TreeSitterSyntacticLock::supports_file(Path::new("app.ts")));
        assert!(!TreeSitterSyntacticLock::supports_file(Path::new(
            "data.json"
        )));
        assert!(!TreeSitterSyntacticLock::supports_file(Path::new(
            "README.md"
        )));
    }
}
