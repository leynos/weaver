//! Tests for the child-process cleanup diagnostics.

#![cfg(unix)]

use std::{
    collections::BTreeMap,
    os::unix::process::ExitStatusExt,
    process::ExitStatus,
    sync::{Arc, Mutex},
};

use rstest::rstest;
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

/// The cleanup path a case exercises.
///
/// The two helpers differ in one respect that matters here: the unresponsive
/// path reaps even when the kill failed, because the server must not outlive
/// the deadline, while the drop path stops after a failed signal.
#[derive(Clone, Copy, Debug)]
enum Cleanup {
    /// [`kill_and_reap`], used when a server misses its shutdown deadline.
    Unresponsive,
    /// [`kill_then_reap`], used when an adapter is dropped mid-conversation.
    OnDrop,
}

impl Cleanup {
    /// Description the cleanup helper logs for this path.
    #[must_use]
    const fn description(self) -> &'static str {
        match self {
            Self::Unresponsive => UNRESPONSIVE_DESCRIPTION,
            Self::OnDrop => DROP_DESCRIPTION,
        }
    }

    /// Runs this path against `child`.
    fn run(self, child: &mut FakeChild) {
        match self {
            Self::Unresponsive => kill_and_reap(child, Language::Rust, self.description()),
            Self::OnDrop => kill_then_reap(child, Language::Rust, self.description()),
        }
    }
}

/// The process operation a [`FakeChild`] is told to refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refused {
    /// `kill` fails.
    Kill,
    /// `wait` fails.
    Wait,
}

impl Refused {
    /// Verb the cleanup diagnostic uses for this operation.
    #[must_use]
    const fn verb(self) -> &'static str {
        match self {
            Self::Kill => "kill",
            Self::Wait => "reap",
        }
    }

    /// Text the fake child reports when it refuses.
    #[must_use]
    const fn message(self) -> &'static str {
        match self {
            Self::Kill => "kill refused",
            Self::Wait => "wait refused",
        }
    }
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
    fn refuse(&self, operation: Refused) -> std::io::Result<()> {
        if self.refused == Some(operation) {
            return Err(std::io::Error::other(operation.message()));
        }
        Ok(())
    }
}

impl ReapableChild for FakeChild {
    fn kill(&mut self) -> std::io::Result<()> {
        self.kills += 1;
        self.refuse(Refused::Kill)
    }

    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.waits += 1;
        self.refuse(Refused::Wait).map(|()| ExitStatus::from_raw(0))
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

/// A refused cleanup call is reported once, and the path decides whether the
/// reap still runs.
#[rstest]
#[case::unresponsive_kill_refused(Cleanup::Unresponsive, Refused::Kill, 1)]
#[case::unresponsive_reap_refused(Cleanup::Unresponsive, Refused::Wait, 1)]
#[case::drop_kill_refused(Cleanup::OnDrop, Refused::Kill, 0)]
#[case::drop_reap_refused(Cleanup::OnDrop, Refused::Wait, 1)]
fn a_refused_operation_warns(
    #[case] cleanup: Cleanup,
    #[case] refused: Refused,
    #[case] expected_waits: usize,
) {
    let mut child = FakeChild::refusing(refused);

    let events = capture_events(|| cleanup.run(&mut child));

    let expected = format!("failed to {} {}", refused.verb(), cleanup.description());
    assert_warning(&events, &expected, refused.message());
    assert_eq!(child.kills, 1, "every path attempts the kill once");
    assert_eq!(
        child.waits, expected_waits,
        concat!(
            "an unresponsive server is reaped even after a failed kill, and ",
            "a dropped one is not waited on after one"
        )
    );
}

/// A cleanup where both calls succeed emits no warning.
#[rstest]
#[case::unresponsive(Cleanup::Unresponsive)]
#[case::on_drop(Cleanup::OnDrop)]
fn a_successful_cleanup_is_silent(#[case] cleanup: Cleanup) {
    let mut child = FakeChild::default();

    let events = capture_events(|| cleanup.run(&mut child));

    assert!(
        events.is_empty(),
        "a successful cleanup must not warn, got {events:#?}"
    );
    assert_eq!((child.kills, child.waits), (1, 1));
}
