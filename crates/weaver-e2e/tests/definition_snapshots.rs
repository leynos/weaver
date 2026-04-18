//! Snapshot tests for definition lookup (`observe get-definition` functionality).
//!
//! These tests validate LSP-based definition lookup using Pyrefly for Python.
//! Tests are skipped gracefully if Pyrefly is not available.

#[path = "support/fixture_io.rs"]
mod fixture_io;
#[path = "definition_snapshots/fixtures_impl.rs"]
mod fixtures_impl;
#[path = "definition_snapshots/test_impl.rs"]
mod test_impl;

use std::path::Path;

use fixtures_impl::{linear_chain_context, python_class_context, python_functions_context};
use lsp_types::{GotoDefinitionResponse, Location, Uri};
use rstest::rstest;
use tempfile::TempDir;
use url::Url;
use weaver_e2e::{
    lsp_client::{LspClient, LspClientError},
    pyrefly_available,
};

/// Test error type for definition snapshot tests.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("LSP client error: {0}")]
    LspClient(#[from] LspClientError),

    #[error("invalid file path: cannot convert to URI")]
    InvalidFilePath,

    #[error("invalid URI: {0}")]
    InvalidUri(String),

    #[error("expected error but operation succeeded")]
    ExpectedError,

    #[error("expected NotInitialized error, got: {actual}")]
    WrongErrorType { actual: String },
}

/// Creates a file URI from a path, handling cross-platform differences correctly.
fn file_uri(path: &Path) -> Result<Uri, TestError> {
    let url = Url::from_file_path(path).map_err(|()| TestError::InvalidFilePath)?;
    url.as_str()
        .parse()
        .map_err(|_| TestError::InvalidUri(url.to_string()))
}

/// Skips the test if Pyrefly is not available.
macro_rules! require_pyrefly {
    () => {
        if !pyrefly_available() {
            eprintln!(
                "Skipping test: Pyrefly not available (install with `uv tool install pyrefly`)"
            );
            return Ok(());
        }
    };
}

fn assert_error_before_init<F, A>(mut action: F, assert_extra: A) -> Result<(), TestError>
where
    F: FnMut(&mut LspClient, &Uri) -> Result<(), LspClientError>,
    A: FnOnce(&LspClientError),
{
    require_pyrefly!();
    let (mut client, uri) = spawn_uninitialized_client()?;

    let err = match action(&mut client, &uri) {
        Ok(()) => return Err(TestError::ExpectedError),
        Err(error) => error,
    };

    match &err {
        LspClientError::NotInitialized => {}
        other => {
            return Err(TestError::WrongErrorType {
                actual: other.to_string(),
            });
        }
    }

    assert_extra(&err);
    drop(client.shutdown());
    Ok(())
}

/// Runs a test implementation with the given fixture context.
macro_rules! run_test_with_context {
    ($fixture:expr, $impl_fn:path) => {{
        require_pyrefly!();
        let Some(ctx) = $fixture.as_mut() else {
            panic!("context should exist when pyrefly is available");
        };
        $impl_fn(ctx)
    }};
}

/// Test context containing an initialised LSP client and file URIs.
///
/// Implements `Drop` to ensure the LSP client is shut down even on early panics.
struct TestContext {
    client: LspClient,
    file_uri: Uri,
    _temp_dir: TempDir,
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Attempt to shut down the client gracefully; ignore errors since
        // we may be dropping due to a panic or the server may have crashed.
        drop(self.client.shutdown());
    }
}

/// Simplified location for snapshot comparison.
///
/// Strips the temp directory path prefix for stable snapshots.
#[derive(Debug)]
#[expect(
    dead_code,
    reason = "fields used in debug output for snapshot comparison"
)]
struct LocationSnapshot {
    filename: String,
    start_line: u32,
    start_char: u32,
    end_line: u32,
    end_char: u32,
}

impl LocationSnapshot {
    fn from_location(loc: &Location) -> Self {
        let path = loc.uri.path().as_str();
        let filename = Path::new(path)
            .file_name()
            .map_or_else(|| path.to_owned(), |f| f.to_string_lossy().into_owned());
        Self {
            filename,
            start_line: loc.range.start.line,
            start_char: loc.range.start.character,
            end_line: loc.range.end.line,
            end_char: loc.range.end.character,
        }
    }

