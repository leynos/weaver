//! Shared fixtures and helpers for host tests.

mod recording_server;
mod world;

use std::str::FromStr;

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

/// Common URI used by host tests.
#[allow_fixture_expansion_lints]
#[fixture]
pub fn sample_uri() -> Result<Uri, String> {
    Uri::from_str("file:///workspace/main.rs").map_err(|error| format!("invalid test URI: {error}"))
}

/// Builds a definition request for the sample URI.
pub fn definition_params() -> Result<GotoDefinitionParams, String> {
    Ok(GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: sample_uri()? },
            position: lsp_types::Position::new(1, 2),
        },
        work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    })
}

/// Builds a references request for the sample URI.
pub fn reference_params() -> Result<ReferenceParams, String> {
    Ok(ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: sample_uri()? },
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
pub fn did_open_params() -> Result<DidOpenTextDocumentParams, String> {
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
pub fn did_change_params() -> Result<DidChangeTextDocumentParams, String> {
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
pub fn did_close_params() -> Result<DidCloseTextDocumentParams, String> {
    Ok(DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: sample_uri()? },
    })
}
