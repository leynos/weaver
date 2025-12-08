//! Behavioural tests for the Double-Lock safety harness.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;

use crate::safety_harness::{
    ConfigurableSemanticLock, ConfigurableSyntacticLock, EditTransaction, FileEdit,
    SafetyHarnessError, TextEdit, TransactionOutcome, VerificationFailure,
};

/// Test world for safety harness BDD scenarios.
pub struct SafetyHarnessWorld {
    temp_dir: TempDir,
    files: HashMap<String, PathBuf>,
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
            syntactic_lock: ConfigurableSyntacticLock::passing(),
            semantic_lock: ConfigurableSemanticLock::passing(),
            pending_edits: Vec::new(),
            outcome: None,
        }
    }

    /// Creates a file with the given content.
    fn create_file(&mut self, name: &str, content: &str) {
        let path = self.temp_dir.path().join(name);
        let mut file = fs::File::create(&path).expect("create file");
        file.write_all(content.as_bytes()).expect("write content");
        self.files.insert(name.to_string(), path);
    }

    /// Returns the path for a named file.
    fn file_path(&self, name: &str) -> PathBuf {
        self.files
            .get(name)
            .cloned()
            .unwrap_or_else(|| self.temp_dir.path().join(name))
    }

    /// Reads the current content of a file.
    fn read_file(&self, name: &str) -> String {
        let path = self.file_path(name);
        fs::read_to_string(&path).expect("read file")
    }

    /// Adds an edit that replaces text.
    fn add_replacement_edit(&mut self, name: &str, old: &str, new: &str) {
        let path = self.file_path(name);
        let content = if path.exists() {
            fs::read_to_string(&path).expect("read file")
        } else {
            String::new()
        };

        // Find the position of the old text
        if let Some(pos) = content.find(old) {
            let line = content[..pos].matches('\n').count() as u32;
            let line_start = content[..pos].rfind('\n').map_or(0, |i| i + 1);
            let column = (pos - line_start) as u32;
            let old_end_col = column + old.len() as u32;

            let edit = TextEdit::from_coords(line, column, line, old_end_col, new.to_string());
            let file_edit = FileEdit::with_edits(path, vec![edit]);
            self.pending_edits.push(file_edit);
        }
    }

    /// Adds an edit that creates a new file with content.
    fn add_creation_edit(&mut self, name: &str, content: &str) {
        let path = self.file_path(name);
        let edit = TextEdit::insert(0, 0, content.to_string());
        let file_edit = FileEdit::with_edits(path.clone(), vec![edit]);
        self.pending_edits.push(file_edit);
        self.files.insert(name.to_string(), path);
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
fn given_source_file(world: &RefCell<SafetyHarnessWorld>, name: String, content: String) {
    let name = name.trim_matches('"');
    let content = content.trim_matches('"');
    world.borrow_mut().create_file(name, content);
}

#[given("no existing file {name}")]
fn given_no_file(world: &RefCell<SafetyHarnessWorld>, name: String) {
    let name = name.trim_matches('"');
    let path = world.borrow().file_path(name);
    assert!(!path.exists(), "file should not exist: {path:?}");
}

#[given("a syntactic lock that passes")]
fn given_syntactic_passes(world: &RefCell<SafetyHarnessWorld>) {
    world.borrow_mut().syntactic_lock = ConfigurableSyntacticLock::passing();
}

#[given("a syntactic lock that fails with {message}")]
fn given_syntactic_fails(world: &RefCell<SafetyHarnessWorld>, message: String) {
    let message = message.trim_matches('"');
    let failure = VerificationFailure::new(PathBuf::from("test"), message);
    world.borrow_mut().syntactic_lock = ConfigurableSyntacticLock::failing(vec![failure]);
}

#[given("a semantic lock that passes")]
fn given_semantic_passes(world: &RefCell<SafetyHarnessWorld>) {
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::passing();
}

#[given("a semantic lock that fails with {message}")]
fn given_semantic_fails(world: &RefCell<SafetyHarnessWorld>, message: String) {
    let message = message.trim_matches('"');
    let failure = VerificationFailure::new(PathBuf::from("test"), message);
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::failing(vec![failure]);
}

#[given("a semantic lock that is unavailable with {message}")]
fn given_semantic_unavailable(world: &RefCell<SafetyHarnessWorld>, message: String) {
    let message = message.trim_matches('"');
    world.borrow_mut().semantic_lock = ConfigurableSemanticLock::unavailable(message);
}

// ---- When steps ----

#[when("an edit replaces {old} with {new}")]
fn when_edit_replaces(world: &RefCell<SafetyHarnessWorld>, old: String, new: String) {
    let old = old.trim_matches('"');
    let new = new.trim_matches('"');
    // Use default file name "test.txt"
    world
        .borrow_mut()
        .add_replacement_edit("test.txt", old, new);
    world.borrow_mut().execute_transaction();
}

#[when("an edit replaces {old} with {new} in {name}")]
fn when_edit_replaces_in_file(
    world: &RefCell<SafetyHarnessWorld>,
    old: String,
    new: String,
    name: String,
) {
    let old = old.trim_matches('"');
    let new = new.trim_matches('"');
    let name = name.trim_matches('"');
    world.borrow_mut().add_replacement_edit(name, old, new);
}

#[when("no edits are submitted")]
fn when_no_edits(_world: &RefCell<SafetyHarnessWorld>) {
    // No edits to add
}

#[when("an edit creates {name} with content {content}")]
fn when_edit_creates(world: &RefCell<SafetyHarnessWorld>, name: String, content: String) {
    let name = name.trim_matches('"');
    let content = content.trim_matches('"');
    world.borrow_mut().add_creation_edit(name, content);
    world.borrow_mut().execute_transaction();
}

// ---- Then steps ----

#[then("the transaction commits successfully")]
fn then_commits(world: &RefCell<SafetyHarnessWorld>) {
    // Execute if not already done
    if world.borrow().outcome().is_none() {
        world.borrow_mut().execute_transaction();
    }
    let world = world.borrow();
    let outcome = world.outcome().expect("outcome should exist");
    assert!(
        outcome.as_ref().is_ok_and(|o| o.committed()),
        "transaction should commit: {outcome:?}"
    );
}

#[then("the transaction fails with a syntactic lock error")]
fn then_syntactic_fails(world: &RefCell<SafetyHarnessWorld>) {
    let world = world.borrow();
    let outcome = world.outcome().expect("outcome should exist");
    match outcome {
        Ok(TransactionOutcome::SyntacticLockFailed { .. }) => {}
        other => panic!("expected syntactic lock failure, got {other:?}"),
    }
}

#[then("the transaction fails with a semantic lock error")]
fn then_semantic_fails(world: &RefCell<SafetyHarnessWorld>) {
    let world = world.borrow();
    let outcome = world.outcome().expect("outcome should exist");
    match outcome {
        Ok(TransactionOutcome::SemanticLockFailed { .. }) => {}
        other => panic!("expected semantic lock failure, got {other:?}"),
    }
}

#[then("the transaction fails with a backend error")]
fn then_backend_error(world: &RefCell<SafetyHarnessWorld>) {
    let world = world.borrow();
    let outcome = world.outcome().expect("outcome should exist");
    match outcome {
        Err(SafetyHarnessError::SemanticBackendUnavailable { .. }) => {}
        other => panic!("expected backend error, got {other:?}"),
    }
}

#[then("the transaction reports no changes")]
fn then_no_changes(world: &RefCell<SafetyHarnessWorld>) {
    // Execute if not already done
    if world.borrow().outcome().is_none() {
        world.borrow_mut().execute_transaction();
    }
    let world = world.borrow();
    let outcome = world.outcome().expect("outcome should exist");
    match outcome {
        Ok(TransactionOutcome::NoChanges) => {}
        other => panic!("expected no changes, got {other:?}"),
    }
}

#[then("the file contains {expected}")]
fn then_file_contains(world: &RefCell<SafetyHarnessWorld>, expected: String) {
    let expected = expected.trim_matches('"');
    let content = world.borrow().read_file("test.txt");
    assert!(
        content.contains(expected),
        "expected file to contain '{expected}', got '{content}'"
    );
}

#[then("the file {name} contains {expected}")]
fn then_named_file_contains(world: &RefCell<SafetyHarnessWorld>, name: String, expected: String) {
    let name = name.trim_matches('"');
    let expected = expected.trim_matches('"');
    let content = world.borrow().read_file(name);
    assert!(
        content.contains(expected),
        "expected {name} to contain '{expected}', got '{content}'"
    );
}

#[then("the file is unchanged")]
fn then_file_unchanged(world: &RefCell<SafetyHarnessWorld>) {
    let content = world.borrow().read_file("test.txt");
    assert_eq!(content, "hello world", "file should be unchanged");
}

#[then("the file {name} is unchanged")]
fn then_named_file_unchanged(world: &RefCell<SafetyHarnessWorld>, name: String) {
    let name = name.trim_matches('"');
    let content = world.borrow().read_file(name);
    let expected = match name {
        "file1.txt" => "aaa",
        "file2.txt" => "bbb",
        _ => panic!("unknown file: {name}"),
    };
    assert_eq!(content, expected, "{name} should be unchanged");
}

#[scenario(path = "tests/features/safety_harness.feature")]
fn safety_harness(#[from(world)] _: RefCell<SafetyHarnessWorld>) {}