    fn from_link(link: &lsp_types::LocationLink) -> Self {
        Self::from_location(&Location {
            uri: link.target_uri.clone(),
            range: link.target_selection_range,
        })
    }

    fn collect_locations(locs: &[Location]) -> DefinitionSnapshot {
        match locs {
            [] => DefinitionSnapshot::None,
            [loc] => DefinitionSnapshot::Single(Self::from_location(loc)),
            _ => DefinitionSnapshot::Multiple(locs.iter().map(Self::from_location).collect()),
        }
    }

    fn collect_links(links: &[lsp_types::LocationLink]) -> DefinitionSnapshot {
        match links {
            [] => DefinitionSnapshot::None,
            [link] => DefinitionSnapshot::Single(Self::from_link(link)),
            _ => DefinitionSnapshot::Multiple(links.iter().map(Self::from_link).collect()),
        }
    }
}

/// Represents a definition result for snapshot comparison.
#[derive(Debug)]
#[expect(
    dead_code,
    reason = "variants used in debug output for snapshot comparison"
)]
enum DefinitionSnapshot {
    None,
    Single(LocationSnapshot),
    Multiple(Vec<LocationSnapshot>),
}

impl From<Option<GotoDefinitionResponse>> for DefinitionSnapshot {
    fn from(response: Option<GotoDefinitionResponse>) -> Self {
        match response {
            None => Self::None,
            Some(GotoDefinitionResponse::Scalar(loc)) => {
                Self::Single(LocationSnapshot::from_location(&loc))
            }
            Some(GotoDefinitionResponse::Array(locs)) => LocationSnapshot::collect_locations(&locs),
            Some(GotoDefinitionResponse::Link(links)) => LocationSnapshot::collect_links(&links),
        }
    }
}

/// Spawns an uninitialized LSP client for error testing.
fn spawn_uninitialized_client() -> Result<(LspClient, Uri), TestError> {
    let client = LspClient::spawn("uvx", &["pyrefly", "lsp"])?;
    let uri: Uri = "file:///tmp/test.py"
        .parse()
        .map_err(|_| TestError::InvalidUri("file:///tmp/test.py".to_owned()))?;
    Ok((client, uri))
}

#[rstest]
fn definition_from_call_to_function(
    mut linear_chain_context: Option<TestContext>,
) -> Result<(), TestError> {
    run_test_with_context!(
        linear_chain_context,
        test_impl::definition_from_call_to_function_impl
    )
}

#[rstest]
fn definition_at_function_definition(
    mut linear_chain_context: Option<TestContext>,
) -> Result<(), TestError> {
    run_test_with_context!(
        linear_chain_context,
        test_impl::definition_at_function_definition_impl
    )
}

#[rstest]
fn definition_self_method_call(
    mut python_class_context: Option<TestContext>,
) -> Result<(), TestError> {
    run_test_with_context!(
        python_class_context,
        test_impl::definition_self_method_call_impl
    )
}

#[rstest]
fn definition_class_method(mut python_class_context: Option<TestContext>) -> Result<(), TestError> {
    run_test_with_context!(
        python_class_context,
        test_impl::definition_class_method_impl
    )
}

#[rstest]
fn definition_class_name(mut python_class_context: Option<TestContext>) -> Result<(), TestError> {
    run_test_with_context!(python_class_context, test_impl::definition_class_name_impl)
}

#[rstest]
fn definition_parameter(
    mut python_functions_context: Option<TestContext>,
) -> Result<(), TestError> {
    run_test_with_context!(
        python_functions_context,
        test_impl::definition_parameter_impl
    )
}

#[rstest]
fn definition_on_whitespace(
    mut linear_chain_context: Option<TestContext>,
) -> Result<(), TestError> {
    run_test_with_context!(
        linear_chain_context,
        test_impl::definition_on_whitespace_impl
    )
}

#[test]
fn lsp_operation_before_init_returns_error() -> Result<(), TestError> {
    assert_error_before_init(
        |client, uri| client.did_open(uri.clone(), "python", "def foo(): pass"),
        |_| {},
    )
}

#[test]
fn lsp_goto_definition_before_init_returns_error() -> Result<(), TestError> {
    assert_error_before_init(
        |client, uri| client.goto_definition_at(uri, 0, 0).map(|_| ()),
        |_| {},
    )
}
