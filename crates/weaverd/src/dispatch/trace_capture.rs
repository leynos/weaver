//! Tracing capture harness shared by `weaverd`'s dispatch unit tests.
//!
//! Dispatch logging is part of the daemon's operator contract, so handler tests
//! assert on the events they emit. Each capture installs a recording subscriber
//! as a scoped dispatcher, which keeps assertions independent of the global
//! telemetry subscriber and of tests running in parallel.
//!
//! The harness is test-only: production code must never install a global
//! subscriber outside `telemetry`, and must not depend on this module.

use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex},
};

use tracing::{
    Dispatch,
    Event,
    Level,
    Subscriber,
    dispatcher,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer,
    layer::Context,
    prelude::*,
    registry::{LookupSpan, Registry},
};

/// Captured tracing event recorded during a dispatch test.
#[derive(Debug)]
pub(crate) struct CapturedEvent {
    pub(crate) level: Level,
    pub(crate) target: String,
    pub(crate) fields: BTreeMap<String, String>,
    /// Names of the spans enclosing the event, outermost first.
    ///
    /// Recorded so a test can prove an event belongs to the span that promises
    /// to give it request context, rather than only that the span exists.
    pub(crate) span_path: Vec<String>,
}

/// Captured tracing span recorded during a dispatch test.
///
/// A span is captured when it is created, so a test can assert that request
/// handling is instrumented even when the event it explains is only emitted on
/// a conditional branch.
#[derive(Debug)]
pub(crate) struct CapturedSpan {
    pub(crate) level: Level,
    pub(crate) target: String,
    pub(crate) name: String,
    pub(crate) fields: BTreeMap<String, String>,
}

/// Everything a recording layer captured during one `capture` call.
#[derive(Debug, Default)]
pub(crate) struct CapturedTrace {
    pub(crate) events: Vec<CapturedEvent>,
    pub(crate) spans: Vec<CapturedSpan>,
}

#[derive(Debug, Default)]
struct RecordingLayer {
    trace: Arc<Mutex<CapturedTrace>>,
}

