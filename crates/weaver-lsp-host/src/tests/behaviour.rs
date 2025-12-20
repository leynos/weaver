//! Behavioural tests for the LSP host facade using `rstest-bdd`.

use std::cell::RefCell;

use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Location, PartialResultParams, Position,
    ReferenceContext, ReferenceParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::capability::{CapabilityKind, CapabilitySource};
use crate::errors::{HostOperation, LspHostError};
use crate::language::Language;
use crate::server::ServerCapabilitySet;
use crate::tests::support::{
    CallKind, DocumentSyncErrors, ResponseSet, TestServerConfig, TestWorld, sample_uri,
};

#[fixture]
fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::new(Vec::new(), CapabilityMatrix::default()))
}

#[given("stub servers for all primary languages")]
fn given_all_languages(world: &RefCell<TestWorld>) {
    let responses = sample_responses();
    let configs = vec![
        TestServerConfig {
            language: Language::Rust,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses: responses.clone(),
            initialization_error: None,
        },
        TestServerConfig {
            language: Language::Python,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses: responses.clone(),
            initialization_error: None,
        },
        TestServerConfig {
            language: Language::TypeScript,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses,
            initialization_error: None,
        },
    ];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
}

#[given("a python server missing references")]
fn given_python_missing_references(world: &RefCell<TestWorld>) {
    let mut responses = sample_responses();
    responses.references = Vec::new();
    let configs = vec![TestServerConfig {
        language: Language::Python,
        capabilities: ServerCapabilitySet::new(true, false, true),
        responses,
        initialization_error: None,
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
}

#[given("a typescript server missing diagnostics")]
fn given_typescript_missing_diagnostics(world: &RefCell<TestWorld>) {
    let mut responses = sample_responses();
    responses.diagnostics = Vec::new();
    let configs = vec![TestServerConfig {
        language: Language::TypeScript,
        capabilities: ServerCapabilitySet::new(true, true, false),
        responses,
        initialization_error: None,
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
}

#[given("a rust server that fails during initialisation")]
fn given_rust_failure(world: &RefCell<TestWorld>) {
    let configs = vec![TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, true, true),
        responses: sample_responses(),
        initialization_error: Some(String::from("intentional init failure")),
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
}

#[given("a rust server that fails during document sync")]
fn given_rust_document_sync_failure(world: &RefCell<TestWorld>) {
    let mut responses = sample_responses();
    responses.document_sync = DocumentSyncErrors {
        did_open_error: None,
        did_change_error: Some(String::from("intentional didChange failure")),
        did_close_error: None,
    };
    let configs = vec![TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, true, true),
        responses,
        initialization_error: None,
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
}

#[given("a deny override for python references")]
fn given_deny_override(world: &RefCell<TestWorld>) {
    apply_override(
        world,
        Language::Python,
        CapabilityKind::References,
        CapabilityOverride::Deny,
    );
}

#[given("a force override for typescript diagnostics")]
fn given_force_override(world: &RefCell<TestWorld>) {
    apply_override(
        world,
        Language::TypeScript,
        CapabilityKind::Diagnostics,
        CapabilityOverride::Force,
    );
}
#[when("rust is initialised")]
fn when_rust_initialised(world: &RefCell<TestWorld>) {
    world.borrow_mut().initialize(Language::Rust);
}
#[when("python is initialised")]
fn when_python_initialised(world: &RefCell<TestWorld>) {
    world.borrow_mut().initialize(Language::Python);
}
#[when("typescript handles a diagnostics request")]
fn when_typescript_diagnostics(world: &RefCell<TestWorld>) {
    let uri = sample_uri();
    world
        .borrow_mut()
        .request_diagnostics(Language::TypeScript, uri);
}
#[when("rust handles a definition request")]
fn when_rust_definition(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .request_definition(Language::Rust, definition_params());
}
#[when("rust handles a references request")]
fn when_rust_references(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .request_references(Language::Rust, reference_params());
}
#[when("rust handles a diagnostics request")]
fn when_rust_diagnostics(world: &RefCell<TestWorld>) {
    let uri = sample_uri();
    world.borrow_mut().request_diagnostics(Language::Rust, uri);
}
#[when("rust opens a document")]
fn when_rust_opens_document(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .notify_did_open(Language::Rust, did_open_params());
}
#[when("rust changes a document")]
fn when_rust_changes_document(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .notify_did_change(Language::Rust, did_change_params());
}
#[when("rust closes a document")]
fn when_rust_closes_document(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .notify_did_close(Language::Rust, did_close_params());
}
#[when("python handles a references request")]
fn when_python_references(world: &RefCell<TestWorld>) {
    world
        .borrow_mut()
        .request_references(Language::Python, reference_params());
}
#[then("rust capabilities are available from the server")]
fn then_rust_capabilities(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    let summary = borrow
        .last_capabilities
        .as_ref()
        .expect("capabilities missing");

    for state in summary.states() {
        assert!(
            state.enabled,
            "capability {:?} should be enabled",
            state.kind
        );
        assert_eq!(state.source, CapabilitySource::ServerAdvertised);
    }
}
#[then("rust recorded a definition call")]
fn then_rust_recorded_definition(world: &RefCell<TestWorld>) {
    assert_call_recorded(world, Language::Rust, CallKind::Definition);
}
#[then("rust recorded a references call")]
fn then_rust_recorded_references(world: &RefCell<TestWorld>) {
    assert_call_recorded(world, Language::Rust, CallKind::References);
}
#[then("rust recorded a diagnostics call")]
fn then_rust_recorded_diagnostics(world: &RefCell<TestWorld>) {
    assert_call_recorded(world, Language::Rust, CallKind::Diagnostics);
}
#[then("rust recorded a did open call")]
fn then_rust_recorded_did_open(world: &RefCell<TestWorld>) {
    assert_call_recorded(world, Language::Rust, CallKind::DidOpen);
}
#[then("rust recorded a did change call")]
fn then_rust_recorded_did_change(world: &RefCell<TestWorld>) {
    assert_call_recorded(world, Language::Rust, CallKind::DidChange);
}
#[then("rust recorded a did close call")]
fn then_rust_recorded_did_close(world: &RefCell<TestWorld>) {
    assert_call_recorded(world, Language::Rust, CallKind::DidClose);
}
#[then("diagnostics succeed via override")]
fn then_override_succeeds(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    assert!(
        borrow.last_error.is_none(),
        "override should allow diagnostics"
    );
    assert!(
        borrow
            .last_diagnostics
            .as_ref()
            .map(|set| !set.is_empty())
            .unwrap_or(false),
        "diagnostics should propagate"
    );

    let summary = borrow
        .host
        .capabilities(Language::TypeScript)
        .expect("capability summary missing");
    let diagnostics = summary.state(CapabilityKind::Diagnostics);
    assert_eq!(diagnostics.source, CapabilitySource::ForcedOverride);
}
#[then("the request fails with an unavailable capability error")]
fn then_missing_capability(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    match &borrow.last_error {
        Some(LspHostError::CapabilityUnavailable {
            capability, reason, ..
        }) => {
            assert_eq!(*capability, CapabilityKind::References);
            assert_eq!(
                *reason,
                CapabilitySource::MissingOnServer,
                "unexpected capability unavailability reason for References"
            );
        }
        other => panic!("expected capability error, got {other:?}"),
    }
}
#[then("python recorded only initialisation")]
fn then_python_calls(world: &RefCell<TestWorld>) {
    let calls = world
        .borrow()
        .calls(Language::Python)
        .expect("calls missing");
    assert_eq!(calls, [CallKind::Initialise]);
}
#[then("typescript recorded a diagnostics call")]
fn then_override_order(world: &RefCell<TestWorld>) {
    assert_call_recorded(world, Language::TypeScript, CallKind::Diagnostics);
}
#[then("the request fails with a server error")]
fn then_server_error(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    match &borrow.last_error {
        Some(LspHostError::Server {
            operation: HostOperation::Initialise,
            ..
        }) => {}
        other => panic!("expected server error, got {other:?}"),
    }
}
#[then("the document sync request fails with a server error")]
fn then_document_sync_error(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    match &borrow.last_error {
        Some(LspHostError::Server {
            operation: HostOperation::DidChange,
            ..
        }) => {}
        other => panic!("expected document sync server error, got {other:?}"),
    }
}
fn assert_call_recorded(world: &RefCell<TestWorld>, language: Language, kind: CallKind) {
    let borrow = world.borrow();
    let calls = borrow.calls(language).expect("missing calls for language");
    assert!(
        calls.contains(&kind),
        "expected to record {kind:?} for {language}, got {calls:?}"
    );
}

fn apply_override(
    world: &RefCell<TestWorld>,
    language: Language,
    capability: CapabilityKind,
    directive: CapabilityOverride,
) {
    let mut overrides = CapabilityMatrix::default();
    overrides.set_override(language.as_str(), capability.key(), directive);
    world.borrow_mut().rebuild_host(overrides);
}

fn sample_responses() -> ResponseSet {
    ResponseSet {
        definition: GotoDefinitionResponse::Array(vec![Location {
            uri: sample_uri(),
            range: lsp_types::Range::default(),
        }]),
        references: vec![Location {
            uri: sample_uri(),
            range: lsp_types::Range::default(),
        }],
        diagnostics: vec![Diagnostic::default()],
        document_sync: DocumentSyncErrors::default(),
    }
}

fn definition_params() -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: sample_uri() },
            position: Position::new(1, 2),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }
}

fn reference_params() -> ReferenceParams {
    ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: sample_uri() },
            position: Position::new(1, 2),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: ReferenceContext {
            include_declaration: false,
        },
    }
}

fn did_open_params() -> DidOpenTextDocumentParams {
    DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: sample_uri(),
            language_id: String::from("rust"),
            version: 1,
            text: String::from("fn main() {}"),
        },
    }
}

fn did_change_params() -> DidChangeTextDocumentParams {
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

fn did_close_params() -> DidCloseTextDocumentParams {
    DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: sample_uri() },
    }
}

#[scenario(path = "tests/features/lsp_host.feature")]
fn lsp_host_behaviour(#[from(world)] _: RefCell<TestWorld>) {}
