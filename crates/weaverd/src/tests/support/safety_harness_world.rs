//! Test world for the Double-Lock safety harness behavioural suite.
//!
//! Extracted from the step definitions so both files stay within the project's
//! file-length limit and the world can be reused by future suites.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

use crate::{
    safety_harness::{
        ConfigurableSemanticLock,
        ConfigurableSyntacticLock,
        EditTransaction,
        FileEdit,
        Position,
        SafetyHarnessError,
        SyntacticLock,
        SyntacticLockResult,
        TextEdit,
        TransactionOutcome,
        TreeSitterSyntacticLockAdapter,
        VerificationContext,
    },
    tests::{
        safety_harness_types::{FileContent, FileName, TextPattern},
        support::fs as test_fs,
    },
};

/// Syntactic lock variant for BDD test scenarios.
///
/// Allows tests to use either a configurable lock (for controlled outcomes)
/// or the real Tree-sitter adapter (for integration testing).
pub(crate) enum SyntacticLockVariant {
    Configurable(ConfigurableSyntacticLock),
    TreeSitter(TreeSitterSyntacticLockAdapter),
}

impl SyntacticLock for SyntacticLockVariant {
    fn validate(&self, context: &VerificationContext) -> SyntacticLockResult {
        match self {
            Self::Configurable(lock) => lock.validate(context),
            Self::TreeSitter(lock) => lock.validate(context),
        }
    }
}

/// Test world for safety harness BDD scenarios.
pub(crate) struct SafetyHarnessWorld {
    temp_dir: TempDir,
    files: HashMap<String, PathBuf>,
    /// Original content of files when created, for unchanged assertions.
    original_content: HashMap<String, String>,
    /// The most recently created source file (used as default for edits).
    current_file: Option<String>,
    pub(crate) syntactic_lock: SyntacticLockVariant,
    pub(crate) semantic_lock: ConfigurableSemanticLock,
    pending_edits: Vec<FileEdit>,
    outcome: Option<Result<TransactionOutcome, SafetyHarnessError>>,
}

impl SafetyHarnessWorld {
    /// Creates a new test world.
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary workspace cannot be created.
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            temp_dir: TempDir::new()
                .map_err(|error| format!("create temporary directory: {error}"))?,
            files: HashMap::new(),
            original_content: HashMap::new(),
            current_file: None,
            syntactic_lock: SyntacticLockVariant::Configurable(ConfigurableSyntacticLock::passing()),
            semantic_lock: ConfigurableSemanticLock::passing(),
            pending_edits: Vec::new(),
            outcome: None,
        })
    }

    /// Creates a file with the given content.
    pub(crate) fn create_file(
        &mut self,
        name: &FileName,
        content: &FileContent,
    ) -> Result<(), String> {
        let path = name.to_path(self.temp_dir.path());
        test_fs::write(&path, content.as_bytes())
            .map_err(|error| format!("write content: {error}"))?;
        let name_str = name.as_str().to_owned();
        self.files.insert(name_str.clone(), path);
        self.original_content
            .insert(name_str.clone(), content.as_str().to_owned());
        self.current_file = Some(name_str);
        Ok(())
    }

    /// Returns the current (most recently created) file name for edits.
    pub(crate) fn current_file_name(&self) -> FileName {
        self.current_file.as_deref().unwrap_or("test.txt").into()
    }

    /// Returns the original content for a named file.
    pub(crate) fn original_content(&self, name: &FileName) -> Option<&str> {
        self.original_content.get(name.as_str()).map(String::as_str)
    }

    /// Returns the path for a named file.
    pub(crate) fn file_path(&self, name: &FileName) -> PathBuf {
        self.files
            .get(name.as_str())
            .cloned()
            .unwrap_or_else(|| name.to_path(self.temp_dir.path()))
    }

    /// Reads the current content of a file.
    pub(crate) fn read_file(&self, name: &FileName) -> Result<String, String> {
        let path = self.file_path(name);
        test_fs::read_to_string(&path).map_err(|error| format!("read file: {error}"))
    }

    /// Adds an edit that replaces text.
    pub(crate) fn add_replacement_edit(
        &mut self,
        name: &FileName,
        old: &TextPattern,
        new: &TextPattern,
    ) -> Result<(), String> {
        let path = self.file_path(name);
        let content = Self::read_existing_or_empty(&path)?;

        // `find` reports a byte offset at a character boundary, so the text
        // before it can always be sliced safely.
        if let Some(offset) = content.find(old.as_str()) {
            let (start, end) = Self::replacement_range(&content, offset, old.len())?;
            let edit = TextEdit::from_positions(start, end, new.as_str().to_owned());
            self.pending_edits
                .push(FileEdit::with_edits(path, vec![edit]));
        }
        Ok(())
    }

    /// Reads `path`, treating a file that does not exist as empty content.
    ///
    /// Replacement edits may target a file the scenario creates earlier in the
    /// same transaction, so a missing file is a normal state rather than an
    /// error.
    fn read_existing_or_empty(path: &Path) -> Result<String, String> {
        if test_fs::exists(path).map_err(|error| format!("check file existence: {error}"))? {
            test_fs::read_to_string(path).map_err(|error| format!("read file: {error}"))
        } else {
            Ok(String::new())
        }
    }

    /// Computes the start and end positions of the match at `offset`.
    ///
    /// Positions are one-line ranges because the harness only replaces text
    /// that stays on the line where it was found.
    fn replacement_range(
        content: &str,
        offset: usize,
        old_len: usize,
    ) -> Result<(Position, Position), String> {
        let prefix = content
            .get(..offset)
            .ok_or_else(|| String::from("matched position should be a character boundary"))?;
        let line = u32::try_from(prefix.matches('\n').count())
            .map_err(|error| format!("line index should fit in u32: {error}"))?;
        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let column = u32::try_from(offset - line_start)
            .map_err(|error| format!("column index should fit in u32: {error}"))?;
        let match_len = u32::try_from(old_len)
            .map_err(|error| format!("replacement length should fit in u32: {error}"))?;
        let end_column = column
            .checked_add(match_len)
            .ok_or_else(|| String::from("column offset should not overflow"))?;
        Ok((Position::new(line, column), Position::new(line, end_column)))
    }

    /// Adds an edit that creates a new file with content.
    pub(crate) fn add_creation_edit(&mut self, name: &FileName, content: &FileContent) {
        let path = self.file_path(name);
        let edit = TextEdit::insert_at(Position::new(0, 0), content.as_str());
        let file_edit = FileEdit::with_edits(path.clone(), vec![edit]);
        self.pending_edits.push(file_edit);
        self.files.insert(name.as_str().to_owned(), path);
    }

    /// Executes the transaction with pending edits.
    pub(crate) fn execute_transaction(&mut self) -> Result<(), String> {
        let mut transaction = EditTransaction::new(&self.syntactic_lock, &self.semantic_lock);
        for edit in self.pending_edits.drain(..) {
            transaction.add_edit(edit);
        }
        let workspace_dir =
            cap_std::fs::Dir::open_ambient_dir(self.temp_dir.path(), cap_std::ambient_authority())
                .map_err(|error| format!("open workspace dir: {error}"))?;
        self.outcome = Some(transaction.execute(&workspace_dir, self.temp_dir.path()));
        Ok(())
    }

    /// Returns the transaction outcome.
    pub(crate) fn outcome(&self) -> Option<&Result<TransactionOutcome, SafetyHarnessError>> {
        self.outcome.as_ref()
    }
}
