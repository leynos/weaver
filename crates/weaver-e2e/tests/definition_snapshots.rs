//! Snapshot tests for definition lookup (`observe get-definition` functionality).
//!
//! These tests validate LSP-based definition lookup using Pyrefly for Python.
//! Tests are skipped gracefully if Pyrefly is not available.

use std::path::Path;

use insta::assert_debug_snapshot;
use lsp_types::{GotoDefinitionResponse, Location, Uri};
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
macro_rules! run_test_with_context {
    ($fixture:expr, $impl_fn:path) => {{
        require_pyrefly!();
        let Some(ctx) = $fixture.as_mut() else {
            panic!("context should exist when pyrefly is available");
        };
        $impl_fn(ctx);
    }};
}

/// Test context containing an initialised LSP client and file URIs.
struct TestContext {
    client: LspClient,
    file_uri: Uri,
    _temp_dir: TempDir,
}

/// Simplified location for snapshot comparison.
///
/// Strips the temp directory path prefix for stable snapshots.
#[derive(Debug)]
struct LocationSnapshot {
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    filename: String,
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    start_line: u32,
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    start_char: u32,
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
    end_line: u32,
    #[expect(
        dead_code,
        reason = "field used in debug output for snapshot comparison"
    )]
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
        let loc = Location {
            uri: link.target_uri.clone(),
            range: link.target_selection_range,
        };
        Self::from_location(&loc)
    }
}

/// Represents a definition result for snapshot comparison.
#[derive(Debug)]
enum DefinitionSnapshot {
    None,
    #[expect(
        dead_code,
        reason = "variant used in debug output for snapshot comparison"
    )]
    Single(LocationSnapshot),
    #[expect(
        dead_code,
        reason = "variant used in debug output for snapshot comparison"
    )]
    Multiple(Vec<LocationSnapshot>),
}

#[expect(clippy::indexing_slicing, reason = "we check length before indexing")]
impl From<Option<GotoDefinitionResponse>> for DefinitionSnapshot {
    fn from(response: Option<GotoDefinitionResponse>) -> Self {
        match response {
            None => Self::None,
            Some(GotoDefinitionResponse::Scalar(loc)) => {
                Self::Single(LocationSnapshot::from_location(&loc))
            }
            Some(GotoDefinitionResponse::Array(locs)) if locs.is_empty() => Self::None,
            Some(GotoDefinitionResponse::Array(locs)) if locs.len() == 1 => {
                Self::Single(LocationSnapshot::from_location(&locs[0]))
            }
            Some(GotoDefinitionResponse::Array(locs)) => {
                Self::Multiple(locs.iter().map(LocationSnapshot::from_location).collect())
            }
            Some(GotoDefinitionResponse::Link(links)) if links.is_empty() => Self::None,
            Some(GotoDefinitionResponse::Link(links)) if links.len() == 1 => {
                Self::Single(LocationSnapshot::from_link(&links[0]))
            }
            Some(GotoDefinitionResponse::Link(links)) => {
                Self::Multiple(links.iter().map(LocationSnapshot::from_link).collect())
            }
        }
    }
}

/// Module containing fixtures for definition tests.
mod fixtures_impl {
    use super::*;

    /// Creates a test context with a Python fixture file opened in Pyrefly.
    #[expect(
        clippy::expect_used,
        reason = "fixture setup uses expect for infallible operations"
    )]
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

    /// Creates a test context with a linear call chain fixture.
    #[fixture]
    pub fn linear_chain_context() -> Option<TestContext> {
        create_test_context(fixtures::LINEAR_CHAIN)
    }

    /// Creates a test context with Python class fixture.
    #[fixture]
    pub fn python_class_context() -> Option<TestContext> {
        create_test_context(fixtures::PYTHON_CLASS)
    }

    /// Creates a test context with Python functions fixture.
    #[fixture]
    pub fn python_functions_context() -> Option<TestContext> {
        create_test_context(fixtures::PYTHON_FUNCTIONS)
    }
}

use fixtures_impl::{linear_chain_context, python_class_context, python_functions_context};

/// Module containing test implementations.
mod test_impl {
    use super::*;

