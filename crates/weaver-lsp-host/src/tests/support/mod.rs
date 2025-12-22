//! Shared fixtures and helpers for host tests.

mod recording_server;
mod world;

use std::str::FromStr;

use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, ReferenceContext, ReferenceParams, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri,
    VersionedTextDocumentIdentifier,
};
use rstest::fixture;

pub use recording_server::{CallKind, DocumentSyncErrors, RecordingLanguageServer, ResponseSet};
pub use world::{TestServerConfig, TestWorld};

/// Common URI used by host tests.
#[fixture]
pub fn sample_uri() -> Uri {
    Uri::from_str("file:///workspace/main.rs").expect("invalid test URI")
}

/// Builds a definition request for the sample URI.
#[must_use]
pub fn definition_params() -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: sample_uri() },
            position: lsp_types::Position::new(1, 2),
        },
        work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    }
}

/// Builds a references request for the sample URI.
#[must_use]
pub fn reference_params() -> ReferenceParams {
    ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: sample_uri() },
            position: lsp_types::Position::new(1, 2),
        },
        work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    }
}

/// Builds a did-open notification for the sample URI.
#[must_use]
pub fn did_open_params() -> DidOpenTextDocumentParams {
    DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: sample_uri(),
            language_id: String::from("rust"),
            version: 1,
            text: String::from("fn main() {}"),
        },
    }
}

/// Builds a did-change notification for the sample URI.
#[must_use]
pub fn did_change_params() -> DidChangeTextDocumentParams {
    DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: sample_uri(),
            version: 2,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: String::from("fn main() { println!(\"hi\"); }"),
        }],
    }
}

/// Builds a did-close notification for the sample URI.
#[must_use]
pub fn did_close_params() -> DidCloseTextDocumentParams {
    DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: sample_uri() },
    }
}