#[derive(Default)]
struct FieldVisitor {
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

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attributes: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        attributes.record(&mut visitor);
        let metadata = attributes.metadata();
        if let Ok(mut trace) = self.trace.lock() {
            trace.spans.push(CapturedSpan {
                level: *metadata.level(),
                target: metadata.target().to_owned(),
                name: metadata.name().to_owned(),
                fields: visitor.fields,
            });
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        // `event_scope` already resolves contextual, explicitly parented and
        // root events, so no `is_contextual` gate is needed: gating would
        // record an empty path for an event whose parent is known.
        let span_path = ctx
            .event_scope(event)
            .map(|scope| {
                scope
                    .from_root()
                    .map(|span| span.name().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Ok(mut trace) = self.trace.lock() {
            trace.events.push(CapturedEvent {
                level: *event.metadata().level(),
                target: event.metadata().target().to_owned(),
                fields: visitor.fields,
                span_path,
            });
        }
    }
}

/// Keeps a second dispatcher registered for the lifetime of the process.
///
/// `tracing_core` optimises its callsite-interest rebuilds while exactly one
/// dispatcher is registered: instead of unioning the interest of every live
/// dispatcher, it asks only the rebuilding thread's current dispatcher. When a
/// daemon test thread performs that rebuild, the answer comes from the global
/// telemetry subscriber, which filters dispatch events out and so marks the
/// callsites `never`. Events emitted later while a recording dispatcher is
/// installed are then discarded before any subscriber sees them, which makes
/// capture assertions fail intermittently depending on test scheduling.
///
/// Holding a second dispatcher alive keeps the dispatcher count above one, so
/// every rebuild takes the general path and folds in the recording layer's
/// interest as well.
static SECOND_DISPATCHER: LazyLock<Dispatch> = LazyLock::new(|| Dispatch::new(Registry::default()));

/// Runs `action` while recording all emitted tracing events.
///
/// Use this helper when asserting on dispatch logging without wiring a bespoke
/// subscriber in each test. The recording layer is installed as a scoped
/// dispatcher rather than the process-wide default, so parallel tests (and the
/// daemon's own telemetry initialization) never contend for the global
/// dispatcher.
///
/// **Scope:** `dispatcher::with_default` sets a thread-local default, so only
/// events emitted on the calling thread are captured. A thread spawned inside
/// `action` does not inherit it and falls back to the global dispatcher. A test
/// that needs events from a spawned thread must propagate the dispatcher
/// explicitly, as [`harness`](crate::dispatch::handler::tests_helpers::harness)
/// does with `dispatcher::get_default` before the spawn and
/// `dispatcher::set_default` inside the closure.
///
/// # Examples
///
/// ```ignore
/// let events = capture_events(|| {
///     tracing::info!(target: "dispatch", event = "ready", "structured dispatch event");
/// });
/// assert_eq!(events.len(), 1);
/// ```
pub(crate) fn capture_events(action: impl FnOnce()) -> Vec<CapturedEvent> {
    capture_trace(action).events
}

/// Runs `action` while recording every emitted event and created span.
///
/// This is the full-fidelity sibling of [`capture_events`], for tests that
/// assert on instrumentation rather than on a single event. See
/// [`capture_events`] for the dispatcher scope and its consequences.
pub(crate) fn capture_trace(action: impl FnOnce()) -> CapturedTrace {
    LazyLock::force(&SECOND_DISPATCHER);
    let trace = Arc::new(Mutex::new(CapturedTrace::default()));
    let subscriber = Registry::default().with(RecordingLayer {
        trace: Arc::clone(&trace),
    });
    dispatcher::with_default(&Dispatch::new(subscriber), action);
    let mut recorded = trace
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::mem::take(&mut *recorded)
}

/// Emits the callsite that [`second_dispatcher_keeps_a_callsite_registered_under_capture`]
/// protects.
///
/// Keeping the emission in one function matters: each `tracing` macro
/// invocation is its own callsite, so two invocations would register two
/// callsites and never exercise the register-then-reuse sequence the workaround
/// depends on.
fn emit_interest_probe() {
    tracing::info!(target: "dispatch", event = "interest_probe", "interest probe");
}

/// Proves that [`SECOND_DISPATCHER`] is what keeps concurrent capture working.
///
/// The probe callsite is registered for the first time while the thread-local
/// default is [`Dispatch::none`], which reproduces the daemon-test scheduling
/// that motivated the workaround. With `SECOND_DISPATCHER` alive,
/// `tracing_core` folds every registered dispatcher into the interest rebuild,
/// so the callsite stays recordable; without it, the rebuild consults only the
/// no-op thread default and marks the callsite `never`, which makes the second
/// emission below unobservable. Removing the workaround therefore fails this
/// test rather than only making other tests flaky.
#[test]
fn second_dispatcher_keeps_a_callsite_registered_under_capture() {
    let events = capture_events(|| {
        dispatcher::with_default(&Dispatch::none(), emit_interest_probe);
        emit_interest_probe();
    });

    let captured = events.iter().any(|event| {
        event.target == "dispatch"
            && event
                .fields
                .get("event")
                .is_some_and(|value| value == "interest_probe")
    });

    assert!(
        captured,
        "expected the emission through the recording dispatcher to be captured"
    );
}

/// Emits one event nested, explicitly parented, and at the root.
///
/// The three forms are separate macro invocations because each is its own
/// callsite; sharing one helper keeps the surrounding test readable.
fn emit_nested_event() {
    tracing::info!(target: "capture", event = "nested", "nested");
}

fn emit_explicitly_parented_event(parent: &tracing::Span) {
    tracing::info!(target: "capture", parent: parent, event = "parented", "parented");
}

fn emit_root_event() {
    tracing::info!(target: "capture", parent: None, event = "root", "root");
}

/// Proves a captured event records the parent it was given, however it named it.
///
/// An explicitly parented event is not contextual, so recording its scope only
/// for contextual events would leave its path empty and make a future test
/// report sound instrumentation as missing.
#[rstest::rstest]
#[case::nested("nested", &["outer"])]
#[case::explicitly_parented("parented", &["outer"])]
#[case::root("root", &[])]
fn captured_event_records_its_enclosing_spans(
    #[case] event_name: &str,
    #[case] expected_path: &[&str],
) {
    let trace = capture_trace(|| {
        emit_root_event();
        let outer = tracing::info_span!(target: "capture", "outer");
        let _guard = outer.enter();
        emit_nested_event();
        emit_explicitly_parented_event(&outer);
    });

    let event = trace
        .events
        .iter()
        .find(|event| {
            event
                .fields
                .get("event")
                .is_some_and(|name| name == event_name)
        })
        .unwrap_or_else(|| panic!("expected a `{event_name}` event"));

    let expected = expected_path
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(event.span_path, expected, "span path for `{event_name}`");
}
