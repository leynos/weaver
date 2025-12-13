//! Behavioural tests for the Double-Lock safety harness.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;

use super::safety_harness_types::{DiagnosticMessage, FileContent, FileName, TextPattern};
use crate::safety_harness::{
    ConfigurableSemanticLock, ConfigurableSyntacticLock, EditTransaction, FileEdit, Position,
    SafetyHarnessError, TextEdit, TransactionOutcome, VerificationFailure,
};

/// Test world for safety harness BDD scenarios.
pub struct SafetyHarnessWorld {
    temp_dir: TempDir,
    files: HashMap<String, PathBuf>,
    /// Original content of files when created, for unchanged assertions.
    original_content: HashMap<String, String>,
    syntactic_lock: ConfigurableSyntacticLock,
    semantic_lock: ConfigurableSemanticLock,
    pending_edits: Vec<FileEdit>,
    outcome: Option<Result<TransactionOutcome, SafetyHarnessError>>,
}

impl SafetyHarnessWorld {
    /// Creates a new test world.
    fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("create temp dir"),
            files: HashMap::new(),
            original_content: HashMap::new(),
            syntactic_lock: ConfigurableSyntacticLock::passing(),
            semantic_lock: ConfigurableSemanticLock::passing(),
            pending_edits: Vec::new(),
            outcome: None,
        }
    }

    /// Creates a file with the given content.
    fn create_file(&mut self, name: &FileName, content: &FileContent) {
        let path = name.to_path(self.temp_dir.path());
        let mut file = fs::File::create(&path).expect("create file");
        file.write_all(content.as_bytes()).expect("write content");
        let name_str = name.as_str().to_string();
        self.files.insert(name_str.clone(), path);
        self.original_content
            .insert(name_str, content.as_str().to_string());
    }

    /// Returns the original content for a named file.
    fn original_content(&self, name: &FileName) -> Option<&str> {
        self.original_content.get(name.as_str()).map(String::as_str)
    }

    /// Returns the path for a named file.
    fn file_path(&self, name: &FileName) -> PathBuf {
        self.files
            .get(name.as_str())
            .cloned()
            .unwrap_or_else(|| name.to_path(self.temp_dir.path()))
    }

    /// Reads the current content of a file.
    fn read_file(&self, name: &FileName) -> String {
        let path = self.file_path(name);
        fs::read_to_string(&path).expect("read file")
    }

    /// Adds an edit that replaces text.
    fn add_replacement_edit(&mut self, name: &FileName, old: &TextPattern, new: &TextPattern) {
        let path = self.file_path(name);
        let content = if path.exists() {
            fs::read_to_string(&path).expect("read file")
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
    }

    /// Adds an edit that creates a new file with content.
    fn add_creation_edit(&mut self, name: &FileName, content: &FileContent) {
        let path = self.file_path(name);
        let edit = TextEdit::insert_at(Position::new(0, 0), content.as_str());
        let file_edit = FileEdit::with_edits(path.clone(), vec![edit]);
        self.pending_edits.push(file_edit);
        self.files.insert(name.as_str().to_string(), path);
    }

    /// Executes the transaction with pending edits.
    fn execute_transaction(&mut self) {
        let mut transaction = EditTransaction::new(&self.syntactic_lock, &self.semantic_lock);
        for edit in self.pending_edits.drain(..) {
            transaction.add_edit(edit);
        }
        self.outcome = Some(transaction.execute());
    }

    /// Returns the transaction outcome.
    fn outcome(&self) -> Option<&Result<TransactionOutcome, SafetyHarnessError>> {
        self.outcome.as_ref()
    }
}

#[fixture]
fn world() -> RefCell<SafetyHarnessWorld> {
    RefCell::new(SafetyHarnessWorld::new())
}

// ---- Given steps ----

#[given("a source file {name} with content {content}")]
fn given_source_file(world: &RefCell<SafetyHarnessWorld>, name: FileName, content: FileContent) {
    world.borrow_mut().create_file(&name, &content);
}

#[given("no existing file {name}")]
fn given_no_file(world: &RefCell<SafetyHarnessWorld>, name: FileName) {
    let path = world.borrow().file_path(&name);
    assert!(!path.exists(), "file should not exist: {path:?}");
}

#[given("a syntactic lock that passes")]
fn given_syntactic_passes(world: &RefCell<SafetyHarnessWorld>) {
    world.borrow_mut().syntactic_lock = ConfigurableSyntacticLock::passing();
}

#[given("a syntactic lock that fails with {message}")]
fn given_syntactic_fails(world: &RefCell<SafetyHarnessWorld>, message: DiagnosticMessage) {
    let failure = VerificationFailure::new(PathBuf::from("test"), message.as_str());
    world.borrow_mut().syntactic_lock = ConfigurableSyntacticLock::failing(vec![failure]);
}

#[given("a semantic lock that passes")]
fn given_semantic_passes(world: &RefCell<SafetyHarnessWorld>) {
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::passing();
}

#[given("a semantic lock that fails with {message}")]
fn given_semantic_fails(world: &RefCell<SafetyHarnessWorld>, message: DiagnosticMessage) {
    let failure = VerificationFailure::new(PathBuf::from("test"), message.as_str());
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::failing(vec![failure]);
}

