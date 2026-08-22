//! Behavioural tests for the process-based language server adapter.

use std::{cell::RefCell, error::Error, path::PathBuf};

use anyhow::{Context as _, Result, ensure};
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

/// Extracts the `AdapterError` wrapped by a language server error.
fn adapter_error_source(error: &LanguageServerError) -> Result<&AdapterError> {
    let source = error
        .source()
        .context("LanguageServerError is expected to wrap an AdapterError source")?;
    source
        .downcast_ref::<AdapterError>()
        .context("LanguageServerError source should be an AdapterError")
}

#[then("the error indicates binary not found")]
fn then_error_indicates_binary_not_found(world: &RefCell<AdapterTestWorld>) -> Result<()> {
    let borrow = world.borrow();
    let error = borrow
        .last_error
        .as_ref()
        .context("expected an error but got none")?;

    ensure!(
        borrow.error_is_binary_not_found,
        "expected binary not found error flag to be set, got: {error:?}"
    );

    let adapter_error = adapter_error_source(error)?;
    ensure!(
        matches!(adapter_error, AdapterError::BinaryNotFound { .. }),
        "expected AdapterError::BinaryNotFound, got: {adapter_error:?}"
    );
    Ok(())
}

#[then("the error message contains the command path")]
fn then_error_contains_command_path(world: &RefCell<AdapterTestWorld>) -> Result<()> {
    let borrow = world.borrow();
    let error = borrow.last_error.as_ref().context("expected an error")?;
    let error_string = error.to_string();
    // The error should mention the command that failed or language server.
    ensure!(
        error_string.contains("language server")
            || error_string.contains("spawn")
            || error_string.contains("/nonexistent/"),
        "error message should contain relevant context, got: {error_string}"
    );
    Ok(())
}

/// Returns the file name of a configured command, if it has one.
fn command_file_name(config: &LspServerConfig) -> Option<&str> {
    config.command.file_name().and_then(|name| name.to_str())
}

#[then("the <language> adapter command is <command>")]
fn then_language_adapter_command_is(
    _world: &RefCell<AdapterTestWorld>,
    language: Language,
    command: &str,
) -> Result<()> {
    let config = LspServerConfig::for_language(language);
    ensure!(
        command_file_name(&config) == Some(command),
        "{language:?} adapter should use {command}, got {:?}",
        config.command
    );
    Ok(())
}

#[then("the adapter command is my-rust-analyzer")]
fn then_adapter_command_is_custom(_world: &RefCell<AdapterTestWorld>) -> Result<()> {
    let config = LspServerConfig {
        command: PathBuf::from("my-rust-analyzer"),
        args: Vec::new(),
        working_dir: None,
    };
    ensure!(
        command_file_name(&config) == Some("my-rust-analyzer"),
        "rust adapter should use my-rust-analyzer, got {:?}",
        config.command
    );
    Ok(())
}

#[scenario(path = "tests/features/process_adapter.feature")]
fn process_adapter_behaviour(#[from(world)] _: RefCell<AdapterTestWorld>) {}
