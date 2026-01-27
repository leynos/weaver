//! End-to-end tests for human-readable output rendering.

use std::path::Path;

use lsp_types::{GotoDefinitionResponse, Location, Uri};
use tempfile::TempDir;
use url::Url;

use weaver_cli::{OutputContext, render_human_output};
use weaver_e2e::fixtures;
use weaver_e2e::lsp_client::{LspClient, LspClientError};

/// Test error type for human output rendering.
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

    #[error("no definition location returned")]
    NoDefinition,

    #[error("failed to render human output")]
    RenderFailed,

    #[error("expected output to contain {0}")]
    MissingOutput(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(serde::Serialize)]
struct DefinitionLocation {
    uri: String,
    line: u32,
    column: u32,
}

fn file_uri(path: &Path) -> Result<Uri, TestError> {
    let url = Url::from_file_path(path).map_err(|()| TestError::InvalidFilePath)?;
    url.as_str()
        .parse()
        .map_err(|_| TestError::InvalidUri(url.to_string()))
}

fn first_location(response: Option<GotoDefinitionResponse>) -> Option<Location> {
    let definition = response?;
    match definition {
        GotoDefinitionResponse::Scalar(location) => Some(location),
        GotoDefinitionResponse::Array(locations) => locations.into_iter().next(),
        GotoDefinitionResponse::Link(links) => links.into_iter().next().map(|link| Location {
            uri: link.target_uri,
            range: link.target_selection_range,
        }),
    }
}

#[test]
#[ignore = "requires pyrefly tooling to be available on PATH"]
fn renders_definition_output_with_context() -> Result<(), TestError> {
    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.py");
    std::fs::write(&file_path, fixtures::LINEAR_CHAIN)?;

    let root_uri = file_uri(temp_dir.path())?;
    let file_uri_value = file_uri(&file_path)?;

    let mut client = LspClient::spawn("uvx", &["pyrefly", "lsp"])?;
    client.initialize(root_uri)?;
    client.did_open(file_uri_value.clone(), "python", fixtures::LINEAR_CHAIN)?;

    let response = client.goto_definition_at(&file_uri_value, 1, 4)?;
    let location = first_location(response).ok_or(TestError::NoDefinition)?;

    let payload = vec![DefinitionLocation {
        uri: location.uri.to_string(),
        line: location.range.start.line + 1,
        column: location.range.start.character + 1,
    }];
    let json = serde_json::to_string(&payload)?;

    let context = OutputContext::new("observe", "get-definition", Vec::new());
    let rendered = render_human_output(&context, &json).ok_or(TestError::RenderFailed)?;
    let normalised = rendered.replace(temp_dir.path().to_string_lossy().as_ref(), "<temp>");

    if !normalised.contains("def b():") {
        return Err(TestError::MissingOutput(String::from("def b():")));
    }
    if !normalised.contains("^ definition") {
        return Err(TestError::MissingOutput(String::from("^ definition")));
    }

    client.shutdown()?;

    Ok(())
}
