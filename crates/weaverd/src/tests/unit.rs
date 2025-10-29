//! Unit tests for the daemon bootstrap utilities.

use std::sync::Arc;

use rstest::rstest;

use crate::{BackendKind, bootstrap_with};

use super::support::{
    HealthEvent, RecordingBackendProvider, RecordingHealthReporter, TestConfigLoader,
};

#[rstest]
fn bootstrap_does_not_eagerly_start_backends() {
    let loader = TestConfigLoader::new();
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();

    bootstrap_with(&loader, reporter.clone(), provider.clone()).expect("bootstrap should succeed");

    let events = reporter.events();
    assert!(events.contains(&HealthEvent::BootstrapStarting));
    assert!(events.contains(&HealthEvent::BootstrapSucceeded));
    assert!(provider.recorded_starts().is_empty());
}

#[rstest]
fn ensure_backend_starts_on_demand() {
    let loader = TestConfigLoader::new();
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();
    let mut daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .expect("bootstrap should succeed");

    daemon
        .ensure_backend(BackendKind::Semantic)
        .expect("backend should start");
    assert_eq!(provider.recorded_starts(), vec![BackendKind::Semantic]);
    let events = reporter.events();
    assert!(events.contains(&HealthEvent::BackendStarting(BackendKind::Semantic)));
    assert!(events.contains(&HealthEvent::BackendReady(BackendKind::Semantic)));
}

#[rstest]
fn ensure_backend_propagates_failures() {
    let loader = TestConfigLoader::new();
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();
    provider.fail_on(BackendKind::Relational, "deliberate failure");
    let mut daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .expect("bootstrap should succeed");

    let error = daemon
        .ensure_backend(BackendKind::Relational)
        .expect_err("backend should fail");
    assert_eq!(error.kind, BackendKind::Relational);
    let events = reporter.events();
    assert!(events.contains(&HealthEvent::BackendFailed {
        kind: BackendKind::Relational,
        message: "deliberate failure".to_owned(),
    }));
}
