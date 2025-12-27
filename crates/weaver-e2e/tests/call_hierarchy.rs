//! End-to-end tests for call hierarchy functionality.
//!
//! These tests exercise the call hierarchy features against a real Pyrefly
//! language server. Tests are skipped gracefully if Pyrefly is not available.

use std::path::Path;

use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use rstest::{fixture, rstest};
use tempfile::TempDir;
use url::Url;

use weaver_e2e::fixtures;
use weaver_e2e::lsp_client::LspClient;
use weaver_e2e::pyrefly_available;

/// Creates a file URI from a path, handling cross-platform differences correctly.
#[expect(
    clippy::expect_used,
    reason = "test helper uses expect for infallible conversions"
)]
fn file_uri(path: &Path) -> Uri {
    let url = Url::from_file_path(path).expect("valid file path");
    url.as_str().parse().expect("valid URI")
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

/// Runs a test implementation with the given fixture context.
///
/// This macro handles the common pattern of:
/// 1. Checking if Pyrefly is available
/// 2. Unwrapping the fixture Option
/// 3. Delegating to the test implementation function
macro_rules! run_test_with_context {
    ($fixture:expr, $impl_fn:path) => {{
        require_pyrefly!();
        let Some(ctx) = $fixture.as_mut() else {
            panic!("context should exist when pyrefly is available");
        };
        $impl_fn(ctx);
    }};
}

/// Test context containing an initialized LSP client and file URIs.
struct TestContext {
    client: LspClient,
    file_uri: Uri,
    _temp_dir: TempDir,
}

/// Module containing fixtures with `expect_used` lint expectation.
#[expect(clippy::expect_used, reason = "fixtures use expect for setup")]
mod fixtures_impl {
    use super::*;

    /// Creates a test context with a Python fixture file opened in Pyrefly.
    fn create_test_context(fixture_content: &str) -> Option<TestContext> {
        if !pyrefly_available() {
            return None;
        }

        let temp_dir = TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("test.py");
        std::fs::write(&file_path, fixture_content).expect("write test file");

        let root_uri = file_uri(temp_dir.path());
        let file_uri_val = file_uri(&file_path);

        let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"]).expect("spawn pyrefly");
        client.initialize(root_uri).expect("initialize");
        client
            .did_open(file_uri_val.clone(), "python", fixture_content)
            .expect("open file");

        Some(TestContext {
            client,
            file_uri: file_uri_val,
            _temp_dir: temp_dir,
        })
    }

    /// Creates a test context with a linear call chain fixture opened in Pyrefly.
    #[fixture]
    pub fn linear_chain_context() -> Option<TestContext> {
        create_test_context(fixtures::LINEAR_CHAIN)
    }

    /// Creates a test context for standalone function tests.
    #[fixture]
    pub fn no_calls_context() -> Option<TestContext> {
        create_test_context(fixtures::NO_CALLS)
    }
}

use fixtures_impl::{linear_chain_context, no_calls_context};

/// Module containing test implementations with `expect_used` lint expectation.
#[expect(
    clippy::expect_used,
    reason = "test implementations use expect for assertions"
)]
mod test_impl {
    use super::*;
    use lsp_types::CallHierarchyItem;

    /// Prepares call hierarchy at the given position and returns the first item.
    fn prepare_call_hierarchy_item(
        ctx: &mut TestContext,
        line: u32,
        column: u32,
    ) -> CallHierarchyItem {
        let prepare_params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: ctx.file_uri.clone(),
                },
                position: Position::new(line, column),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        ctx.client
            .prepare_call_hierarchy(prepare_params)
            .expect("prepare")
            .expect("items")
            .into_iter()
            .next()
            .expect("first item")
    }

    pub fn prepare_call_hierarchy_finds_function_impl(ctx: &mut TestContext) {
        // Prepare call hierarchy at function `a` (line 0, column 4)
        let params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: ctx.file_uri.clone(),
                },
                position: Position::new(0, 4),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        let result = ctx
            .client
            .prepare_call_hierarchy(params)
            .expect("prepare call hierarchy");

        let items = result.expect("should find call hierarchy item");
        assert!(!items.is_empty(), "should have at least one item");
        assert_eq!(items.first().map(|i| i.name.as_str()), Some("a"));

        ctx.client.shutdown().expect("shutdown");
    }

    pub fn outgoing_calls_returns_callees_impl(ctx: &mut TestContext) {
        let item = prepare_call_hierarchy_item(ctx, 0, 4);

        let outgoing_params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let calls = ctx
            .client
            .outgoing_calls(outgoing_params)
            .expect("outgoing calls")
            .expect("should have outgoing calls");

        assert!(!calls.is_empty(), "should have at least one call");

        // `a` calls `b`
        let callee_names: Vec<_> = calls.iter().map(|c| c.to.name.as_str()).collect();
        assert!(
            callee_names.contains(&"b"),
            "should include call to `b`, got: {callee_names:?}"
        );

        ctx.client.shutdown().expect("shutdown");
    }

    pub fn incoming_calls_returns_callers_impl(ctx: &mut TestContext) {
        let item = prepare_call_hierarchy_item(ctx, 3, 4);

        let incoming_params = CallHierarchyIncomingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let calls = ctx
            .client
            .incoming_calls(incoming_params)
            .expect("incoming calls")
            .expect("should have incoming calls");

        assert!(!calls.is_empty(), "should have at least one call");

        // `b` is called by `a`
        let caller_names: Vec<_> = calls.iter().map(|c| c.from.name.as_str()).collect();
        assert!(
            caller_names.contains(&"a"),
            "should include call from `a`, got: {caller_names:?}"
        );

        ctx.client.shutdown().expect("shutdown");
    }

    pub fn no_calls_for_standalone_function_impl(ctx: &mut TestContext) {
        let item = prepare_call_hierarchy_item(ctx, 0, 4);

        // Check incoming calls - should be empty or None
        let incoming_params = CallHierarchyIncomingCallsParams {
            item: item.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let incoming = ctx
            .client
            .incoming_calls(incoming_params)
            .expect("incoming calls");

        let incoming_count = incoming.map_or(0, |c| c.len());
        assert_eq!(incoming_count, 0, "standalone should have no callers");

        // Check outgoing calls - should be empty or None (no user-defined function calls)
        // Note: outgoing may include built-in function calls, so we just check it doesn't error
        let outgoing_params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        ctx.client
            .outgoing_calls(outgoing_params)
            .expect("outgoing calls");

        ctx.client.shutdown().expect("shutdown");
    }
}

#[rstest]
fn prepare_call_hierarchy_finds_function(mut linear_chain_context: Option<TestContext>) {
    run_test_with_context!(
        linear_chain_context,
        test_impl::prepare_call_hierarchy_finds_function_impl
    );
}

#[rstest]
fn outgoing_calls_returns_callees(mut linear_chain_context: Option<TestContext>) {
    run_test_with_context!(
        linear_chain_context,
        test_impl::outgoing_calls_returns_callees_impl
    );
}

#[rstest]
fn incoming_calls_returns_callers(mut linear_chain_context: Option<TestContext>) {
    run_test_with_context!(
        linear_chain_context,
        test_impl::incoming_calls_returns_callers_impl
    );
}

#[rstest]
fn no_calls_for_standalone_function(mut no_calls_context: Option<TestContext>) {
    run_test_with_context!(
        no_calls_context,
        test_impl::no_calls_for_standalone_function_impl
    );
}
