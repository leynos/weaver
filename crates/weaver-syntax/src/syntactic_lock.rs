//! Tree-sitter based syntactic validation for the Double-Lock harness.
//!
//! This module provides [`TreeSitterSyntacticLock`], which validates that
//! modified files produce valid syntax trees. It integrates with the
//! safety harness in `weaverd` to prevent syntactically invalid code from
//! being committed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    parsers: Mutex<HashMap<SupportedLanguage, Arc<Mutex<Parser>>>>,
}

/// Owned file content for syntactic lock validation.
///
/// This is a small convenience wrapper for callers that naturally work with
/// `PathBuf` and `String` values (e.g. staging edits before validation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedFile {
    /// Path to the file (used for language detection).
    pub path: PathBuf,
    /// File content to validate.
    pub content: String,
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
        let parser = {
            let mut parsers = self
                .parsers
                .lock()
                .map_err(|_| SyntaxError::internal_error("parser map lock poisoned"))?;

            if let Some(parser) = parsers.get(&language) {
                parser.clone()
            } else {
                let parser = Arc::new(Mutex::new(Parser::new(language)?));
                parsers.insert(language, parser.clone());
                parser
            }
        };

        let mut parser_guard = parser
            .lock()
            .map_err(|_| SyntaxError::internal_error("parser lock poisoned"))?;

        // Parse the content
        let result = parser_guard.parse(content)?;

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

    /// Validates a single file using owned inputs.
    ///
    /// This is a convenience wrapper around [`Self::validate_file`] for call
    /// sites that already have owned `PathBuf`/`String` values.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::validate_file`], such as parser
    /// initialisation failures or internal lock poisoning.
    pub fn validate_owned_file(
        &self,
        file: OwnedFile,
    ) -> Result<Vec<ValidationFailure>, SyntaxError> {
        let OwnedFile { path, content } = file;
        self.validate_file(&path, &content)
    }

    fn validate_pairs<I, P, C>(&self, files: I) -> Result<Vec<ValidationFailure>, SyntaxError>
    where
        I: IntoIterator<Item = (P, C)>,
        P: AsRef<Path>,
        C: AsRef<str>,
    {
        let mut all_failures = Vec::new();

        for (path, content) in files {
            let failures = self.validate_file(path.as_ref(), content.as_ref())?;
            all_failures.extend(failures);
        }

        Ok(all_failures)
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
        self.validate_pairs(files)
    }

    /// Validates multiple files using owned inputs.
    ///
    /// This is a convenience wrapper around [`Self::validate_files`] for call
    /// sites that store file data as owned values.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::validate_files`], such as parser
    /// initialisation failures.
    pub fn validate_owned_files<I>(&self, files: I) -> Result<Vec<ValidationFailure>, SyntaxError>
    where
        I: IntoIterator<Item = OwnedFile>,
    {
        self.validate_pairs(
            files
                .into_iter()
                .map(|OwnedFile { path, content }| (path, content)),
        )
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
    use rstest::rstest;
    use std::path::PathBuf;

    #[rstest]
    #[case("test.rs", "fn main() { println!(\"hello\"); }", true)]
    #[case("test.rs", "fn broken() {", false)]
    #[case("script.py", "def hello():\n    print('hello')", true)]
    #[case("script.py", "def broken(", false)]
    #[case(
        "app.ts",
        "function greet(name: string): void { console.log(name); }",
        true
    )]
    #[case("broken.tsx", "function broken( {", false)]
    #[case("data.json", "{invalid json without quotes}", true)]
    fn validate_file_cases(
        #[case] filename: &str,
        #[case] content: &str,
        #[case] should_pass: bool,
    ) {
        let lock = TreeSitterSyntacticLock::new();
        let path = PathBuf::from(filename);

        let failures = lock.validate_file(&path, content).expect("validate");
        if should_pass {
            assert!(failures.is_empty(), "Expected no failures for {filename}");
        } else {
            assert!(!failures.is_empty(), "Expected failures for {filename}");
        }
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

    #[rstest]
    #[case("main.rs", true)]
    #[case("script.py", true)]
    #[case("app.ts", true)]
    #[case("view.tsx", true)]
    #[case("data.json", false)]
    #[case("README.md", false)]
    fn supports_file_detects_extensions(#[case] path: &str, #[case] expected: bool) {
        assert_eq!(
            TreeSitterSyntacticLock::supports_file(Path::new(path)),
            expected
        );
    }

    #[test]
    fn validate_owned_file_accepts_pathbuf_and_string() {
        let lock = TreeSitterSyntacticLock::new();
        let file = OwnedFile {
            path: PathBuf::from("test.rs"),
            content: "fn main() {}".to_owned(),
        };

        let failures = lock.validate_owned_file(file).expect("validate");
        assert!(failures.is_empty());
    }

    #[test]
    fn validate_owned_files_collects_failures() {
        let lock = TreeSitterSyntacticLock::new();
        let files = vec![
            OwnedFile {
                path: PathBuf::from("ok.rs"),
                content: "fn main() {}".to_owned(),
            },
            OwnedFile {
                path: PathBuf::from("broken.rs"),
                content: "fn broken() {".to_owned(),
            },
        ];

        let failures = lock.validate_owned_files(files).expect("validate");
        assert!(!failures.is_empty());
        assert!(failures.iter().any(|f| f.path.ends_with("broken.rs")));
    }
}
