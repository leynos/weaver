//! Behavioural tests for the LSP host facade using `rstest-bdd`.
//!
//! The `then` steps live in [`super::behaviour_assertions`] so neither file
//! grows past the repository's module size budget.

use std::cell::RefCell;

use anyhow::{Context as _, Result};
use lsp_types::{Diagnostic, GotoDefinitionResponse, Location};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, when};
use weaver_config::{CapabilityMatrix, CapabilityOverride};

use crate::{
    capability::CapabilityKind,
    language::Language,
    server::ServerCapabilitySet,
    tests::support::{
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

#[fixture]
fn world() -> RefCell<TestWorld> {
    let initial = TestWorld::empty(CapabilityMatrix::default());
    RefCell::new(initial)
}

/// Replaces the world with one built from `configs`, preserving any
/// capability overrides already applied to the current world.
fn install_world(world: &RefCell<TestWorld>, configs: Vec<TestServerConfig>) -> Result<()> {
    let overrides = world.borrow().active_overrides();
    let rebuilt = TestWorld::new(configs, overrides).context("stub servers should register")?;
    *world.borrow_mut() = rebuilt;
    Ok(())
}

#[given("stub servers for all primary languages")]
fn given_all_languages(world: &RefCell<TestWorld>) -> Result<()> {
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

    install_world(world, configs)
}

#[given("a python server missing references")]
fn given_python_missing_references(world: &RefCell<TestWorld>) -> Result<()> {
    given_server_with_missing_capability(
        world,
        Language::Python,
        |responses| responses.references = Vec::new(),
        ServerCapabilitySet::new(true, false, true),
    )
}

#[given("a typescript server missing diagnostics")]
fn given_typescript_missing_diagnostics(world: &RefCell<TestWorld>) -> Result<()> {
    given_server_with_missing_capability(
        world,
        Language::TypeScript,
        |responses| responses.diagnostics = Vec::new(),
        ServerCapabilitySet::new(true, true, false),
    )
}

#[given("a rust server that fails during initialisation")]
fn given_rust_failure(world: &RefCell<TestWorld>) -> Result<()> {
    let configs = vec![TestServerConfig {
        language: Language::Rust,
        capabilities: ServerCapabilitySet::new(true, true, true),
        responses: sample_responses()?,
        initialization_error: Some(String::from("intentional init failure")),
    }];

    install_world(world, configs)
}

#[given("a rust server that fails during document sync")]
fn given_rust_document_sync_failure(world: &RefCell<TestWorld>) -> Result<()> {
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

    install_world(world, configs)
}

#[given("a deny override for python references")]
fn given_deny_override(world: &RefCell<TestWorld>) -> Result<()> {
    apply_override(
        world,
        Language::Python,
        CapabilityKind::References,
        CapabilityOverride::Deny,
    )
}

#[given("a force override for typescript diagnostics")]
fn given_force_override(world: &RefCell<TestWorld>) -> Result<()> {
    apply_override(
        world,
        Language::TypeScript,
        CapabilityKind::Diagnostics,
        CapabilityOverride::Force,
    )
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
fn when_typescript_diagnostics(world: &RefCell<TestWorld>) -> Result<()> {
    let uri = sample_uri()?;
    world
        .borrow_mut()
        .request_diagnostics(Language::TypeScript, uri);
    Ok(())
}

#[when("rust handles a definition request")]
fn when_rust_definition(world: &RefCell<TestWorld>) -> Result<()> {
    let params = definition_params()?;
    world
        .borrow_mut()
        .request_definition(Language::Rust, params);
    Ok(())
}

#[when("rust handles a references request")]
fn when_rust_references(world: &RefCell<TestWorld>) -> Result<()> {
    let params = reference_params()?;
    world
        .borrow_mut()
        .request_references(Language::Rust, params);
    Ok(())
}

#[when("rust handles a diagnostics request")]
fn when_rust_diagnostics(world: &RefCell<TestWorld>) -> Result<()> {
    let uri = sample_uri()?;
    world.borrow_mut().request_diagnostics(Language::Rust, uri);
    Ok(())
}

#[when("rust opens a document")]
fn when_rust_opens_document(world: &RefCell<TestWorld>) -> Result<()> {
    let params = did_open_params()?;
    world.borrow_mut().notify_did_open(Language::Rust, params);
    Ok(())
}

#[when("rust changes a document")]
fn when_rust_changes_document(world: &RefCell<TestWorld>) -> Result<()> {
    let params = did_change_params()?;
    world.borrow_mut().notify_did_change(Language::Rust, params);
    Ok(())
}

#[when("rust closes a document")]
fn when_rust_closes_document(world: &RefCell<TestWorld>) -> Result<()> {
    let params = did_close_params()?;
    world.borrow_mut().notify_did_close(Language::Rust, params);
    Ok(())
}

#[when("python handles a references request")]
fn when_python_references(world: &RefCell<TestWorld>) -> Result<()> {
    let params = reference_params()?;
    world
        .borrow_mut()
        .request_references(Language::Python, params);
    Ok(())
}

fn apply_override(
    world: &RefCell<TestWorld>,
    language: Language,
    capability: CapabilityKind,
    directive: CapabilityOverride,
) -> Result<()> {
    let mut overrides = CapabilityMatrix::default();
    overrides.set_override(language.as_str(), capability.key(), directive);
    world
        .borrow_mut()
        .rebuild_host(overrides)
        .context("host rebuild should register the stub servers")
}

fn given_server_with_missing_capability(
    world: &RefCell<TestWorld>,
    language: Language,
    modify_responses: impl FnOnce(&mut ResponseSet),
    capabilities: ServerCapabilitySet,
) -> Result<()> {
    let mut responses = sample_responses()?;
    modify_responses(&mut responses);
    let configs = vec![TestServerConfig {
        language,
        capabilities,
        responses,
        initialization_error: None,
    }];

    install_world(world, configs)
}

fn sample_responses() -> Result<ResponseSet> {
    let uri = sample_uri()?;
    Ok(ResponseSet {
        definition: GotoDefinitionResponse::Array(vec![Location {
            uri: uri.clone(),
            range: lsp_types::Range::default(),
        }]),
        references: vec![Location {
            uri,
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
