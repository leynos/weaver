//! Test world for the Double-Lock safety harness behavioural suite.
//!
//! Extracted from the step definitions so both files stay within the project's
//! file-length limit and the world can be reused by future suites.

use std::{collections::HashMap, path::PathBuf};

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
        let name_str = name.as_str().to_string();
        self.files.insert(name_str.clone(), path);
        self.original_content
            .insert(name_str.clone(), content.as_str().to_string());
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
        let content =
            if test_fs::exists(&path).map_err(|error| format!("check file existence: {error}"))? {
                test_fs::read_to_string(&path).map_err(|error| format!("read file: {error}"))?
            } else {
                String::new()
            };

        // Find the position of the old text
        if let Some(pos) = content.find(old.as_str()) {
            let line = content[..pos].matches('\n').count() as u32;
            let line_start = content[..pos].rfind('\n').map_or(0, |i| i + 1);
            let column = (pos - line_start) as u32;
            let old_end_col = column + old.len() as u32;

            let edit = TextEdit::from_positions(
                Position::new(line, column),
                Position::new(line, old_end_col),
                new.as_str().to_string(),
            );
            let file_edit = FileEdit::with_edits(path, vec![edit]);
            self.pending_edits.push(file_edit);
        }
        Ok(())
    }

    /// Adds an edit that creates a new file with content.
    pub(crate) fn add_creation_edit(&mut self, name: &FileName, content: &FileContent) {
        let path = self.file_path(name);
        let edit = TextEdit::insert_at(Position::new(0, 0), content.as_str());
        let file_edit = FileEdit::with_edits(path.clone(), vec![edit]);
        self.pending_edits.push(file_edit);
        self.files.insert(name.as_str().to_string(), path);
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