#[given("a semantic lock that is unavailable with {message}")]
fn given_semantic_unavailable(world: &RefCell<SafetyHarnessWorld>, message: DiagnosticMessage) {
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::unavailable(message.as_str());
}

// ---- When steps ----

#[when("an edit replaces {old} with {new}")]
fn when_edit_replaces(world: &RefCell<SafetyHarnessWorld>, old: TextPattern, new: TextPattern) {
    // Use default file name "test.txt"
    let default_name: FileName = "test.txt".into();
    world
        .borrow_mut()
        .add_replacement_edit(&default_name, &old, &new);
    world.borrow_mut().execute_transaction();
}

#[when("an edit replaces {old} with {new} in {name}")]
fn when_edit_replaces_in_file(
    world: &RefCell<SafetyHarnessWorld>,
    old: TextPattern,
    new: TextPattern,
    name: FileName,
) {
    world.borrow_mut().add_replacement_edit(&name, &old, &new);
}

#[when("no edits are submitted")]
fn when_no_edits(_: &RefCell<SafetyHarnessWorld>) {}

#[when("an edit creates {name} with content {content}")]
fn when_edit_creates(world: &RefCell<SafetyHarnessWorld>, name: FileName, content: FileContent) {
    world.borrow_mut().add_creation_edit(&name, &content);
    world.borrow_mut().execute_transaction();
}

// ---- Then steps ----

/// Helper for outcome assertion steps that execute the transaction if needed.
fn assert_outcome<F>(world: &RefCell<SafetyHarnessWorld>, assertion: F)
where
    F: FnOnce(&Result<TransactionOutcome, SafetyHarnessError>),
{
    if world.borrow().outcome().is_none() {
        world.borrow_mut().execute_transaction();
    }
    let world = world.borrow();
    let outcome = world.outcome().expect("outcome should exist");
    assertion(outcome);
}

#[then("the transaction commits successfully")]
fn then_commits(world: &RefCell<SafetyHarnessWorld>) {
    assert_outcome(world, |outcome| {
        assert!(
            outcome.as_ref().is_ok_and(|o| o.committed()),
            "transaction should commit: {outcome:?}"
        );
    });
}

#[then("the transaction fails with a syntactic lock error")]
fn then_syntactic_fails(world: &RefCell<SafetyHarnessWorld>) {
    assert_outcome(world, |outcome| match outcome {
        Ok(TransactionOutcome::SyntacticLockFailed { .. }) => {}
        other => panic!("expected syntactic lock failure, got {other:?}"),
    });
}

#[then("the transaction fails with a semantic lock error")]
fn then_semantic_fails(world: &RefCell<SafetyHarnessWorld>) {
    assert_outcome(world, |outcome| match outcome {
        Ok(TransactionOutcome::SemanticLockFailed { .. }) => {}
        other => panic!("expected semantic lock failure, got {other:?}"),
    });
}

#[then("the transaction fails with a backend error")]
fn then_backend_error(world: &RefCell<SafetyHarnessWorld>) {
    assert_outcome(world, |outcome| match outcome {
        Err(SafetyHarnessError::SemanticBackendUnavailable { .. }) => {}
        other => panic!("expected backend error, got {other:?}"),
    });
}

#[then("the transaction reports no changes")]
fn then_no_changes(world: &RefCell<SafetyHarnessWorld>) {
    assert_outcome(world, |outcome| match outcome {
        Ok(TransactionOutcome::NoChanges) => {}
        other => panic!("expected no changes, got {other:?}"),
    });
}

#[then("the file contains {expected}")]
fn then_file_contains(world: &RefCell<SafetyHarnessWorld>, expected: TextPattern) {
    let default_name: FileName = "test.txt".into();
    let content = world.borrow().read_file(&default_name);
    assert!(
        content.contains(expected.as_str()),
        "expected file to contain '{}', got '{content}'",
        expected.as_str()
    );
}

#[then("the file {name} contains {expected}")]
fn then_named_file_contains(
    world: &RefCell<SafetyHarnessWorld>,
    name: FileName,
    expected: TextPattern,
) {
    let content = world.borrow().read_file(&name);
    assert!(
        content.contains(expected.as_str()),
        "expected {} to contain '{}', got '{content}'",
        name.as_str(),
        expected.as_str()
    );
}

#[then("the file is unchanged")]
fn then_file_unchanged(world: &RefCell<SafetyHarnessWorld>) {
    let default_name: FileName = "test.txt".into();
    let content = world.borrow().read_file(&default_name);
    assert_eq!(content, "hello world", "file should be unchanged");
}

#[then("the file {name} is unchanged")]
fn then_named_file_unchanged(world: &RefCell<SafetyHarnessWorld>, name: FileName) {
    let world = world.borrow();
    let content = world.read_file(&name);
    let expected = world
        .original_content(&name)
        .unwrap_or_else(|| panic!("no original content recorded for {}", name.as_str()));
    assert_eq!(content, expected, "{} should be unchanged", name.as_str());
}

#[scenario(path = "tests/features/safety_harness.feature")]
fn safety_harness(#[from(world)] _: RefCell<SafetyHarnessWorld>) {}
