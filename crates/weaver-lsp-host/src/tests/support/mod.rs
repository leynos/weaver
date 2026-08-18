//! Shared fixtures and helpers for host tests.

mod recording_server;
mod world;

use std::str::FromStr;

use anyhow::{Context as _, Result};
use lsp_types::{
    DidChangeTextDocumentParams,
    DidCloseTextDocumentParams,
    DidOpenTextDocumentParams,
    GotoDefinitionParams,
    ReferenceContext,
    ReferenceParams,
    TextDocumentContentChangeEvent,
    TextDocumentIdentifier,
    TextDocumentItem,
    TextDocumentPositionParams,
    Uri,
    VersionedTextDocumentIdentifier,
};
pub use recording_server::{CallKind, DocumentSyncErrors, RecordingLanguageServer, ResponseSet};
use rstest::fixture;
use weaver_test_macros::allow_fixture_expansion_lints;
pub use world::{TestServerConfig, TestWorld};

/// Returns the sample file URI used by tests.
///
/// # Errors
///
/// Returns an error if the fixed sample URI fails to parse.
#[allow_fixture_expansion_lints]
#[fixture]
pub fn sample_uri() -> Result<Uri> {
    Uri::from_str("file:///workspace/main.rs").context("sample test URI should parse")
}

fn sample_text_document() -> Result<TextDocumentIdentifier> {
    Ok(TextDocumentIdentifier { uri: sample_uri()? })
}

/// Builds a definition request for the sample URI.
///
/// # Errors
///
/// Returns an error if the sample URI fails to parse.
pub fn definition_params() -> Result<GotoDefinitionParams> {
    Ok(GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: sample_text_document()?,
            position: lsp_types::Position::new(1, 2),
        },
        work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    })
}

/// Builds a references request for the sample URI.
///
/// # Errors
///
/// Returns an error if the sample URI fails to parse.
pub fn reference_params() -> Result<ReferenceParams> {
    Ok(ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: sample_text_document()?,
            position: lsp_types::Position::new(1, 2),
        },
        work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    })
}

/// Builds a did-open notification for the sample URI.
///
/// # Errors
///
/// Returns an error if the sample URI fails to parse.
pub fn did_open_params() -> Result<DidOpenTextDocumentParams> {
    Ok(DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: sample_uri()?,
            language_id: String::from("rust"),
            version: 1,
            text: String::from("fn main() {}"),
        },
    })
}

/// Builds a did-change notification for the sample URI.
///
/// # Errors
///
/// Returns an error if the sample URI fails to parse.
pub fn did_change_params() -> Result<DidChangeTextDocumentParams> {
    Ok(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: sample_uri()?,
            version: 2,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: String::from("fn main() { println!(\"hi\"); }"),
        }],
    })
}

/// Builds a did-close notification for the sample URI.
///
/// # Errors
///
/// Returns an error if the sample URI fails to parse.
pub fn did_close_params() -> Result<DidCloseTextDocumentParams> {
    Ok(DidCloseTextDocumentParams {
        text_document: sample_text_document()?,
    })
}
