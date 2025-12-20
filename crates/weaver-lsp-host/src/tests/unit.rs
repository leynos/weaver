//! Unit tests for small host behaviours.

use std::str::FromStr;

use rstest::rstest;
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::capability::{CapabilityKind, CapabilitySource};
use crate::errors::HostOperation;
use crate::errors::LspHostError;
use crate::language::Language;
use crate::server::ServerCapabilitySet;
use crate::tests::support::{
    CallKind, RecordingLanguageServer, ResponseSet, TestWorld, sample_uri,
};

#[rstest]
fn applies_force_and_deny_overrides() {
    let mut overrides = CapabilityMatrix::default();
    overrides.set_override(
        Language::Rust.as_str(),
        CapabilityKind::Diagnostics.key(),
        CapabilityOverride::Deny,
    );
    overrides.set_override(
        Language::Rust.as_str(),
        CapabilityKind::References.key(),
        CapabilityOverride::Force,
    );

    let config = vec![crate::tests::support::TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, false, false),
        responses: ResponseSet::default(),
        initialization_error: None,
    }];
    let mut world = TestWorld::new(config, overrides);

    world.initialize(Language::Rust);
    let summary = world
        .last_capabilities
        .take()
        .expect("missing capabilities");

    let references = summary.state(CapabilityKind::References);
    assert!(references.enabled);
    assert_eq!(references.source, CapabilitySource::ForcedOverride);

    let diagnostics = summary.state(CapabilityKind::Diagnostics);
    assert!(!diagnostics.enabled);
    assert_eq!(diagnostics.source, CapabilitySource::DeniedOverride);
}

#[rstest]
fn parses_known_languages() {
    assert_eq!(
        Language::from_str("rust").expect("rust should parse"),
        Language::Rust
    );
    assert_eq!(
        Language::from_str("python").expect("python should parse"),
        Language::Python
    );
    assert_eq!(
        Language::from_str("typescript").expect("typescript should parse"),
        Language::TypeScript
    );
}

#[rstest]
fn parses_typescript_alias_ts() {
    assert_eq!(
        Language::from_str("ts").expect("ts alias should parse"),
        Language::TypeScript
    );
}

#[rstest]
fn trims_whitespace_in_language_parse() {
    assert_eq!(
        Language::from_str(" rust ").expect("padded rust should parse"),
        Language::Rust
    );
    assert_eq!(
        Language::from_str("\tpython\n").expect("padded python should parse"),
        Language::Python
    );
}

#[rstest]
fn rejects_invalid_language_with_message() {
    let err = Language::from_str("go").unwrap_err();
    assert_eq!(err.input(), "go");
    assert_eq!(err.to_string(), "unsupported language 'go'");
}

#[rstest]
fn rejects_duplicate_language_registration() {
    let server = RecordingLanguageServer::new(
        ServerCapabilitySet::new(true, true, true),
        ResponseSet::default(),
    );
    let mut host = crate::LspHost::new(CapabilityMatrix::default());

    assert!(
        host.register_language(Language::Rust, Box::new(server.clone()))
            .is_ok()
    );
    match host.register_language(Language::Rust, Box::new(server)) {
        Err(LspHostError::DuplicateLanguage { .. }) => {}
        other => panic!("expected duplicate language error, got {other:?}"),
    }
}

#[rstest]
fn reports_unknown_language_on_request() {
    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    match host.goto_definition(Language::Rust, definition_params()) {
        Err(LspHostError::UnknownLanguage { .. }) => {}
        other => panic!("expected unknown language error, got {other:?}"),
    }
}

#[rstest]
fn propagates_server_error_from_definition() {
    struct FailingServer;
    impl crate::server::LanguageServer for FailingServer {
        fn initialize(
            &mut self,
        ) -> Result<ServerCapabilitySet, crate::server::LanguageServerError> {
            Ok(ServerCapabilitySet::new(true, true, true))
        }

        fn goto_definition(
            &mut self,
            _params: lsp_types::GotoDefinitionParams,
        ) -> Result<lsp_types::GotoDefinitionResponse, crate::server::LanguageServerError> {
            Err(crate::server::LanguageServerError::new("boom"))
        }

        fn references(
            &mut self,
            _params: lsp_types::ReferenceParams,
        ) -> Result<Vec<lsp_types::Location>, crate::server::LanguageServerError> {
            Ok(Vec::new())
        }

        fn diagnostics(
            &mut self,
            _uri: lsp_types::Uri,
        ) -> Result<Vec<lsp_types::Diagnostic>, crate::server::LanguageServerError> {
            Ok(Vec::new())
        }

        fn did_open(
            &mut self,
            _params: lsp_types::DidOpenTextDocumentParams,
        ) -> Result<(), crate::server::LanguageServerError> {
            Ok(())
        }

        fn did_change(
            &mut self,
            _params: lsp_types::DidChangeTextDocumentParams,
        ) -> Result<(), crate::server::LanguageServerError> {
            Ok(())
        }

        fn did_close(
            &mut self,
            _params: lsp_types::DidCloseTextDocumentParams,
        ) -> Result<(), crate::server::LanguageServerError> {
            Ok(())
        }
    }

    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    host.register_language(Language::Rust, Box::new(FailingServer))
        .expect("registration failed");

    match host.goto_definition(Language::Rust, definition_params()) {
        Err(LspHostError::Server {
            language,
            operation,
            ..
        }) => {
            assert_eq!(language, Language::Rust);
            assert_eq!(operation, HostOperation::Definition);
        }
        other => panic!("expected server error, got {other:?}"),
    }
}

