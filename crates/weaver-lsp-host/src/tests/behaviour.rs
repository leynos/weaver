//! Behavioural tests for the LSP host facade using `rstest-bdd`.

use std::cell::RefCell;
use std::str::FromStr;

use lsp_types::{
    Diagnostic, GotoDefinitionParams, GotoDefinitionResponse, Location, PartialResultParams,
    Position, ReferenceContext, ReferenceParams, TextDocumentIdentifier, TextDocumentPositionParams,
    Uri, WorkDoneProgressParams,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::capability::{CapabilityKind, CapabilitySource};
use crate::errors::{HostOperation, LspHostError};
use crate::language::Language;
use crate::server::ServerCapabilitySet;
use crate::tests::support::{CallKind, ResponseSet, TestServerConfig, TestWorld};

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
            initialisation_error: None,
        },
        TestServerConfig {
            language: Language::Python,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses: responses.clone(),
            initialisation_error: None,
        },
        TestServerConfig {
            language: Language::TypeScript,
            capabilities: ServerCapabilitySet::new(true, true, true),
            responses,
            initialisation_error: None,
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
        initialisation_error: None,
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
        initialisation_error: None,
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
}

#[given("a rust server that fails during initialisation")]
fn given_rust_failure(world: &RefCell<TestWorld>) {
    let configs = vec![TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, true, true),
        responses: sample_responses(),
        initialisation_error: Some(String::from("intentional init failure")),
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
}

#[given("a deny override for python references")]
fn given_deny_override(world: &RefCell<TestWorld>) {
    apply_override(world, Language::Python, CapabilityKind::References, CapabilityOverride::Deny);
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
    world.borrow_mut().initialise(Language::Rust);
}

#[when("python is initialised")]
fn when_python_initialised(world: &RefCell<TestWorld>) {
    world.borrow_mut().initialise(Language::Python);
}

#[when("typescript handles a diagnostics request")]
fn when_typescript_diagnostics(world: &RefCell<TestWorld>) {
    let uri = sample_uri();
    world.borrow_mut().request_diagnostics(Language::TypeScript, uri);
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
        .unwrap_or_else(|| panic!("capabilities missing"));

    for state in summary.states() {
        assert!(state.enabled, "capability {:?} should be enabled", state.kind);
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

#[then("diagnostics succeed via override")]
fn then_override_succeeds(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    assert!(borrow.last_error.is_none(), "override should allow diagnostics");
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
        .unwrap_or_else(|| panic!("capability summary missing"));
    let diagnostics = summary.state(CapabilityKind::Diagnostics);
    assert_eq!(diagnostics.source, CapabilitySource::ForcedOverride);
}

#[then("the request fails with an unavailable capability error")]
fn then_missing_capability(world: &RefCell<TestWorld>) {
    let borrow = world.borrow();
    match &borrow.last_error {
        Some(LspHostError::CapabilityUnavailable { capability, .. }) => {
            assert_eq!(*capability, CapabilityKind::References);
        }
        other => panic!("expected capability error, got {other:?}"),
    }
}

#[then("python recorded only initialisation")]
fn then_python_calls(world: &RefCell<TestWorld>) {
    let calls = world
        .borrow()
        .calls(Language::Python)
        .unwrap_or_else(|| panic!("calls missing"));
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

fn assert_call_recorded(world: &RefCell<TestWorld>, language: Language, kind: CallKind) {
    let borrow = world.borrow();
    let calls = borrow
        .calls(language)
        .unwrap_or_else(|| panic!("missing calls for {language}"));
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
    }
}

fn sample_uri() -> Uri {
    Uri::from_str("file:///workspace/main.rs")
        .unwrap_or_else(|error| panic!("invalid test URL: {error}"))
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

#[scenario(path = "tests/features/lsp_host.feature")]
fn lsp_host_behaviour(#[from(world)] _: RefCell<TestWorld>) {}
