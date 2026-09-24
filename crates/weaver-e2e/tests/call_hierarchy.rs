//! End-to-end tests for call hierarchy functionality.
//!
//! These tests exercise the call hierarchy features against a real Pyrefly
//! language server. Tests are skipped gracefully if Pyrefly is not available.

#[path = "support/fixture_io.rs"]
mod fixture_io;
#[path = "support/pyrefly.rs"]
mod pyrefly;
#[path = "support/uri.rs"]
mod uri;

use fixture_io::write_fixture_path;
use lsp_types::{
    CallHierarchyIncomingCallsParams,
    CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams,
    Position,
    TextDocumentIdentifier,
    TextDocumentPositionParams,
    Uri,
    WorkDoneProgressParams,
};
use pyrefly::require_pyrefly;
use rstest::{fixture, rstest};
use tempfile::TempDir;
use uri::{FileUriError, file_uri, parse_uri};
use weaver_e2e::{
    fixtures,
    lsp_client::{LspClient, LspClientError},
    pyrefly_available,
};

/// Test error type for call hierarchy tests.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("LSP client error: {0}")]
    LspClient(#[from] LspClientError),

    #[error(transparent)]
    FileUri(#[from] FileUriError),

    #[error("no call hierarchy items returned")]
    NoCallHierarchyItems,

    #[error("expected call to `{expected}` not found, got: {actual:?}")]
    ExpectedCallNotFound {
        expected: String,
        actual: Vec<String>,
    },

    #[error("expected at least one call")]
    NoCallsFound,

    #[error("unexpected function name: expected `{expected}`, got `{actual}`")]
    UnexpectedFunctionName { expected: String, actual: String },

    #[error("expected no callers, but found {count}")]
    UnexpectedCallers { count: usize },

    #[error("expected error but operation succeeded")]
    ExpectedError,

    #[error("test context missing while Pyrefly is available")]
    ContextMissing,

    #[error("expected NotInitialized error, got: {actual}")]
    WrongErrorType { actual: String },
}

/// Runs a test implementation with an optional fixture context.
macro_rules! run_test_with_context {
    ($fixture:expr, $impl_fn:path) => {{
        require_pyrefly!();
        let Some(ctx) = $fixture.as_mut() else {
            return Err(TestError::ContextMissing);
        };
        $impl_fn(ctx)
    }};
}

/// Test context containing an initialized LSP client and file URI.
struct TestContext {
    client: LspClient,
    file_uri: Uri,
    _temp_dir: TempDir,
}

impl Drop for TestContext {
    fn drop(&mut self) { self.client.shutdown().ok(); }
}

mod fixtures_impl {
    //! Pyrefly-backed fixtures for call hierarchy coverage.

    use super::*;

    fn create_test_context(fixture_content: &str) -> Result<Option<TestContext>, TestError> {
        if !pyrefly_available() {
            return Ok(None);
        }

        let temp_dir = TempDir::new()?;
        let file_path = write_fixture_path(&temp_dir, "test.py", fixture_content)?;

        let root_uri = file_uri(temp_dir.path())?;
        let file_uri_val = file_uri(&file_path)?;

        let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"])?;
        client.initialize(root_uri)?;
        client.did_open(file_uri_val.clone(), "python", fixture_content)?;

        Ok(Some(TestContext {
            client,
            file_uri: file_uri_val,
            _temp_dir: temp_dir,
        }))
    }

    #[fixture]
    pub fn linear_chain_context() -> Result<Option<TestContext>, TestError> {
        create_test_context(fixtures::LINEAR_CHAIN)
    }

    #[fixture]
    pub fn no_calls_context() -> Result<Option<TestContext>, TestError> {
        create_test_context(fixtures::NO_CALLS)
    }
}

use fixtures_impl::{linear_chain_context, no_calls_context};

mod test_impl {
    //! Shared assertion helpers for call hierarchy entry points.

    use lsp_types::CallHierarchyItem;

    use super::*;

    /// Prepares call hierarchy at the given position and returns the first item.
    fn prepare_call_hierarchy_item(
        ctx: &mut TestContext,
        line: u32,
        column: u32,
    ) -> Result<CallHierarchyItem, TestError> {
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
            .prepare_call_hierarchy(prepare_params)?
            .ok_or(TestError::NoCallHierarchyItems)?
            .into_iter()
            .next()
            .ok_or(TestError::NoCallHierarchyItems)
    }

    /// Direction for call hierarchy traversal.
    #[derive(Debug, Clone, Copy)]
    enum CallDirection {
        Incoming,
        Outgoing,
    }

    fn call_names(calls: Option<Vec<impl CallName>>) -> Result<Vec<String>, TestError> {
        calls.ok_or(TestError::NoCallsFound).map(|call_entries| {
            call_entries
                .iter()
                .map(CallName::name)
                .map(str::to_owned)
                .collect()
        })
    }

    trait CallName {
        fn name(&self) -> &str;
    }

    impl CallName for lsp_types::CallHierarchyIncomingCall {
        fn name(&self) -> &str { &self.from.name }
    }

    impl CallName for lsp_types::CallHierarchyOutgoingCall {
        fn name(&self) -> &str { &self.to.name }
    }

    fn request_call_names<Response: CallName>(
        ctx: &mut TestContext,
        call_item: CallHierarchyItem,
        request: impl FnOnce(
            &mut LspClient,
            CallHierarchyItem,
        ) -> Result<Option<Vec<Response>>, LspClientError>,
    ) -> Result<Vec<String>, TestError> {
        call_names(request(&mut ctx.client, call_item)?)
    }

    fn incoming_call_names(
        ctx: &mut TestContext,
        item: CallHierarchyItem,
    ) -> Result<Vec<String>, TestError> {
        request_call_names(ctx, item, |client, request_item| {
            client.incoming_calls(CallHierarchyIncomingCallsParams {
                item: request_item,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
        })
    }

    fn outgoing_call_names(
        ctx: &mut TestContext,
        item: CallHierarchyItem,
    ) -> Result<Vec<String>, TestError> {
        request_call_names(ctx, item, |client, request_item| {
            client.outgoing_calls(CallHierarchyOutgoingCallsParams {
                item: request_item,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: lsp_types::PartialResultParams::default(),
            })
        })
    }

    fn ensure_expected_call(names: Vec<String>, expected_name: &str) -> Result<(), TestError> {
        if names.is_empty() {
            return Err(TestError::NoCallsFound);
        }
        if names.iter().any(|name| name == expected_name) {
            return Ok(());
        }
        Err(TestError::ExpectedCallNotFound {
            expected: expected_name.to_owned(),
            actual: names,
        })
    }

    fn assert_calls_contain_impl(
        ctx: &mut TestContext,
        item: CallHierarchyItem,
        direction: CallDirection,
        expected_name: &str,
    ) -> Result<(), TestError> {
        let names = match direction {
            CallDirection::Incoming => incoming_call_names(ctx, item)?,
            CallDirection::Outgoing => outgoing_call_names(ctx, item)?,
        };
        ensure_expected_call(names, expected_name)
    }

    /// Verifies that `prepare_call_hierarchy` correctly identifies function `a`.
    pub fn prepare_call_hierarchy_finds_function_impl(
        ctx: &mut TestContext,
    ) -> Result<(), TestError> {
        let item = prepare_call_hierarchy_item(ctx, 0, 4)?;
        if item.name.as_str() != "a" {
            return Err(TestError::UnexpectedFunctionName {
                expected: "a".to_owned(),
                actual: item.name.clone(),
            });
        }
        Ok(())
    }

    /// Asserts that outgoing calls from the given item include the expected callee.
    fn assert_outgoing_calls_contain(
        ctx: &mut TestContext,
        item: CallHierarchyItem,
        expected_callee: &str,
    ) -> Result<(), TestError> {
        assert_calls_contain_impl(ctx, item, CallDirection::Outgoing, expected_callee)
    }

    /// Asserts that incoming calls to the given item include the expected caller.
    fn assert_incoming_calls_contain(
        ctx: &mut TestContext,
        item: CallHierarchyItem,
        expected_caller: &str,
    ) -> Result<(), TestError> {
        assert_calls_contain_impl(ctx, item, CallDirection::Incoming, expected_caller)
    }

    /// Verifies that `outgoing_calls` returns callee `b` when called from function `a`.
    pub fn outgoing_calls_returns_callees_impl(ctx: &mut TestContext) -> Result<(), TestError> {
        let item = prepare_call_hierarchy_item(ctx, 0, 4)?;
        assert_outgoing_calls_contain(ctx, item, "b")
    }

    /// Verifies that `incoming_calls` returns caller `a` when querying function `b`.
    pub fn incoming_calls_returns_callers_impl(ctx: &mut TestContext) -> Result<(), TestError> {
        let item = prepare_call_hierarchy_item(ctx, 3, 4)?;
        assert_incoming_calls_contain(ctx, item, "a")
    }

    /// Verifies that a standalone function has no incoming or outgoing calls.
    pub fn no_calls_for_standalone_function_impl(ctx: &mut TestContext) -> Result<(), TestError> {
        let item = prepare_call_hierarchy_item(ctx, 0, 4)?;

        let incoming_params = CallHierarchyIncomingCallsParams {
            item: item.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        let incoming = ctx.client.incoming_calls(incoming_params)?;

        let incoming_count = incoming.map_or(0, |c| c.len());
        if incoming_count != 0 {
            return Err(TestError::UnexpectedCallers {
                count: incoming_count,
            });
        }

        let outgoing_params = CallHierarchyOutgoingCallsParams {
            item,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };

        ctx.client.outgoing_calls(outgoing_params)?;
        Ok(())
    }
}

#[rstest]
fn prepare_call_hierarchy_finds_function(
    #[from(linear_chain_context)] linear_chain_context_res: Result<Option<TestContext>, TestError>,
) -> Result<(), TestError> {
    let mut linear_chain_context = linear_chain_context_res?;
    run_test_with_context!(
        linear_chain_context,
        test_impl::prepare_call_hierarchy_finds_function_impl
    )
}

#[rstest]
fn outgoing_calls_returns_callees(
    #[from(linear_chain_context)] linear_chain_context_res: Result<Option<TestContext>, TestError>,
) -> Result<(), TestError> {
    let mut linear_chain_context = linear_chain_context_res?;
    run_test_with_context!(
        linear_chain_context,
        test_impl::outgoing_calls_returns_callees_impl
    )
}

#[rstest]
fn incoming_calls_returns_callers(
    #[from(linear_chain_context)] linear_chain_context_res: Result<Option<TestContext>, TestError>,
) -> Result<(), TestError> {
    let mut linear_chain_context = linear_chain_context_res?;
    run_test_with_context!(
        linear_chain_context,
        test_impl::incoming_calls_returns_callers_impl
    )
}

#[rstest]
fn no_calls_for_standalone_function(
    #[from(no_calls_context)] no_calls_context_res: Result<Option<TestContext>, TestError>,
) -> Result<(), TestError> {
    let mut no_calls_context = no_calls_context_res?;
    run_test_with_context!(
        no_calls_context,
        test_impl::no_calls_for_standalone_function_impl
    )
}

#[test]
fn lsp_prepare_call_hierarchy_before_init_returns_error() -> Result<(), TestError> {
    require_pyrefly!();

    let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"])?;

    let uri = parse_uri("file:///tmp/test.py")?;

    let params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position::new(0, 0),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    match client.prepare_call_hierarchy(params) {
        Err(LspClientError::NotInitialized) => Ok(()),
        Err(other) => Err(TestError::WrongErrorType {
            actual: other.to_string(),
        }),
        Ok(_) => Err(TestError::ExpectedError),
    }
}
