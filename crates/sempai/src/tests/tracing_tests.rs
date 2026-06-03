//! Tracing coverage tests for the `sempai` engine pipeline.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use tracing::{
    Event,
    Level,
    Metadata,
    Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

use crate::{Engine, EngineConfig};

#[derive(Default)]
struct SpanCountingSubscriber {
    compile_yaml_spans: Arc<AtomicUsize>,
    debug_events: Arc<Mutex<Vec<RecordedEvent>>>,
    next_id: AtomicU64,
}

impl SpanCountingSubscriber {
    fn compile_yaml_spans(&self) -> Arc<AtomicUsize> { Arc::clone(&self.compile_yaml_spans) }

    fn debug_events(&self) -> Arc<Mutex<Vec<RecordedEvent>>> { Arc::clone(&self.debug_events) }
}

impl Subscriber for SpanCountingSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool { metadata.target().starts_with("sempai") }

    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        let metadata = attributes.metadata();
        if metadata.name() == "compile_yaml" && metadata.target() == "sempai::engine" {
            self.compile_yaml_spans.fetch_add(1, Ordering::Relaxed);
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
        self.debug_events
            .lock()
            .expect("debug event storage should not be poisoned")
            .push(RecordedEvent {
                target: event.metadata().target().to_owned(),
                fields: fields.into_fields(),
            });
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
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

#[derive(Debug)]
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
    let subscriber = SpanCountingSubscriber::default();
    let compile_yaml_spans = subscriber.compile_yaml_spans();
    let debug_events = subscriber.debug_events();

    tracing::subscriber::with_default(subscriber, || {
        let engine = Engine::new(EngineConfig::default());
        engine
            .compile_yaml(yaml)
            .expect("valid rule should compile");
    });

    assert!(
        compile_yaml_spans.load(Ordering::Relaxed) >= 1,
        "expected compile_yaml to create an observable tracing span",
    );
    assert_compile_yaml_debug_events(&debug_events);
}

fn assert_compile_yaml_debug_events(debug_events: &Arc<Mutex<Vec<RecordedEvent>>>) {
    let recorded_events = debug_events
        .lock()
        .expect("debug event storage should not be poisoned");
    assert_event(
        &recorded_events,
        "sempai::engine",
        "yaml parsed successfully",
        &[("rules", "1")],
    );
    assert_event(
        &recorded_events,
        "sempai::engine",
        "principal normalized",
        &[("rule_id", "demo.span")],
    );
    assert_event(
        &recorded_events,
        "sempai::semantic_check",
        "semantic validation passed",
        &[("span", "Some(")],
    );
    assert_event(
        &recorded_events,
        "sempai::engine",
        "query plan created",
        &[("rule_id", "demo.span"), ("language", "rust")],
    );
}

fn assert_event(
    debug_events: &[RecordedEvent],
    target: &str,
    message: &str,
    expected_fields: &[(&str, &str)],
) {
    assert!(
        debug_events.iter().any(|event| {
            event.target == target
                && event.message() == Some(message)
                && expected_fields.iter().all(|(field, value)| {
                    event
                        .field(field)
                        .is_some_and(|actual| actual == *value || actual.starts_with(value))
                })
        }),
        "expected debug event `{message}` with fields {expected_fields:?}, got {debug_events:?}",
    );
}
