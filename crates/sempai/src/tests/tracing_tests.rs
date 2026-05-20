//! Tracing coverage tests for the `sempai` engine pipeline.

use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use tracing::{
    Metadata,
    Subscriber,
    span::{Attributes, Id, Record},
};

use crate::{Engine, EngineConfig};

#[derive(Default)]
struct SpanCountingSubscriber {
    compile_yaml_spans: Arc<AtomicUsize>,
    next_id: AtomicU64,
}

impl SpanCountingSubscriber {
    fn compile_yaml_spans(&self) -> Arc<AtomicUsize> { Arc::clone(&self.compile_yaml_spans) }
}

impl Subscriber for SpanCountingSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool { true }

    fn new_span(&self, attributes: &Attributes<'_>) -> Id {
        if attributes.metadata().name() == "compile_yaml" {
            self.compile_yaml_spans.fetch_add(1, Ordering::Relaxed);
        }
        Id::from_u64(self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &tracing::Event<'_>) {}

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
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
}