#[rstest]
fn propagates_server_error_from_did_change() {
    struct FailingServer;
    impl crate::server::LanguageServer for FailingServer {
        fn initialize(
            &mut self,
        ) -> Result<ServerCapabilitySet, crate::server::LanguageServerError> {
            Ok(ServerCapabilitySet::new(true, true, true))
        }

        fn goto_definition(
            &mut self,
            _params: lsp_types::GotoDefinitionParams,
        ) -> Result<lsp_types::GotoDefinitionResponse, crate::server::LanguageServerError> {
            Ok(lsp_types::GotoDefinitionResponse::Array(Vec::new()))
        }

        fn references(
            &mut self,
            _params: lsp_types::ReferenceParams,
        ) -> Result<Vec<lsp_types::Location>, crate::server::LanguageServerError> {
            Ok(Vec::new())
        }

        fn diagnostics(
            &mut self,
            _uri: lsp_types::Uri,
        ) -> Result<Vec<lsp_types::Diagnostic>, crate::server::LanguageServerError> {
            Ok(Vec::new())
        }

        fn did_open(
            &mut self,
            _params: lsp_types::DidOpenTextDocumentParams,
        ) -> Result<(), crate::server::LanguageServerError> {
            Ok(())
        }

        fn did_change(
            &mut self,
            _params: lsp_types::DidChangeTextDocumentParams,
        ) -> Result<(), crate::server::LanguageServerError> {
            Err(crate::server::LanguageServerError::new("change failed"))
        }

        fn did_close(
            &mut self,
            _params: lsp_types::DidCloseTextDocumentParams,
        ) -> Result<(), crate::server::LanguageServerError> {
            Ok(())
        }
    }

    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    host.register_language(Language::Rust, Box::new(FailingServer))
        .expect("registration failed");

    match host.did_change(Language::Rust, did_change_params()) {
        Err(LspHostError::Server {
            language,
            operation,
            ..
        }) => {
            assert_eq!(language, Language::Rust);
            assert_eq!(operation, HostOperation::DidChange);
        }
        other => panic!("expected server error, got {other:?}"),
    }
}

#[rstest]
fn calls_initialise_before_requests() {
    let responses = ResponseSet::default();
    let server =
        RecordingLanguageServer::new(ServerCapabilitySet::new(true, true, true), responses);
    let handle = server.handle();
    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    assert!(
        host.register_language(Language::Rust, Box::new(server))
            .is_ok()
    );

    let uri = sample_uri();
    let _ = host.diagnostics(Language::Rust, uri);

    let calls = handle.calls();
    assert!(
        calls.starts_with(&[CallKind::Initialise]),
        "initialise should precede requests: {calls:?}"
    );
}

#[rstest]
fn calls_initialise_before_document_sync() {
    let responses = ResponseSet::default();
    let server =
        RecordingLanguageServer::new(ServerCapabilitySet::new(true, true, true), responses);
    let handle = server.handle();
    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    assert!(
        host.register_language(Language::Rust, Box::new(server))
            .is_ok()
    );

    let _ = host.did_open(Language::Rust, did_open_params());

    let calls = handle.calls();
    assert!(
        calls.starts_with(&[CallKind::Initialise, CallKind::DidOpen]),
        "initialise should precede didOpen: {calls:?}"
    );
}

fn definition_params() -> lsp_types::GotoDefinitionParams {
    lsp_types::GotoDefinitionParams {
        text_document_position_params: lsp_types::TextDocumentPositionParams {
            text_document: lsp_types::TextDocumentIdentifier { uri: sample_uri() },
            position: lsp_types::Position::new(1, 2),
        },
        work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    }
}

fn did_open_params() -> lsp_types::DidOpenTextDocumentParams {
    lsp_types::DidOpenTextDocumentParams {
        text_document: lsp_types::TextDocumentItem {
            uri: sample_uri(),
            language_id: String::from("rust"),
            version: 1,
            text: String::from("fn main() {}"),
        },
    }
}

fn did_change_params() -> lsp_types::DidChangeTextDocumentParams {
    lsp_types::DidChangeTextDocumentParams {
        text_document: lsp_types::VersionedTextDocumentIdentifier {
            uri: sample_uri(),
            version: 2,
        },
        content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: String::from("fn main() { println!(\"hi\"); }"),
        }],
    }
}
