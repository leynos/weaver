//! Unit tests for the daemon bootstrap utilities.

use std::sync::Arc;

use rstest::rstest;

use super::support::{
    HealthEvent,
    RecordingBackendProvider,
    RecordingHealthReporter,
    TestConfigLoader,
};
use crate::{BackendKind, bootstrap_with};

#[rstest]
fn bootstrap_does_not_eagerly_start_backends() -> Result<(), String> {
    let loader = TestConfigLoader::new()?;
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();

    bootstrap_with(&loader, reporter.clone(), provider.clone()).expect("bootstrap should succeed");

    let events = reporter.events();
    if !events.contains(&HealthEvent::BootstrapStarting) {
        return Err(format!("missing bootstrap start event: {events:?}"));
    }
    if !events.contains(&HealthEvent::BootstrapSucceeded) {
        return Err(format!("missing bootstrap success event: {events:?}"));
    }
    let starts = provider.recorded_starts();
    if !starts.is_empty() {
        return Err(format!("expected no backend starts, got: {starts:?}"));
    }
    Ok(())
}

#[rstest]
fn ensure_backend_starts_on_demand() -> Result<(), String> {
    let loader = TestConfigLoader::new()?;
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();
    let mut daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .expect("bootstrap should succeed");

    daemon
        .ensure_backend(BackendKind::Semantic)
        .expect("backend should start");
    let starts = provider.recorded_starts();
    if starts != vec![BackendKind::Semantic] {
        return Err(format!("expected semantic backend start, got: {starts:?}"));
    }
    let events = reporter.events();
    if !events.contains(&HealthEvent::BackendStarting(BackendKind::Semantic)) {
        return Err(format!("missing backend start event: {events:?}"));
    }
    if !events.contains(&HealthEvent::BackendReady(BackendKind::Semantic)) {
        return Err(format!("missing backend ready event: {events:?}"));
    }
    Ok(())
}

#[rstest]
fn ensure_backend_propagates_failures() -> Result<(), String> {
    let loader = TestConfigLoader::new()?;
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();
    provider.fail_on(BackendKind::Relational, "deliberate failure");
    let mut daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .expect("bootstrap should succeed");

    let error = daemon
        .ensure_backend(BackendKind::Relational)
        .expect_err("backend should fail");
    if error.kind != BackendKind::Relational {
        return Err(format!("expected relational backend error, got: {error:?}"));
    }
    let events = reporter.events();
    let failure_event = HealthEvent::BackendFailed {
        kind: BackendKind::Relational,
        message: "deliberate failure".to_owned(),
    };
    if !events.contains(&failure_event) {
        return Err(format!("missing backend failure event: {events:?}"));
    }
    Ok(())
}
