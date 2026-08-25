//! Behavioural tests for the Double-Lock safety harness.

use std::{cell::RefCell, path::PathBuf};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::safety_harness_types::{DiagnosticMessage, FileContent, FileName, TextPattern};
use crate::{
    safety_harness::{
        ConfigurableSemanticLock,
        ConfigurableSyntacticLock,
        SafetyHarnessError,
        TransactionOutcome,
        TreeSitterSyntacticLockAdapter,
        VerificationFailure,
    },
    tests::support::{
        fs as test_fs,
        safety_harness_world::{SafetyHarnessWorld, SyntacticLockVariant},
    },
};

/// Fixture payload: workspace creation can fail, so steps unwrap it themselves.
type SafetyHarnessWorldFixture = Result<RefCell<SafetyHarnessWorld>, String>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> SafetyHarnessWorldFixture { SafetyHarnessWorld::new().map(RefCell::new) }

/// Borrows the world, surfacing fixture construction failure as a step failure.
fn safety_world(world: &SafetyHarnessWorldFixture) -> Result<&RefCell<SafetyHarnessWorld>, String> {
    world.as_ref().map_err(Clone::clone)
}

// ---- Given steps ----

#[given("a source file {name} with content {content}")]
fn given_source_file(
    world: &SafetyHarnessWorldFixture,
    name: FileName,
    content: FileContent,
) -> Result<(), String> {
    safety_world(world)?
        .borrow_mut()
        .create_file(&name, &content)
}

#[given("no existing file {name}")]
fn given_no_file(world: &SafetyHarnessWorldFixture, name: FileName) -> Result<(), String> {
    let path = safety_world(world)?.borrow().file_path(&name);
    assert!(
        !test_fs::exists(&path).map_err(|error| format!("check file existence: {error}"))?,
        "file should not exist: {path:?}"
    );
    Ok(())
}

#[given("a syntactic lock that passes")]
fn given_syntactic_passes(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    safety_world(world)?.borrow_mut().syntactic_lock =
        SyntacticLockVariant::Configurable(ConfigurableSyntacticLock::passing());
    Ok(())
}

#[given("a syntactic lock that fails with {message}")]
fn given_syntactic_fails(
    world: &SafetyHarnessWorldFixture,
    message: DiagnosticMessage,
) -> Result<(), String> {
    let failure = VerificationFailure::new(PathBuf::from("test"), message.as_str());
    safety_world(world)?.borrow_mut().syntactic_lock =
        SyntacticLockVariant::Configurable(ConfigurableSyntacticLock::failing(vec![failure]));
    Ok(())
}

#[given("a Tree-sitter syntactic lock")]
fn given_tree_sitter_syntactic_lock(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    safety_world(world)?.borrow_mut().syntactic_lock =
        SyntacticLockVariant::TreeSitter(TreeSitterSyntacticLockAdapter::new());
    Ok(())
}

#[given("a semantic lock that passes")]
fn given_semantic_passes(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    safety_world(world)?.borrow_mut().semantic_lock = ConfigurableSemanticLock::passing();
    Ok(())
}

#[given("a semantic lock that fails with {message}")]
fn given_semantic_fails(
    world: &SafetyHarnessWorldFixture,
    message: DiagnosticMessage,
) -> Result<(), String> {
    let failure = VerificationFailure::new(PathBuf::from("test"), message.as_str());
    safety_world(world)?.borrow_mut().semantic_lock =
        ConfigurableSemanticLock::failing(vec![failure]);
    Ok(())
}

#[given("a semantic lock that is unavailable with {message}")]
fn given_semantic_unavailable(
    world: &SafetyHarnessWorldFixture,
    message: DiagnosticMessage,
) -> Result<(), String> {
    safety_world(world)?.borrow_mut().semantic_lock =
        ConfigurableSemanticLock::unavailable(message.as_str());
    Ok(())
}

// ---- When steps ----

#[when("an edit replaces {old} with {new}")]
fn when_edit_replaces(
    world: &SafetyHarnessWorldFixture,
    old: TextPattern,
    new: TextPattern,
) -> Result<(), String> {
    // Use current file from scenario, falling back to "test.txt"
    let state = safety_world(world)?;
    let file_name = state.borrow().current_file_name();
    state
        .borrow_mut()
        .add_replacement_edit(&file_name, &old, &new)?;
    state.borrow_mut().execute_transaction()
}

#[when("an edit replaces {old} with {new} in {name}")]
fn when_edit_replaces_in_file(
    world: &SafetyHarnessWorldFixture,
    old: TextPattern,
    new: TextPattern,
    name: FileName,
) -> Result<(), String> {
    safety_world(world)?
        .borrow_mut()
        .add_replacement_edit(&name, &old, &new)
}

