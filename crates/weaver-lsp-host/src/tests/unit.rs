//! Unit tests for small host behaviours.

use std::cell::RefCell;
use std::str::FromStr;

use rstest::rstest;
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::capability::{CapabilityKind, CapabilitySource};
use crate::errors::LspHostError;
use crate::language::Language;
use crate::server::ServerCapabilitySet;
use crate::tests::support::{CallKind, RecordingLanguageServer, ResponseSet, TestWorld};

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
        initialisation_error: None,
    }];
    let mut world = TestWorld::new(config, overrides);

    world.initialise(Language::Rust);
    let summary = world
        .last_capabilities
        .take()
        .unwrap_or_else(|| panic!("missing capabilities"));

    let references = summary.state(CapabilityKind::References);
    assert!(references.enabled);
    assert_eq!(references.source, CapabilitySource::ForcedOverride);

    let diagnostics = summary.state(CapabilityKind::Diagnostics);
    assert!(!diagnostics.enabled);
    assert_eq!(diagnostics.source, CapabilitySource::DeniedOverride);
}

#[rstest]
fn rejects_duplicate_language_registration() {
    let server = RecordingLanguageServer::new(
        ServerCapabilitySet::new(true, true, true),
        ResponseSet::default(),
    );
    let mut host = crate::LspHost::new(CapabilityMatrix::default());

    assert!(host
        .register_language(Language::Rust, Box::new(server.clone()))
        .is_ok());
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
fn calls_initialise_before_requests() {
    let responses = ResponseSet::default();
    let server = RecordingLanguageServer::new(
        ServerCapabilitySet::new(true, true, true),
        responses,
    );
    let handle = server.handle();
    let mut host = crate::LspHost::new(CapabilityMatrix::default());
    assert!(host
        .register_language(Language::Rust, Box::new(server))
        .is_ok());

    let uri = sample_uri();
    let _ = host.diagnostics(Language::Rust, uri);

    let calls = handle.calls();
    assert!(
        calls.starts_with(&[CallKind::Initialise]),
        "initialise should precede requests: {calls:?}"
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

fn sample_uri() -> lsp_types::Uri {
    lsp_types::Uri::from_str("file:///workspace/main.rs")
        .unwrap_or_else(|error| panic!("invalid test URL: {error}"))
}
