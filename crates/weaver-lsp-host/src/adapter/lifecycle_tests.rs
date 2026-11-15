//! Tests for the child-process cleanup diagnostics.

#![cfg(unix)]

use std::{
    collections::BTreeMap,
    os::unix::process::ExitStatusExt,
    process::ExitStatus,
    sync::{Arc, Mutex},
};

use tracing::{
    Event,
    Level,
    Subscriber,
    field::{Field, Visit},
    subscriber::with_default,
};
use tracing_subscriber::{
    Layer,
    layer::Context,
    prelude::*,
    registry::{LookupSpan, Registry},
};

use super::{
    ADAPTER_TARGET,
    DROP_DESCRIPTION,
    ReapableChild,
    UNRESPONSIVE_DESCRIPTION,
    kill_and_reap,
    kill_then_reap,
};
use crate::Language;

/// One tracing event captured while a cleanup path runs.
#[derive(Debug)]
struct CapturedEvent {
    /// Severity of the event.
    level: Level,
    /// Target the event was emitted to.
    target: String,
    /// Recorded fields, rendered with their `Debug` representation.
    fields: BTreeMap<String, String>,
}

/// Tracing layer that collects every event it observes.
struct RecordingLayer {
    /// Buffer shared with the test that owns the layer.
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for RecordingLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let captured = CapturedEvent {
            level: *event.metadata().level(),
            target: event.metadata().target().to_owned(),
            fields: visitor.fields,
        };
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        events.push(captured);
    }
}

/// Collects event fields as their `Debug` rendering.
#[derive(Default)]
struct FieldVisitor {
    /// Recorded fields, keyed by field name.
    fields: BTreeMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }
}

/// Runs `action` while recording the events emitted to any target.
///
/// The subscriber is scoped to the calling thread rather than installed
/// globally, so parallel tests keep their own event logs.
fn capture_events(action: impl FnOnce()) -> Vec<CapturedEvent> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = Registry::default().with(RecordingLayer {
        events: Arc::clone(&events),
    });

    with_default(subscriber, action);

    let mut recorded = events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::mem::take(&mut *recorded)
}

/// The process operation a [`FakeChild`] is told to refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refused {
    /// `kill` fails.
    Kill,
    /// `wait` fails.
    Wait,
}

/// Stands in for a child whose cleanup calls can be made to fail.
#[derive(Debug, Default)]
struct FakeChild {
    /// Operation this child refuses, if any.
    refused: Option<Refused>,
    /// Number of `kill` calls observed.
    kills: usize,
    /// Number of `wait` calls observed.
    waits: usize,
}

impl FakeChild {
    /// Returns a child that fails `refused` and succeeds otherwise.
    fn refusing(refused: Refused) -> Self {
        Self {
            refused: Some(refused),
            ..Self::default()
        }
    }

    /// Returns `Err` when `operation` is the refused one.
    fn refuse(&self, operation: Refused, message: &'static str) -> std::io::Result<()> {
        if self.refused == Some(operation) {
            return Err(std::io::Error::other(message));
        }
        Ok(())
    }
}

impl ReapableChild for FakeChild {
    fn kill(&mut self) -> std::io::Result<()> {
        self.kills += 1;
        self.refuse(Refused::Kill, "kill refused")
    }

    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.waits += 1;
        self.refuse(Refused::Wait, "wait refused")
            .map(|()| ExitStatus::from_raw(0))
    }
}

/// Asserts that `events` holds exactly one adapter warning with `message` and
/// `error` fields, so removing or rewording a diagnostic fails the test.
fn assert_warning(events: &[CapturedEvent], message: &str, error: &str) {
    assert_eq!(events.len(), 1, "expected one diagnostic, got {events:#?}");
    let event = events.first().expect("the single event was just checked");
    assert_eq!(event.level, Level::WARN, "the diagnostic must be a warning");
    assert_eq!(
        event.target, ADAPTER_TARGET,
        "the diagnostic must use the adapter target"
    );
    assert_eq!(
        event.fields.get("language").map(String::as_str),
        Some("rust"),
        "the diagnostic must name the language"
    );
    assert_eq!(
        event.fields.get("message").map(String::as_str),
        Some(message),
        "the diagnostic text must be stable"
    );
    assert_eq!(
        event.fields.get("error").map(String::as_str),
        Some(error),
        "the diagnostic must carry the failure"
    );
}

/// A failed kill is reported once, and the unresponsive path still reaps.
#[test]
fn kill_and_reap_warns_when_the_kill_fails() {
    let mut child = FakeChild::refusing(Refused::Kill);

    let events = capture_events(|| {
        kill_and_reap(&mut child, Language::Rust, UNRESPONSIVE_DESCRIPTION);
    });

    assert_warning(
        &events,
        "failed to kill unresponsive language server",
        "kill refused",
    );
    assert_eq!(
        child.waits, 1,
        "an unresponsive server is reaped even after a failed kill"
    );
}

/// A failed reap is reported once, after the kill was attempted.
#[test]
fn kill_and_reap_warns_when_the_reap_fails() {
    let mut child = FakeChild::refusing(Refused::Wait);

    let events = capture_events(|| {
        kill_and_reap(&mut child, Language::Rust, UNRESPONSIVE_DESCRIPTION);
    });

    assert_warning(
        &events,
        "failed to reap unresponsive language server",
        "wait refused",
    );
    assert_eq!(child.kills, 1, "the kill must happen before the reap");
}

/// A failed kill on the drop path is reported and the reap is skipped.
#[test]
fn kill_then_reap_warns_when_the_kill_fails_and_does_not_reap() {
    let mut child = FakeChild::refusing(Refused::Kill);

    let events = capture_events(|| {
        kill_then_reap(&mut child, Language::Rust, DROP_DESCRIPTION);
    });

    assert_warning(
        &events,
        "failed to kill language server process on drop",
        "kill refused",
    );
    assert_eq!(
        child.waits, 0,
        "the adapter must not wait on a process it failed to signal"
    );
}

/// A failed reap on the drop path is reported once.
#[test]
fn kill_then_reap_warns_when_the_reap_fails() {
    let mut child = FakeChild::refusing(Refused::Wait);

    let events = capture_events(|| {
        kill_then_reap(&mut child, Language::Rust, DROP_DESCRIPTION);
    });

    assert_warning(
        &events,
        "failed to reap language server process on drop",
        "wait refused",
    );
    assert_eq!(child.kills, 1, "the kill must happen before the reap");
}

/// A cleanup where both calls succeed emits no warning.
#[test]
fn a_successful_kill_and_reap_is_silent() {
    let mut child = FakeChild::default();

    let events = capture_events(|| {
        kill_and_reap(&mut child, Language::Rust, UNRESPONSIVE_DESCRIPTION);
    });

    assert!(
        events.is_empty(),
        "a successful cleanup must not warn, got {events:#?}"
    );
    assert_eq!((child.kills, child.waits), (1, 1));
}

/// A drop-path cleanup where both calls succeed emits no warning.
#[test]
fn a_successful_kill_then_reap_is_silent() {
    let mut child = FakeChild::default();

    let events = capture_events(|| {
        kill_then_reap(&mut child, Language::Rust, DROP_DESCRIPTION);
    });

    assert!(
        events.is_empty(),
        "a successful cleanup must not warn, got {events:#?}"
    );
    assert_eq!((child.kills, child.waits), (1, 1));
}
