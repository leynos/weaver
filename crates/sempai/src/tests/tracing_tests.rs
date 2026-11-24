//! Tracing coverage tests for the `sempai` engine pipeline.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        Mutex,
        MutexGuard,
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread::ThreadId,
};

use tracing::{
    Event,
    Level,
    Metadata,
    Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    subscriber::Interest,
};

use crate::{Engine, EngineConfig};

struct SpanCountingSubscriber {
    state: Arc<TraceCaptureState>,
    next_id: AtomicU64,
}

static TRACE_SUBSCRIBER_INSTALLATION: OnceLock<Result<(), String>> = OnceLock::new();

struct TraceCaptureState {
    capture: Mutex<TraceCapture>,
    capture_lock: Mutex<()>,
}

impl TraceCaptureState {
    fn shared() -> &'static Arc<Self> {
        static STATE: OnceLock<Arc<TraceCaptureState>> = OnceLock::new();
        STATE.get_or_init(|| {
            Arc::new(Self {
                capture: Mutex::default(),
                capture_lock: Mutex::default(),
            })
        })
    }

    fn install_global_subscriber(state: &Arc<Self>) -> Result<(), String> {
        // The registry is global, but each guard serialises and resets its
        // capture state so the test cannot leak observations to another test.
        TRACE_SUBSCRIBER_INSTALLATION
            .get_or_init(|| {
                let subscriber = SpanCountingSubscriber {
                    state: Arc::clone(state),
                    next_id: AtomicU64::default(),
                };
                tracing::subscriber::set_global_default(subscriber)
                    .map_err(|error| error.to_string())
            })
            .clone()
    }

    fn begin_capture(&self) -> TraceCaptureGuard<'_> {
        let capture_lock = self
            .capture_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut capture = self
            .capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *capture = TraceCapture {
            trace_thread_id: Some(std::thread::current().id()),
            ..TraceCapture::default()
        };
        TraceCaptureGuard {
            state: self,
            _capture_lock: capture_lock,
        }
    }

    fn records_current_thread(&self) -> bool {
        self.capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .trace_thread_id
            == Some(std::thread::current().id())
    }
}

#[derive(Default)]
struct TraceCapture {
    trace_thread_id: Option<ThreadId>,
    compile_yaml_spans: usize,
    debug_events: Vec<RecordedEvent>,
}

#[derive(Debug)]
struct TraceCaptureSnapshot {
    compile_yaml_spans: usize,
    debug_events: Vec<RecordedEvent>,
}

struct TraceCaptureGuard<'a> {
    state: &'a TraceCaptureState,
    _capture_lock: MutexGuard<'a, ()>,
}

impl TraceCaptureGuard<'_> {
    fn snapshot(&self) -> TraceCaptureSnapshot {
        let capture = self
            .state
            .capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        TraceCaptureSnapshot {
            compile_yaml_spans: capture.compile_yaml_spans,
            debug_events: capture.debug_events.clone(),
        }
    }
}

impl Drop for TraceCaptureGuard<'_> {
    fn drop(&mut self) {
        *self
            .state
            .capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = TraceCapture::default();
    }
}

impl Subscriber for SpanCountingSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.state.records_current_thread() && metadata.target().starts_with("sempai")
    }

    fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let metadata = attributes.metadata();
        let mut capture = self
            .state
            .capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if capture.trace_thread_id == Some(std::thread::current().id())
            && is_compile_yaml_span(metadata)
        {
            capture.compile_yaml_spans += 1;
        }
        Id::from_u64(self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if event.metadata().level() != &Level::DEBUG {
            return;
        }
        let mut fields = FieldRecorder::default();
        event.record(&mut fields);
        let mut capture = self
            .state
            .capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if capture.trace_thread_id == Some(std::thread::current().id()) {
            capture.debug_events.push(RecordedEvent {
                target: event.metadata().target().to_owned(),
                fields: fields.into_fields(),
            });
        }
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

fn is_compile_yaml_span(metadata: &Metadata<'_>) -> bool {
    metadata.name() == "compile_yaml" && metadata.target() == "sempai::engine"
}

#[derive(Debug, Default)]
struct FieldRecorder {
    fields: BTreeMap<String, String>,
}

impl FieldRecorder {
    fn into_fields(self) -> BTreeMap<String, String> { self.fields }
}

impl Visit for FieldRecorder {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

#[derive(Debug, Clone)]
struct RecordedEvent {
    target: String,
    fields: BTreeMap<String, String>,
}

impl RecordedEvent {
    fn message(&self) -> Option<&str> { self.fields.get("message").map(String::as_str) }

    fn field(&self, name: &str) -> Option<&str> { self.fields.get(name).map(String::as_str) }
}

#[test]
fn compile_yaml_emits_observable_compile_span() {
    let yaml = concat!(
        "rules:\n",
        "  - id: demo.span\n",
        "    message: span coverage\n",
        "    languages: [rust]\n",
        "    severity: ERROR\n",
        "    pattern: foo($X)\n",
    );
    let state = TraceCaptureState::shared();
    TraceCaptureState::install_global_subscriber(state)
        .expect("tracing subscriber must not already be configured");
    let capture = state.begin_capture();
    let engine = Engine::new(EngineConfig::default());
    engine
        .compile_yaml(yaml)
        .expect("valid rule should compile");

    let trace = capture.snapshot();
    assert_eq!(
        trace.compile_yaml_spans, 1,
        "expected compile_yaml to create exactly one observable tracing span",
    );
    assert_compile_yaml_debug_events(&trace.debug_events);
}

fn assert_compile_yaml_debug_events(recorded_events: &[RecordedEvent]) {
    assert_event(
        recorded_events,
        "sempai::engine",
        "yaml parsed successfully",
        &[("rules", FieldMatcher::Exact("1"))],
    );
    assert_event(
        recorded_events,
        "sempai::engine",
        "principal normalized",
        &[("rule_id", FieldMatcher::Exact("demo.span"))],
    );
    assert_event(
        recorded_events,
        "sempai::semantic_check",
        "semantic validation passed",
        &[("source_span", FieldMatcher::StartsWith("Some("))],
    );
    assert_event(recorded_events, "sempai::engine", "query plan created", &[]);
}

#[derive(Debug, Clone, Copy)]
enum FieldMatcher<'a> {
    Exact(&'a str),
    StartsWith(&'a str),
}

/// Pure query: does a single recorded event match the target, message, and
/// every expected field?
fn event_matches(
    event: &RecordedEvent,
    target: &str,
    message: &str,
    expected_fields: &[(&str, FieldMatcher<'_>)],
) -> bool {
    event.target == target
        && event.message() == Some(message)
        && expected_fields.iter().all(|(field, value)| {
            event.field(field).is_some_and(|actual| match value {
                FieldMatcher::Exact(expected) => actual == *expected,
                FieldMatcher::StartsWith(expected) => actual.starts_with(expected),
            })
        })
}

/// Searches `debug_events` for one matching `target`/`message`/fields.
fn find_matching_event<'a>(
    debug_events: &'a [RecordedEvent],
    target: &str,
    message: &str,
    expected_fields: &[(&str, FieldMatcher<'_>)],
) -> Option<&'a RecordedEvent> {
    debug_events
        .iter()
        .find(|event| event_matches(event, target, message, expected_fields))
}

fn assert_event(
    debug_events: &[RecordedEvent],
    target: &str,
    message: &str,
    expected_fields: &[(&str, FieldMatcher<'_>)],
) {
    assert!(
        find_matching_event(debug_events, target, message, expected_fields).is_some(),
        "expected debug event `{message}` with fields {expected_fields:?}, got {debug_events:?}",
    );
}
