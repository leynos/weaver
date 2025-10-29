//! Tests that exercise the crate's public re-exports.

use std::sync::Arc;

use rstest::rstest;

use crate::{BackendKind, bootstrap_with};

use super::support::{
    HealthEvent, RecordingBackendProvider, RecordingHealthReporter, TestConfigLoader,
};

/// Verifies that the public `bootstrap_with` re-export correctly initialises
/// the daemon without eagerly starting backends.
#[rstest]
fn bootstrap_with_reexport_initialises_daemon() {
    let loader = TestConfigLoader::new();
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();

    let _daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .expect("bootstrap should succeed");

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
