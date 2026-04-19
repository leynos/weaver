//! Pyrefly-backed fixtures for definition snapshot coverage.

use rstest::fixture;
use tempfile::TempDir;
use weaver_e2e::{fixtures, lsp_client::LspClient, pyrefly_available};
use weaver_test_macros::allow_fixture_expansion_lints;

use super::{TestContext, TestError, file_uri, fixture_io};

/// Creates a test context with a Python fixture file opened in Pyrefly.
fn create_test_context(fixture_content: &str) -> Result<Option<TestContext>, TestError> {
    if !pyrefly_available() {
        return Ok(None);
    }

    let temp_dir = TempDir::new()?;
    let file_path = fixture_io::write_fixture_path(&temp_dir, "test.py", fixture_content)?;

    let root_uri = file_uri(temp_dir.path())?;
    let file_uri_val = file_uri(&file_path)?;

    let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"])?;
    if let Err(error) = client
        .initialize(root_uri)
        .and_then(|_| client.did_open(file_uri_val.clone(), "python", fixture_content))
    {
        client.shutdown().ok();
        return Err(error.into());
    }

    Ok(Some(TestContext {
        client,
        file_uri: file_uri_val,
        _temp_dir: temp_dir,
    }))
}

/// Provides a Pyrefly-backed context for the linear call-chain fixture.
///
/// Returns `Ok(Some(...))` when Pyrefly is available and the fixture is ready,
/// `Ok(None)` when the test should be skipped because Pyrefly is unavailable,
/// and `Err(...)` when fixture setup fails.
#[allow_fixture_expansion_lints]
#[fixture]
pub fn linear_chain_context() -> Result<Option<TestContext>, TestError> {
    create_test_context(fixtures::LINEAR_CHAIN)
}

/// Provides a Pyrefly-backed context for the Python class fixture.
///
/// Returns `Ok(Some(...))` when Pyrefly is available and the fixture is ready,
/// `Ok(None)` when the test should be skipped because Pyrefly is unavailable,
/// and `Err(...)` when fixture setup fails.
#[allow_fixture_expansion_lints]
#[fixture]
pub fn python_class_context() -> Result<Option<TestContext>, TestError> {
    create_test_context(fixtures::PYTHON_CLASS)
}

/// Provides a Pyrefly-backed context for the Python functions fixture.
///
/// Returns `Ok(Some(...))` when Pyrefly is available and the fixture is ready,
/// `Ok(None)` when the test should be skipped because Pyrefly is unavailable,
/// and `Err(...)` when fixture setup fails.
#[allow_fixture_expansion_lints]
#[fixture]
pub fn python_functions_context() -> Result<Option<TestContext>, TestError> {
    create_test_context(fixtures::PYTHON_FUNCTIONS)
}
