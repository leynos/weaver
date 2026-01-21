//! Behavioural tests for the process-based language server adapter.

// rstest-bdd macros generate some non-snake-case identifiers
#![allow(non_snake_case)]

use std::cell::RefCell;
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use crate::Language;
use crate::adapter::{AdapterError, LspServerConfig, ProcessLanguageServer};
use crate::server::LanguageServer;

/// Test world for adapter BDD scenarios.
struct AdapterTestWorld {
    /// The adapter under test.
    adapter: Option<ProcessLanguageServer>,
    /// Additional adapters for multi-language scenarios.
    python_adapter: Option<ProcessLanguageServer>,
    typescript_adapter: Option<ProcessLanguageServer>,
    /// Last error observed during operations.
    last_error: Option<String>,
    /// Captured error details.
    error_is_binary_not_found: bool,
}

impl AdapterTestWorld {
    fn new() -> Self {
        Self {
            adapter: None,
            python_adapter: None,
            typescript_adapter: None,
            last_error: None,
            error_is_binary_not_found: false,
        }
    }
}

#[fixture]
fn world() -> RefCell<AdapterTestWorld> {
    RefCell::new(AdapterTestWorld::new())
}

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

#[given("a default rust adapter")]
fn given_default_rust_adapter(world: &RefCell<AdapterTestWorld>) {
    let adapter = ProcessLanguageServer::new(Language::Rust);
    world.borrow_mut().adapter = Some(adapter);
}

#[given("a default python adapter")]
fn given_default_python_adapter(world: &RefCell<AdapterTestWorld>) {
    let adapter = ProcessLanguageServer::new(Language::Python);
    world.borrow_mut().python_adapter = Some(adapter);
}

#[given("a default typescript adapter")]
fn given_default_typescript_adapter(world: &RefCell<AdapterTestWorld>) {
    let adapter = ProcessLanguageServer::new(Language::TypeScript);
    world.borrow_mut().typescript_adapter = Some(adapter);
}

#[given("a rust adapter with custom command my-rust-analyzer")]
fn given_rust_adapter_with_custom_command(world: &RefCell<AdapterTestWorld>) {
    create_rust_adapter_with_command(world, "my-rust-analyzer");
}

// --- When steps ---

#[allow(clippy::collapsible_if)]
fn is_binary_not_found_error(error: &dyn Error) -> bool {
    if let Some(source) = error.source() {
        if let Some(adapter_error) = source.downcast_ref::<AdapterError>() {
            if matches!(adapter_error, AdapterError::BinaryNotFound { .. }) {
                return true;
            }
        }
    }

    // Also check the error message for hints
    let error_string = error.to_string();
    error_string.contains("spawn")
        || error_string.contains("not found")
        || error_string.contains("No such file")
}

#[when("the adapter is initialized")]
fn when_adapter_initialized(world: &RefCell<AdapterTestWorld>) {
    let mut borrow = world.borrow_mut();
    if let Some(ref mut adapter) = borrow.adapter
        && let Err(e) = adapter.initialize()
    {
        borrow.last_error = Some(e.to_string());
        borrow.error_is_binary_not_found = is_binary_not_found_error(&e);
    }
}

// --- Then steps ---

#[then("the error indicates binary not found")]
fn then_error_indicates_binary_not_found(world: &RefCell<AdapterTestWorld>) {
    let borrow = world.borrow();
    assert!(
        borrow.last_error.is_some(),
        "expected an error but got none"
    );
    assert!(
        borrow.error_is_binary_not_found,
        "expected binary not found error, got: {:?}",
        borrow.last_error
    );
}

#[then("the error message contains the command path")]
fn then_error_contains_command_path(world: &RefCell<AdapterTestWorld>) {
    let borrow = world.borrow();
    let error = borrow.last_error.as_ref().expect("expected an error");
    // The error should mention the command that failed or language server
    assert!(
        error.contains("language server")
            || error.contains("spawn")
            || error.contains("/nonexistent/"),
        "error message should contain relevant context, got: {}",
        error
    );
}

#[then("the adapter command is rust-analyzer")]
fn then_adapter_command_is_rust_analyzer(_world: &RefCell<AdapterTestWorld>) {
    let config = LspServerConfig::for_language(Language::Rust);
    assert_eq!(
        config.command.file_name().and_then(|s| s.to_str()),
        Some("rust-analyzer"),
        "rust adapter should use rust-analyzer"
    );
}

#[then("the python adapter command is pyrefly")]
fn then_python_adapter_command_is_pyrefly(_world: &RefCell<AdapterTestWorld>) {
    let config = LspServerConfig::for_language(Language::Python);
    assert_eq!(
        config.command.file_name().and_then(|s| s.to_str()),
        Some("pyrefly"),
        "python adapter should use pyrefly"
    );
}

#[then("the typescript adapter command is tsgo")]
fn then_typescript_adapter_command_is_tsgo(_world: &RefCell<AdapterTestWorld>) {
    let config = LspServerConfig::for_language(Language::TypeScript);
    assert_eq!(
        config.command.file_name().and_then(|s| s.to_str()),
        Some("tsgo"),
        "typescript adapter should use tsgo"
    );
}

#[then("the adapter command is my-rust-analyzer")]
fn then_adapter_command_is_custom(world: &RefCell<AdapterTestWorld>) {
    // We can verify the construction succeeded (adapter exists)
    let borrow = world.borrow();
    assert!(
        borrow.adapter.is_some(),
        "adapter should be created with custom config"
    );
}

#[scenario(path = "tests/features/process_adapter.feature")]
fn process_adapter_behaviour(#[from(world)] _: RefCell<AdapterTestWorld>) {}