#[when("no edits are submitted")]
#[expect(unused_variables, reason = "No edits to add for this step.")]
fn when_no_edits(world: &SafetyHarnessWorldFixture) {}

#[when("an edit creates {name} with content {content}")]
fn when_edit_creates(
    world: &SafetyHarnessWorldFixture,
    name: FileName,
    content: FileContent,
) -> Result<(), String> {
    let state = safety_world(world)?;
    state.borrow_mut().add_creation_edit(&name, &content);
    state.borrow_mut().execute_transaction()
}

/// Executes the transaction, if needed, before asserting its outcome.
fn assert_outcome<F>(world: &SafetyHarnessWorldFixture, assertion: F) -> Result<(), String>
where
    F: FnOnce(&Result<TransactionOutcome, SafetyHarnessError>),
{
    let cell = safety_world(world)?;
    if cell.borrow().outcome().is_none() {
        cell.borrow_mut().execute_transaction()?;
    }
    let state = cell.borrow();
    let Some(outcome) = state.outcome() else {
        return Err("outcome should exist".to_string());
    };
    assertion(outcome);
    Ok(())
}

#[then("the transaction commits successfully")]
fn then_commits(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    assert_outcome(world, |outcome| {
        assert!(
            outcome.as_ref().is_ok_and(|o| o.committed()),
            "transaction should commit: {outcome:?}"
        );
    })?;
    Ok(())
}

#[then("the transaction fails with a syntactic lock error")]
fn then_syntactic_fails(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    assert_outcome(world, |outcome| match outcome {
        Ok(TransactionOutcome::SyntacticLockFailed { .. }) => {}
        other => panic!("expected syntactic lock failure, got {other:?}"),
    })?;
    Ok(())
}

#[then("the transaction fails with a semantic lock error")]
fn then_semantic_fails(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    assert_outcome(world, |outcome| match outcome {
        Ok(TransactionOutcome::SemanticLockFailed { .. }) => {}
        other => panic!("expected semantic lock failure, got {other:?}"),
    })?;
    Ok(())
}

#[then("the transaction fails with a backend error")]
fn then_backend_error(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    assert_outcome(world, |outcome| match outcome {
        Err(SafetyHarnessError::SemanticBackendUnavailable { .. }) => {}
        other => panic!("expected backend error, got {other:?}"),
    })?;
    Ok(())
}

#[then("the transaction reports no changes")]
fn then_no_changes(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    assert_outcome(world, |outcome| match outcome {
        Ok(TransactionOutcome::NoChanges) => {}
        other => panic!("expected no changes, got {other:?}"),
    })?;
    Ok(())
}

#[then("the file contains {expected}")]
fn then_file_contains(
    world: &SafetyHarnessWorldFixture,
    expected: TextPattern,
) -> Result<(), String> {
    let state = safety_world(world)?;
    let file_name = state.borrow().current_file_name();
    let content = state.borrow().read_file(&file_name)?;
    assert!(
        content.contains(expected.as_str()),
        "expected file to contain '{}', got '{content}'",
        expected.as_str()
    );
    Ok(())
}

#[then("the file {name} contains {expected}")]
fn then_named_file_contains(
    world: &SafetyHarnessWorldFixture,
    name: FileName,
    expected: TextPattern,
) -> Result<(), String> {
    let content = safety_world(world)?.borrow().read_file(&name)?;
    assert!(
        content.contains(expected.as_str()),
        "expected {} to contain '{}', got '{content}'",
        name.as_str(),
        expected.as_str()
    );
    Ok(())
}

#[then("the file is unchanged")]
fn then_file_unchanged(world: &SafetyHarnessWorldFixture) -> Result<(), String> {
    let state = safety_world(world)?.borrow();
    let file_name = state.current_file_name();
    let content = state.read_file(&file_name)?;
    let expected = state
        .original_content(&file_name)
        .ok_or_else(|| format!("no original content recorded for {}", file_name.as_str()))?;
    assert_eq!(content, expected, "file should be unchanged");
    Ok(())
}

#[then("the file {name} is unchanged")]
fn then_named_file_unchanged(
    world: &SafetyHarnessWorldFixture,
    name: FileName,
) -> Result<(), String> {
    let state = safety_world(world)?.borrow();
    let content = state.read_file(&name)?;
    let expected = state
        .original_content(&name)
        .ok_or_else(|| format!("no original content recorded for {}", name.as_str()))?;
    assert_eq!(content, expected, "{} should be unchanged", name.as_str());
    Ok(())
}

#[scenario(path = "tests/features/safety_harness.feature")]
fn safety_harness(#[from(world)] _: SafetyHarnessWorldFixture) {}
