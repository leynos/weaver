//! `then` steps for the LSP host behavioural suite.
//!
//! Kept apart from [`super::behaviour`] so the arrange/act steps and the
//! assertions each stay within the module size budget.

use std::cell::RefCell;

use anyhow::{Context as _, Result, ensure};
use rstest_bdd_macros::then;

use crate::{
    capability::{CapabilityKind, CapabilitySource},
    errors::{HostOperation, LspHostError},
    language::Language,
    tests::support::{CallKind, TestWorld},
};

#[then("rust capabilities are available from the server")]
fn then_rust_capabilities(world: &RefCell<TestWorld>) -> Result<()> {
    let borrow = world.borrow();
    let summary = borrow
        .last_capabilities
        .as_ref()
        .context("capabilities missing")?;

    for state in summary.states() {
        ensure!(
            state.enabled,
            "capability {:?} should be enabled",
            state.kind
        );
        ensure!(
            state.source == CapabilitySource::ServerAdvertised,
            "capability {:?} should come from the server, got {:?}",
            state.kind,
            state.source
        );
    }
    Ok(())
}

#[then("rust recorded a definition call")]
fn then_rust_recorded_definition(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_call_recorded(world, Language::Rust, CallKind::Definition)
}

#[then("rust recorded a references call")]
fn then_rust_recorded_references(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_call_recorded(world, Language::Rust, CallKind::References)
}

#[then("rust recorded a diagnostics call")]
fn then_rust_recorded_diagnostics(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_call_recorded(world, Language::Rust, CallKind::Diagnostics)
}

#[then("rust recorded a did open call")]
fn then_rust_recorded_did_open(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_call_recorded(world, Language::Rust, CallKind::DidOpen)
}

#[then("rust recorded a did change call")]
fn then_rust_recorded_did_change(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_call_recorded(world, Language::Rust, CallKind::DidChange)
}

#[then("rust recorded a did close call")]
fn then_rust_recorded_did_close(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_call_recorded(world, Language::Rust, CallKind::DidClose)
}

#[then("diagnostics succeed via override")]
fn then_override_succeeds(world: &RefCell<TestWorld>) -> Result<()> {
    let borrow = world.borrow();
    ensure!(
        borrow.last_error.is_none(),
        "override should allow diagnostics, got {:?}",
        borrow.last_error
    );
    ensure!(
        borrow
            .last_diagnostics
            .as_ref()
            .is_some_and(|set| !set.is_empty()),
        "diagnostics should propagate"
    );

    let summary = borrow
        .host
        .capabilities(Language::TypeScript)
        .context("capability summary missing")?;
    let diagnostics = summary.state(CapabilityKind::Diagnostics);
    ensure!(
        diagnostics.source == CapabilitySource::ForcedOverride,
        "expected a forced override, got {:?}",
        diagnostics.source
    );
    Ok(())
}

#[then("the request fails with an unavailable capability error")]
fn then_missing_capability(world: &RefCell<TestWorld>) -> Result<()> {
    let borrow = world.borrow();
    let Some(LspHostError::CapabilityUnavailable {
        capability, reason, ..
    }) = borrow.last_error.as_ref()
    else {
        anyhow::bail!("expected capability error, got {:?}", borrow.last_error);
    };

    ensure!(
        *capability == CapabilityKind::References,
        "expected a References capability failure, got {capability:?}"
    );
    ensure!(
        *reason == CapabilitySource::MissingOnServer,
        "unexpected capability unavailability reason for References: {reason:?}"
    );
    Ok(())
}

#[then("python recorded only initialisation")]
fn then_python_calls(world: &RefCell<TestWorld>) -> Result<()> {
    let calls = world
        .borrow()
        .calls(Language::Python)
        .context("calls missing")?;
    ensure!(
        calls == [CallKind::Initialise],
        "python should record initialisation alone, got {calls:?}"
    );
    Ok(())
}

#[then("typescript recorded a diagnostics call")]
fn then_override_order(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_call_recorded(world, Language::TypeScript, CallKind::Diagnostics)
}

#[then("the request fails with a server error")]
fn then_server_error(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_server_error(world, HostOperation::Initialise)
}

#[then("the document sync request fails with a server error")]
fn then_document_sync_error(world: &RefCell<TestWorld>) -> Result<()> {
    ensure_server_error(world, HostOperation::DidChange)
}

fn ensure_call_recorded(
    world: &RefCell<TestWorld>,
    language: Language,
    kind: CallKind,
) -> Result<()> {
    let borrow = world.borrow();
    let calls = borrow
        .calls(language)
        .context("missing calls for language")?;
    ensure!(
        calls.contains(&kind),
        "expected to record {kind:?} for {language}, got {calls:?}"
    );
    Ok(())
}

fn ensure_server_error(world: &RefCell<TestWorld>, operation: HostOperation) -> Result<()> {
    let borrow = world.borrow();
    let Some(LspHostError::Server {
        operation: observed_operation,
        ..
    }) = borrow.last_error.as_ref()
    else {
        anyhow::bail!("expected server error, got {:?}", borrow.last_error);
    };
    ensure!(
        *observed_operation == operation,
        "expected a {operation:?} failure, got {observed_operation:?}"
    );
    Ok(())
}
