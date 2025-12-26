//! End-to-end tests for call hierarchy functionality.
//!
//! These tests exercise the call hierarchy features against a real Pyrefly
//! language server. Tests are skipped gracefully if Pyrefly is not available.

#![expect(clippy::expect_used, reason = "test code uses expect for assertions")]

use std::str::FromStr;

use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use tempfile::TempDir;

use weaver_e2e::fixtures;
use weaver_e2e::lsp_client::LspClient;
use weaver_e2e::pyrefly_available;

/// Creates a file URI from a path string.
fn file_uri(path: &str) -> Uri {
    Uri::from_str(&format!("file://{path}")).expect("valid URI")
}

/// Skips the test if Pyrefly is not available.
macro_rules! require_pyrefly {
    () => {
        if !pyrefly_available() {
            eprintln!(
                "Skipping test: Pyrefly not available (install with `uv tool install pyrefly`)"
            );
            return;
        }
    };
}

#[test]
fn prepare_call_hierarchy_finds_function() {
    require_pyrefly!();

    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("test.py");
    std::fs::write(&file_path, fixtures::LINEAR_CHAIN).expect("write test file");

    let root_uri = file_uri(temp_dir.path().to_str().expect("valid path"));
    let file_uri = file_uri(file_path.to_str().expect("valid path"));

    let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"]).expect("spawn pyrefly");
    client.initialize(root_uri).expect("initialize");
    client
        .did_open(file_uri.clone(), "python", fixtures::LINEAR_CHAIN)
        .expect("open file");

    // Prepare call hierarchy at function `a` (line 0, column 4)
    let params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: file_uri },
            position: Position::new(0, 4),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let result = client
        .prepare_call_hierarchy(params)
        .expect("prepare call hierarchy");

    let items = result.expect("should find call hierarchy item");
    assert!(!items.is_empty(), "should have at least one item");
    assert_eq!(items.first().map(|i| i.name.as_str()), Some("a"));

    client.shutdown().expect("shutdown");
}

#[test]
fn outgoing_calls_returns_callees() {
    require_pyrefly!();

    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("test.py");
    std::fs::write(&file_path, fixtures::LINEAR_CHAIN).expect("write test file");

    let root_uri = file_uri(temp_dir.path().to_str().expect("valid path"));
    let file_uri = file_uri(file_path.to_str().expect("valid path"));

    let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"]).expect("spawn pyrefly");
    client.initialize(root_uri).expect("initialize");
    client
        .did_open(file_uri.clone(), "python", fixtures::LINEAR_CHAIN)
        .expect("open file");

    // First prepare call hierarchy for function `a`
    let prepare_params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.clone(),
            },
            position: Position::new(0, 4),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let items = client
        .prepare_call_hierarchy(prepare_params)
        .expect("prepare")
        .expect("items");

    let item = items.into_iter().next().expect("first item");

    // Now get outgoing calls
    let outgoing_params = CallHierarchyOutgoingCallsParams {
        item,
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };

    let outgoing = client
        .outgoing_calls(outgoing_params)
        .expect("outgoing calls");

    let calls = outgoing.expect("should have outgoing calls");
    assert!(!calls.is_empty(), "should have at least one call");

    // `a` calls `b`
    let callee_names: Vec<_> = calls.iter().map(|c| c.to.name.as_str()).collect();
    assert!(
        callee_names.contains(&"b"),
        "should include call to `b`, got: {callee_names:?}"
    );

    client.shutdown().expect("shutdown");
}

#[test]
fn incoming_calls_returns_callers() {
    require_pyrefly!();

    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("test.py");
    std::fs::write(&file_path, fixtures::LINEAR_CHAIN).expect("write test file");

    let root_uri = file_uri(temp_dir.path().to_str().expect("valid path"));
    let file_uri = file_uri(file_path.to_str().expect("valid path"));

    let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"]).expect("spawn pyrefly");
    client.initialize(root_uri).expect("initialize");
    client
        .did_open(file_uri.clone(), "python", fixtures::LINEAR_CHAIN)
        .expect("open file");

    // Prepare call hierarchy for function `b` (line 3, column 4)
    let prepare_params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.clone(),
            },
            position: Position::new(3, 4),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let items = client
        .prepare_call_hierarchy(prepare_params)
        .expect("prepare")
        .expect("items");

    let item = items.into_iter().next().expect("first item");

    // Now get incoming calls
    let incoming_params = CallHierarchyIncomingCallsParams {
        item,
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };

    let incoming = client
        .incoming_calls(incoming_params)
        .expect("incoming calls");

    let calls = incoming.expect("should have incoming calls");
    assert!(!calls.is_empty(), "should have at least one call");

    // `b` is called by `a`
    let caller_names: Vec<_> = calls.iter().map(|c| c.from.name.as_str()).collect();
    assert!(
        caller_names.contains(&"a"),
        "should include call from `a`, got: {caller_names:?}"
    );

    client.shutdown().expect("shutdown");
}

#[test]
fn no_calls_for_standalone_function() {
    require_pyrefly!();

    let temp_dir = TempDir::new().expect("create temp dir");
    let file_path = temp_dir.path().join("test.py");
    std::fs::write(&file_path, fixtures::NO_CALLS).expect("write test file");

    let root_uri = file_uri(temp_dir.path().to_str().expect("valid path"));
    let file_uri = file_uri(file_path.to_str().expect("valid path"));

    let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"]).expect("spawn pyrefly");
    client.initialize(root_uri).expect("initialize");
    client
        .did_open(file_uri.clone(), "python", fixtures::NO_CALLS)
        .expect("open file");

    // Prepare call hierarchy for function `standalone`
    let prepare_params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.clone(),
            },
            position: Position::new(0, 4),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let items = client
        .prepare_call_hierarchy(prepare_params)
        .expect("prepare")
        .expect("items");

    let item = items.into_iter().next().expect("first item");

    // Check incoming calls - should be empty or None
    let incoming_params = CallHierarchyIncomingCallsParams {
        item: item.clone(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };

    let incoming = client
        .incoming_calls(incoming_params)
        .expect("incoming calls");

    let incoming_count = incoming.map_or(0, |c| c.len());
    assert_eq!(incoming_count, 0, "standalone should have no callers");

    // Check outgoing calls - should be empty or None (no user-defined function calls)
    let outgoing_params = CallHierarchyOutgoingCallsParams {
        item,
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };

    let outgoing = client
        .outgoing_calls(outgoing_params)
        .expect("outgoing calls");

    // Note: outgoing may include built-in function calls, so we just check it doesn't error
    let _ = outgoing;

    client.shutdown().expect("shutdown");
}
