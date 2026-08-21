//! Behavioural tests for the LSP host facade using `rstest-bdd`.

use std::cell::RefCell;

use lsp_types::{Diagnostic, GotoDefinitionResponse, Location};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::{
    capability::{CapabilityKind, CapabilitySource},
    errors::{HostOperation, LspHostError},
    language::Language,
    server::ServerCapabilitySet,
    tests::support::{
        CallKind,
        DocumentSyncErrors,
        ResponseSet,
        TestServerConfig,
        TestWorld,
        definition_params,
        did_change_params,
        did_close_params,
        did_open_params,
        reference_params,
        sample_uri,
    },
};

type StepResult = Result<(), String>;

#[fixture]
fn world() -> RefCell<TestWorld> {
    RefCell::new(TestWorld::new(Vec::new(), CapabilityMatrix::default()))
}
#[given("stub servers for all primary languages")]
fn given_all_languages(world: &RefCell<TestWorld>) -> StepResult {
    let responses = sample_responses()?;
    let all_caps = ServerCapabilitySet::new(true, true, true)
        .with_call_hierarchy(true)
        .with_hover(true);
    let configs = vec![
        TestServerConfig {
            language: Language::Rust,
            capabilities: all_caps.clone(),
            responses: responses.clone(),
            initialization_error: None,
        },
        TestServerConfig {
            language: Language::Python,
            capabilities: all_caps.clone(),
            responses: responses.clone(),
            initialization_error: None,
        },
        TestServerConfig {
            language: Language::TypeScript,
            capabilities: all_caps,
            responses,
            initialization_error: None,
        },
    ];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
    Ok(())
}
#[given("a python server missing references")]
fn given_python_missing_references(world: &RefCell<TestWorld>) -> StepResult {
    given_server_with_missing_capability(
        world,
        Language::Python,
        |responses| responses.references = Vec::new(),
        ServerCapabilitySet::new(true, false, true),
    )
}

#[given("a typescript server missing diagnostics")]
fn given_typescript_missing_diagnostics(world: &RefCell<TestWorld>) -> StepResult {
    given_server_with_missing_capability(
        world,
        Language::TypeScript,
        |responses| responses.diagnostics = Vec::new(),
        ServerCapabilitySet::new(true, true, false),
    )
}

#[given("a rust server that fails during initialisation")]
fn given_rust_failure(world: &RefCell<TestWorld>) -> StepResult {
    let configs = vec![TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, true, true),
        responses: sample_responses()?,
        initialization_error: Some(String::from("intentional init failure")),
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
    Ok(())
}