    /// Gets definition at the given position and returns a snapshot.
    #[expect(
        clippy::expect_used,
        reason = "test helper uses expect for LSP operations"
    )]
    fn get_definition_snapshot(
        ctx: &mut TestContext,
        line: u32,
        character: u32,
    ) -> DefinitionSnapshot {
        let response = ctx
            .client
            .goto_definition_at(ctx.file_uri.clone(), line, character)
            .expect("goto_definition");
        DefinitionSnapshot::from(response)
    }

    /// Tests definition lookup from a function call to its definition.
    #[expect(clippy::expect_used, reason = "test uses expect for LSP operations")]
    pub fn definition_from_call_to_function_impl(ctx: &mut TestContext) {
        // In LINEAR_CHAIN: def a() calls b() on line 1, character ~4
        // b() is defined on line 3
        let snapshot = get_definition_snapshot(ctx, 1, 4);
        assert_debug_snapshot!("definition_from_call_to_function", snapshot);
        ctx.client.shutdown().expect("shutdown");
    }

    /// Tests definition lookup for a function name at its definition site.
    #[expect(clippy::expect_used, reason = "test uses expect for LSP operations")]
    pub fn definition_at_function_definition_impl(ctx: &mut TestContext) {
        // In LINEAR_CHAIN: def a() is on line 0, character 4
        let snapshot = get_definition_snapshot(ctx, 0, 4);
        assert_debug_snapshot!("definition_at_function_definition", snapshot);
        ctx.client.shutdown().expect("shutdown");
    }

    /// Tests definition lookup for a method call on self.
    #[expect(clippy::expect_used, reason = "test uses expect for LSP operations")]
    pub fn definition_self_method_call_impl(ctx: &mut TestContext) {
        // In PYTHON_CLASS: self.validate() is called on line 2
        // validate is defined on line 5
        let snapshot = get_definition_snapshot(ctx, 2, 25);
        assert_debug_snapshot!("definition_self_method_call", snapshot);
        ctx.client.shutdown().expect("shutdown");
    }

    /// Tests definition lookup for a class method definition.
    #[expect(clippy::expect_used, reason = "test uses expect for LSP operations")]
    pub fn definition_class_method_impl(ctx: &mut TestContext) {
        // In PYTHON_CLASS: def process(self, data) on line 1
        let snapshot = get_definition_snapshot(ctx, 1, 8);
        assert_debug_snapshot!("definition_class_method", snapshot);
        ctx.client.shutdown().expect("shutdown");
    }

    /// Tests definition lookup for the class name.
    #[expect(clippy::expect_used, reason = "test uses expect for LSP operations")]
    pub fn definition_class_name_impl(ctx: &mut TestContext) {
        // In PYTHON_CLASS: class Service on line 0
        let snapshot = get_definition_snapshot(ctx, 0, 6);
        assert_debug_snapshot!("definition_class_name", snapshot);
        ctx.client.shutdown().expect("shutdown");
    }

    /// Tests definition lookup for a parameter.
    #[expect(clippy::expect_used, reason = "test uses expect for LSP operations")]
    pub fn definition_parameter_impl(ctx: &mut TestContext) {
        // In PYTHON_FUNCTIONS: def greet(name) - name parameter on line 0
        let snapshot = get_definition_snapshot(ctx, 0, 10);
        assert_debug_snapshot!("definition_parameter", snapshot);
        ctx.client.shutdown().expect("shutdown");
    }

    /// Tests definition lookup on whitespace (should return None).
    #[expect(clippy::expect_used, reason = "test uses expect for LSP operations")]
    pub fn definition_on_whitespace_impl(ctx: &mut TestContext) {
        // Position on whitespace/indentation
        let snapshot = get_definition_snapshot(ctx, 1, 0);
        assert_debug_snapshot!("definition_on_whitespace", snapshot);
        ctx.client.shutdown().expect("shutdown");
    }
}

// =============================================================================
// Test Entry Points
// =============================================================================

#[rstest]
fn definition_from_call_to_function(mut linear_chain_context: Option<TestContext>) {
    run_test_with_context!(
        linear_chain_context,
        test_impl::definition_from_call_to_function_impl
    );
}

#[rstest]
fn definition_at_function_definition(mut linear_chain_context: Option<TestContext>) {
    run_test_with_context!(
        linear_chain_context,
        test_impl::definition_at_function_definition_impl
    );
}

#[rstest]
fn definition_self_method_call(mut python_class_context: Option<TestContext>) {
    run_test_with_context!(
        python_class_context,
        test_impl::definition_self_method_call_impl
    );
}

#[rstest]
fn definition_class_method(mut python_class_context: Option<TestContext>) {
    run_test_with_context!(
        python_class_context,
        test_impl::definition_class_method_impl
    );
}

#[rstest]
fn definition_class_name(mut python_class_context: Option<TestContext>) {
    run_test_with_context!(python_class_context, test_impl::definition_class_name_impl);
}

#[rstest]
fn definition_parameter(mut python_functions_context: Option<TestContext>) {
    run_test_with_context!(
        python_functions_context,
        test_impl::definition_parameter_impl
    );
}

#[rstest]
fn definition_on_whitespace(mut linear_chain_context: Option<TestContext>) {
    run_test_with_context!(
        linear_chain_context,
        test_impl::definition_on_whitespace_impl
    );
}
