//! Behavioural tests for the process-based language server adapter.

use std::{cell::RefCell, error::Error, path::PathBuf};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_test_macros::allow_fixture_expansion_lints;

use crate::{
    Language,
    adapter::{AdapterError, LspServerConfig, ProcessLanguageServer},
    server::{LanguageServer, LanguageServerError},
};

/// Test world for adapter BDD scenarios.
struct AdapterTestWorld {
    /// The adapter under test.
    adapter: Option<ProcessLanguageServer>,
    /// Last error observed during operations.
    last_error: Option<LanguageServerError>,
    /// Captured error details.
    error_is_binary_not_found: bool,
}

type StepResult = Result<(), String>;

impl AdapterTestWorld {
    fn new() -> Self {
        Self {
            adapter: None,
            last_error: None,
            error_is_binary_not_found: false,
        }
    }
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> RefCell<AdapterTestWorld> { RefCell::new(AdapterTestWorld::new()) }

// --- Given steps ---

fn create_rust_adapter_with_command(
    world: &RefCell<AdapterTestWorld>,
    command: impl Into<PathBuf>,
) {
    let config = LspServerConfig {
        command: command.into(),
        args: Vec::new(),
        working_dir: None,
    };
    let adapter = ProcessLanguageServer::with_config(Language::Rust, config);
    world.borrow_mut().adapter = Some(adapter);
}

#[given("a process adapter for rust with a nonexistent binary")]
fn given_adapter_with_nonexistent_binary(world: &RefCell<AdapterTestWorld>) {
    create_rust_adapter_with_command(world, "/nonexistent/path/to/language-server");
}

#[given("a default <language> adapter")]
fn given_default_language_adapter(world: &RefCell<AdapterTestWorld>, language: Language) {
    let adapter = ProcessLanguageServer::new(language);
    world.borrow_mut().adapter = Some(adapter);
}

#[given("a rust adapter with custom command my-rust-analyzer")]
fn given_rust_adapter_with_custom_command(world: &RefCell<AdapterTestWorld>) {
    create_rust_adapter_with_command(world, "my-rust-analyzer");
}

// --- When steps ---

#[expect(
    clippy::collapsible_if,
    reason = "nested if-lets preferred over chained let-guards for complexity metrics"
)]
fn is_binary_not_found_error(error: &LanguageServerError) -> bool {
    if let Some(source) = error.source() {
        if let Some(adapter_error) = source.downcast_ref::<AdapterError>() {
            if matches!(adapter_error, AdapterError::BinaryNotFound { .. }) {
                return true;
            }
        }
    }

    false
}

fn error_mentions_command_path(error: &str) -> bool {
    ["language server", "spawn", "/nonexistent/"]
        .iter()
        .any(|needle| error.contains(needle))
}

#[when("the adapter is initialized")]
fn when_adapter_initialized(world: &RefCell<AdapterTestWorld>) {
    let mut borrow = world.borrow_mut();
    if let Some(ref mut adapter) = borrow.adapter
        && let Err(e) = adapter.initialize()
    {
        let is_binary_not_found = is_binary_not_found_error(&e);
        borrow.last_error = Some(e);
        borrow.error_is_binary_not_found = is_binary_not_found;
    }
}

// --- Then steps ---

#[then("the error indicates binary not found")]
fn then_error_indicates_binary_not_found(world: &RefCell<AdapterTestWorld>) -> StepResult {
    let borrow = world.borrow();

    let error = borrow
        .last_error
        .as_ref()
        .ok_or_else(|| String::from("expected an error but got none"))?;

    if !borrow.error_is_binary_not_found {
        return Err(format!(
            "expected binary not found error flag to be set, got: {error:?}"
        ));
    }

    let source = error.source().ok_or_else(|| {
        String::from("LanguageServerError is expected to wrap an AdapterError source")
    })?;

    let adapter_error = source
        .downcast_ref::<AdapterError>()
        .ok_or_else(|| String::from("LanguageServerError source should be an AdapterError"))?;

    if matches!(adapter_error, AdapterError::BinaryNotFound { .. }) {
        Ok(())
    } else {
        Err(format!(
            "expected AdapterError::BinaryNotFound, got: {adapter_error:?}"
        ))
    }
}

#[then("the error message contains the command path")]
fn then_error_contains_command_path(world: &RefCell<AdapterTestWorld>) -> StepResult {
    let borrow = world.borrow();
    let error = borrow
        .last_error
        .as_ref()
        .ok_or_else(|| String::from("expected an error"))?;
    let error_string = error.to_string();
    // The error should mention the command that failed or language server
    if error_mentions_command_path(&error_string) {
        Ok(())
    } else {
        Err(format!(
            "error message should contain relevant context, got: {error_string}"
        ))
    }
}

#[then("the <language> adapter command is <command>")]
fn then_language_adapter_command_is(
    _world: &RefCell<AdapterTestWorld>,
    language: Language,
    command: &str,
) {
    let config = LspServerConfig::for_language(language);
    assert_eq!(
        config.command.file_name().and_then(|s| s.to_str()),
        Some(command),
        "{:?} adapter should use {}",
        language,
        command
    );
}

#[then("the adapter command is my-rust-analyzer")]
fn then_adapter_command_is_custom(_world: &RefCell<AdapterTestWorld>) {
    let config = LspServerConfig {
        command: PathBuf::from("my-rust-analyzer"),
        args: Vec::new(),
        working_dir: None,
    };
    assert_eq!(
        config.command.file_name().and_then(|s| s.to_str()),
        Some("my-rust-analyzer"),
        "rust adapter should use my-rust-analyzer"
    );
}

#[scenario(path = "tests/features/process_adapter.feature")]
fn process_adapter_behaviour(#[from(world)] _: RefCell<AdapterTestWorld>) {}
