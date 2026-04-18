//! Pyrefly-backed fixtures for definition snapshot coverage.

use rstest::fixture;
use tempfile::TempDir;
use weaver_e2e::{fixtures, lsp_client::LspClient, pyrefly_available};

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
pub fn python_class_context() -> Result<Option<TestContext>, TestError> {
    create_test_context(fixtures::PYTHON_CLASS)
}

#[fixture]
pub fn python_functions_context() -> Result<Option<TestContext>, TestError> {
    create_test_context(fixtures::PYTHON_FUNCTIONS)
}
