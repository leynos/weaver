//! Tests that exercise the crate's public re-exports.

use std::sync::Arc;

use rstest::rstest;

use crate::{BackendKind, bootstrap_with};

use super::support::{
    HealthEvent, RecordingBackendProvider, RecordingHealthReporter, TestConfigLoader,
};

#[rstest]
fn bootstrap_with_reexport_initialises_daemon() {
    let loader = TestConfigLoader::new();
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();

    let daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .expect("bootstrap should succeed");

    assert!(daemon.config().daemon_socket().prepare_filesystem().is_ok());
    let events = reporter.events();
    assert!(events.contains(&HealthEvent::BootstrapStarting));
    assert!(events.contains(&HealthEvent::BootstrapSucceeded));
    assert!(provider.recorded_starts().is_empty());
}

#[rstest]
fn daemon_reexport_controls_backends() {
    let loader = TestConfigLoader::new();
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();
    let mut daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .expect("bootstrap should succeed");

    daemon
        .ensure_backend(BackendKind::Semantic)
        .expect("backend should start");
    assert_eq!(provider.recorded_starts(), vec![BackendKind::Semantic]);
}