#[given("a rust server that fails during document sync")]
fn given_rust_document_sync_failure(world: &RefCell<TestWorld>) -> StepResult {
    let mut responses = sample_responses()?;
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
    Ok(())
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
fn when_typescript_diagnostics(world: &RefCell<TestWorld>) -> StepResult {
    let uri = sample_uri()?;
    world
        .borrow_mut()
        .request_diagnostics(Language::TypeScript, uri);
    Ok(())
}
#[when("rust handles a definition request")]
fn when_rust_definition(world: &RefCell<TestWorld>) -> StepResult {
    world
        .borrow_mut()
        .request_definition(Language::Rust, definition_params()?);
    Ok(())
}
#[when("rust handles a references request")]
fn when_rust_references(world: &RefCell<TestWorld>) -> StepResult {
    world
        .borrow_mut()
        .request_references(Language::Rust, reference_params()?);
    Ok(())
}
#[when("rust handles a diagnostics request")]
fn when_rust_diagnostics(world: &RefCell<TestWorld>) -> StepResult {
    let uri = sample_uri()?;
    world.borrow_mut().request_diagnostics(Language::Rust, uri);
    Ok(())
}
#[when("rust opens a document")]
fn when_rust_opens_document(world: &RefCell<TestWorld>) -> StepResult {
    world
        .borrow_mut()
        .notify_did_open(Language::Rust, did_open_params()?);
    Ok(())
}
#[when("rust changes a document")]
fn when_rust_changes_document(world: &RefCell<TestWorld>) -> StepResult {
    world
        .borrow_mut()
        .notify_did_change(Language::Rust, did_change_params()?);
    Ok(())
}
#[when("rust closes a document")]
fn when_rust_closes_document(world: &RefCell<TestWorld>) -> StepResult {
    world
        .borrow_mut()
        .notify_did_close(Language::Rust, did_close_params()?);
    Ok(())
}
#[when("python handles a references request")]
fn when_python_references(world: &RefCell<TestWorld>) -> StepResult {
    world
        .borrow_mut()
        .request_references(Language::Python, reference_params()?);
    Ok(())
}
#[then("rust capabilities are available from the server")]
fn then_rust_capabilities(world: &RefCell<TestWorld>) -> StepResult {
    let borrow = world.borrow();
    let summary = borrow
        .last_capabilities
        .as_ref()
        .ok_or_else(|| String::from("capabilities missing"))?;

    for state in summary.states() {
        if !state.enabled {
            return Err(format!("capability {:?} should be enabled", state.kind));
        }
        if state.source != CapabilitySource::ServerAdvertised {
            return Err(format!(
                "expected {:?} to be server-advertised, got {:?}",
                state.kind, state.source
            ));
        }
    }
    Ok(())
}
#[then("rust recorded a definition call")]
fn then_rust_recorded_definition(world: &RefCell<TestWorld>) -> StepResult {
    assert_call_recorded(world, Language::Rust, CallKind::Definition)
}
#[then("rust recorded a references call")]
fn then_rust_recorded_references(world: &RefCell<TestWorld>) -> StepResult {
    assert_call_recorded(world, Language::Rust, CallKind::References)
}
#[then("rust recorded a diagnostics call")]
fn then_rust_recorded_diagnostics(world: &RefCell<TestWorld>) -> StepResult {
    assert_call_recorded(world, Language::Rust, CallKind::Diagnostics)
}
#[then("rust recorded a did open call")]
fn then_rust_recorded_did_open(world: &RefCell<TestWorld>) -> StepResult {
    assert_call_recorded(world, Language::Rust, CallKind::DidOpen)
}
#[then("rust recorded a did change call")]
fn then_rust_recorded_did_change(world: &RefCell<TestWorld>) -> StepResult {
    assert_call_recorded(world, Language::Rust, CallKind::DidChange)
}
#[then("rust recorded a did close call")]
fn then_rust_recorded_did_close(world: &RefCell<TestWorld>) -> StepResult {
    assert_call_recorded(world, Language::Rust, CallKind::DidClose)
}
#[then("diagnostics succeed via override")]
fn then_override_succeeds(world: &RefCell<TestWorld>) -> StepResult {
    let borrow = world.borrow();
    if borrow.last_error.is_some() {
        return Err(String::from("override should allow diagnostics"));
    }
    if borrow.last_diagnostics.as_ref().is_none_or(Vec::is_empty) {
        return Err(String::from("diagnostics should propagate"));
    }

    let summary = borrow
        .host
        .capabilities(Language::TypeScript)
        .ok_or_else(|| String::from("capability summary missing"))?;
    let diagnostics = summary.state(CapabilityKind::Diagnostics);
    if diagnostics.source == CapabilitySource::ForcedOverride {
        Ok(())
    } else {
        Err(format!(
            "expected forced diagnostics override, got {:?}",
            diagnostics.source
        ))
    }
}
#[then("the request fails with an unavailable capability error")]
fn then_missing_capability(world: &RefCell<TestWorld>) -> StepResult {
    let borrow = world.borrow();
    match &borrow.last_error {
        Some(LspHostError::CapabilityUnavailable {
            capability, reason, ..
        }) => {
            if *capability != CapabilityKind::References {
                return Err(format!(
                    "expected References capability, got {capability:?}"
                ));
            }
            if *reason != CapabilitySource::MissingOnServer {
                return Err(format!("expected missing-on-server reason, got {reason:?}"));
            }
            Ok(())
        }
        other => Err(format!("expected capability error, got {other:?}")),
    }
}
#[then("python recorded only initialisation")]
fn then_python_calls(world: &RefCell<TestWorld>) -> StepResult {
    let calls = world
        .borrow()
        .calls(Language::Python)
        .ok_or_else(|| String::from("calls missing"))?;
    if calls == [CallKind::Initialise] {
        Ok(())
    } else {
        Err(format!("expected only initialisation, got {calls:?}"))
    }
}
#[then("typescript recorded a diagnostics call")]
fn then_override_order(world: &RefCell<TestWorld>) -> StepResult {
    assert_call_recorded(world, Language::TypeScript, CallKind::Diagnostics)
}
#[then("the request fails with a server error")]
fn then_server_error(world: &RefCell<TestWorld>) -> StepResult {
    assert_server_error(world, HostOperation::Initialise)
}
#[then("the document sync request fails with a server error")]
fn then_document_sync_error(world: &RefCell<TestWorld>) -> StepResult {
    assert_server_error(world, HostOperation::DidChange)
}
fn assert_call_recorded(
    world: &RefCell<TestWorld>,
    language: Language,
    kind: CallKind,
) -> StepResult {
    let borrow = world.borrow();
    let calls = borrow
        .calls(language)
        .ok_or_else(|| String::from("missing calls for language"))?;
    if calls.contains(&kind) {
        Ok(())
    } else {
        Err(format!(
            "expected to record {kind:?} for {language}, got {calls:?}"
        ))
    }
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

fn given_server_with_missing_capability(
    world: &RefCell<TestWorld>,
    language: Language,
    modify_responses: impl FnOnce(&mut ResponseSet),
    capabilities: ServerCapabilitySet,
) -> StepResult {
    let mut responses = sample_responses()?;
    modify_responses(&mut responses);
    let configs = vec![TestServerConfig {
        language,
        capabilities,
        responses,
        initialization_error: None,
    }];

    *world.borrow_mut() = TestWorld::new(configs, CapabilityMatrix::default());
    Ok(())
}

fn assert_server_error(world: &RefCell<TestWorld>, operation: HostOperation) -> StepResult {
    let borrow = world.borrow();
    match &borrow.last_error {
        Some(LspHostError::Server {
            operation: observed_operation,
            ..
        }) if *observed_operation == operation => Ok(()),
        Some(LspHostError::Server {
            operation: observed_operation,
            ..
        }) => Err(format!(
            "expected {operation:?} server error, got {observed_operation:?}"
        )),
        other => Err(format!("expected server error, got {other:?}")),
    }
}

fn sample_responses() -> Result<ResponseSet, String> {
    Ok(ResponseSet {
        definition: GotoDefinitionResponse::Array(vec![Location {
            uri: sample_uri()?,
            range: lsp_types::Range::default(),
        }]),
        references: vec![Location {
            uri: sample_uri()?,
            range: lsp_types::Range::default(),
        }],
        diagnostics: vec![Diagnostic::default()],
        document_sync: DocumentSyncErrors::default(),
        call_hierarchy: Default::default(),
        hover: None,
    })
}

#[scenario(path = "tests/features/lsp_host.feature")]
fn lsp_host_behaviour(#[from(world)] _: RefCell<TestWorld>) {}
