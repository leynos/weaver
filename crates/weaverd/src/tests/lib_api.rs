//! Tests that exercise the crate's public re-exports.

use std::sync::Arc;

use rstest::rstest;

use super::support::{
    HealthEvent,
    RecordingBackendProvider,
    RecordingHealthReporter,
    TestConfigLoader,
};
use crate::{BackendKind, bootstrap_with};

/// Verifies that the public `bootstrap_with` re-export correctly initialises
/// the daemon without eagerly starting backends.
#[rstest]
fn bootstrap_with_reexport_initialises_daemon() -> Result<(), String> {
    let loader = TestConfigLoader::new()?;
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();

    let _daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .map_err(|error| format!("bootstrap should succeed: {error}"))?;

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
fn daemon_reexport_controls_backends() -> Result<(), String> {
    let loader = TestConfigLoader::new()?;
    let reporter = Arc::new(RecordingHealthReporter::default());
    let provider = RecordingBackendProvider::default();
    let mut daemon = bootstrap_with(&loader, reporter.clone(), provider.clone())
        .map_err(|error| format!("bootstrap should succeed: {error}"))?;

    daemon
        .ensure_backend(BackendKind::Semantic)
        .map_err(|error| format!("backend should start: {error}"))?;
    let starts = provider.recorded_starts();
    if starts != vec![BackendKind::Semantic] {
        return Err(format!("expected semantic backend start, got: {starts:?}"));
    }
    Ok(())
}
