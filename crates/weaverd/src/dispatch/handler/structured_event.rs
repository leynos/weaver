//! Structured event metadata and serialization for dispatch logging.
//!
//! The dispatch handler uses these helpers to build tracing payloads for
//! request failures and other notable states. Sensitive request data is
//! redacted before serialization so the emitted logs remain safe to forward.

use std::path::Path;

use serde_json::{Map, Value, json};
use tracing::{error, info};

use crate::dispatch::{errors::DispatchError, router::DISPATCH_TARGET};

/// Placeholder written in place of every sensitive field value.
const REDACTION_MARKER: &str = "<redacted>";
/// File name of the health snapshot recorded next to the runtime directory.
const HEALTH_SNAPSHOT_NAME: &str = "weaverd.health";

/// Metadata fields attached to a structured dispatch event.
#[derive(Debug)]
pub(super) struct StructuredEventMetadata {
    /// Dispatch domain name, omitted when the request could not be parsed.
    domain: Option<String>,
    /// Operation name within the domain, omitted when it is unknown.
    operation: Option<String>,
    /// Observed request size in bytes, when known.
    size: Option<usize>,
    /// Configured request size limit in bytes, set only for size rejections.
    max_size: Option<usize>,
}

impl StructuredEventMetadata {
    /// Creates metadata with every optional field absent.
    pub(super) const fn none() -> Self {
        Self {
            domain: None,
            operation: None,
            size: None,
            max_size: None,
        }
    }

    /// Creates metadata carrying the domain and operation it is given.
    pub(super) fn new(domain: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            domain: Some(domain.into()),
            operation: Some(operation.into()),
            size: None,
            max_size: None,
        }
    }

    /// Records the observed request size in bytes.
    pub(super) const fn with_size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    /// Records the configured request size limit in bytes.
    pub(super) const fn with_max_size(mut self, max_size: usize) -> Self {
        self.max_size = Some(max_size);
        self
    }

    /// Copies the present metadata fields into a JSON payload object.
    pub(super) fn extend_payload(&self, payload: &mut serde_json::Map<String, serde_json::Value>) {
        if let Some(domain) = &self.domain {
            payload.insert("domain".into(), json!(domain));
        }
        if let Some(operation) = &self.operation {
            payload.insert("operation".into(), json!(operation));
        }
        if let Some(size) = self.size {
            payload.insert("size".into(), json!(size));
        }
        if let Some(max_size) = self.max_size {
            payload.insert("max_size".into(), json!(max_size));
        }
    }
}

/// Structured event payload assembled by the dispatch handler.
#[derive(Debug)]
pub(super) struct StructuredDispatchEvent {
    /// Stable event name emitted in the structured log line.
    event: &'static str,
    /// Endpoint that received the request.
    endpoint: String,
    /// Runtime directory whose health snapshot accompanies the event.
    runtime_dir: std::path::PathBuf,
    /// Optional request metadata attached to the event.
    metadata: StructuredEventMetadata,
    /// Request patch text, retained only to mark it redacted.
    pub(super) patch: Option<String>,
    /// Request body text, retained only to mark it redacted.
    pub(super) body: Option<String>,
    /// Source text referenced by the request, retained only to mark it redacted.
    pub(super) source: Option<String>,
    /// Environment snapshot, retained only to mark it redacted.
    pub(super) env: Option<String>,
    /// Full request payload, retained only to mark it redacted.
    pub(super) full_payload: Option<String>,
}

impl StructuredDispatchEvent {
    /// Creates an event with no sensitive payload fields attached.
    pub(super) fn new(
        event: &'static str,
        endpoint: impl Into<String>,
        runtime_dir: &Path,
        metadata: StructuredEventMetadata,
    ) -> Self {
        Self {
            event,
            endpoint: endpoint.into(),
            runtime_dir: runtime_dir.to_path_buf(),
            metadata,
            patch: None,
            body: None,
            source: None,
            env: None,
            full_payload: None,
        }
    }
}

/// Maps a dispatch error to the structured event shape used by the handler.
pub(super) fn read_error_event(
    error: &DispatchError,
    endpoint: &str,
    runtime_dir: &Path,
) -> StructuredDispatchEvent {
    match error {
        DispatchError::RequestTooLarge { size, max_size } => StructuredDispatchEvent::new(
            "request_too_large",
            endpoint,
            runtime_dir,
            StructuredEventMetadata::none()
                .with_size(*size)
                .with_max_size(*max_size),
        ),
        _ => StructuredDispatchEvent::new(
            "request_rejected",
            endpoint,
            runtime_dir,
            StructuredEventMetadata::none(),
        ),
    }
}

/// Replaces a present sensitive value with the redaction marker.
///
/// The original value is never serialized; only its presence is preserved so
/// consumers can tell that a field was supplied.
fn redacted(value: Option<&String>) -> Option<Value> {
    value.map(|_| Value::String(REDACTION_MARKER.to_owned()))
}

/// Serializes a structured event into JSON with sensitive fields redacted.
///
/// The handler stores request bodies, patches, source text, environment
/// snapshots, and full payloads behind a stable redaction marker instead of
/// logging the raw values.
pub(super) fn serialize_structured_event(event: &StructuredDispatchEvent) -> Value {
    let mut payload = Map::<String, Value>::new();
    payload.insert("event".into(), json!(event.event));
    payload.insert("endpoint".into(), json!(event.endpoint));
    payload.insert(
        "runtime_dir".into(),
        json!(event.runtime_dir.to_string_lossy().to_string()),
    );
    payload.insert(
        "weaverd.health".into(),
        json!(
            event
                .runtime_dir
                .join(HEALTH_SNAPSHOT_NAME)
                .to_string_lossy()
                .to_string()
        ),
    );

    event.metadata.extend_payload(&mut payload);
    let redacted_fields: &[(&str, &Option<String>)] = &[
        ("patch", &event.patch),
        ("body", &event.body),
        ("source", &event.source),
        ("env", &event.env),
        ("fullPayload", &event.full_payload),
    ];
    for (key, value) in redacted_fields {
        if let Some(redacted) = redacted(value.as_ref()) {
            payload.insert((*key).into(), redacted);
        }
    }

    Value::Object(payload)
}

/// Renders a structured event as compact JSON text for tracing.
pub(super) fn format_structured_event(event: &StructuredDispatchEvent) -> String {
    serialize_structured_event(event).to_string()
}

/// Emits the structured dispatch event through tracing on the dispatch target.
///
/// The handler uses `info!` for normal structured events and `error!` for
/// failure paths so downstream consumers can distinguish severity.
pub(super) fn emit_structured_event(
    event: &StructuredDispatchEvent,
    message: &str,
    is_error: bool,
) {
    let payload = format_structured_event(event);
    if is_error {
        error!(
            target: DISPATCH_TARGET,
            event = %event.event,
            message = %message,
            payload = %payload,
            "structured dispatch event"
        );
    } else {
        info!(
            target: DISPATCH_TARGET,
            event = %event.event,
            message = %message,
            payload = %payload,
            "structured dispatch event"
        );
    }
}
