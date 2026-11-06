//! Serialization tests for the structured dispatch events.
//!
//! Operator-facing events must never carry request payloads, patches, bodies,
//! or environment data. These tests pin both the emitted field set and the
//! redaction behaviour of the structured-event builders.

use super::*;
use crate::dispatch::handler::structured_event::{
    format_structured_event,
    serialize_structured_event,
};

#[test]
fn serialize_structured_dispatch_event_omits_sensitive_fields() {
    let temp_dir = std::env::temp_dir();
    let endpoint = temp_dir.join("weaverd.sock");
    let event = StructuredDispatchEvent::new(
        "dispatching_request",
        endpoint.to_string_lossy().to_string(),
        &temp_dir,
        StructuredEventMetadata::new("observe", "get-card").with_size(42),
    );
    let value = serialize_structured_event(&event);

    assert_eq!(
        value.get("event").and_then(serde_json::Value::as_str),
        Some("dispatching_request")
    );
    assert!(value.get("patch").is_none());
    assert!(value.get("body").is_none());
    assert!(value.get("source").is_none());
    assert!(value.get("env").is_none());
    assert!(value.get("fullPayload").is_none());
    assert_eq!(value.get("size"), Some(&serde_json::json!(42)));
    assert_eq!(
        value.get("runtime_dir"),
        Some(&serde_json::json!(temp_dir.to_string_lossy().to_string()))
    );
    assert_eq!(
        value.get("weaverd.health"),
        Some(&serde_json::json!(
            temp_dir
                .join("weaverd.health")
                .to_string_lossy()
                .to_string()
        ))
    );
}

#[test]
fn serialize_dispatching_request_event_snapshot() {
    let event = StructuredDispatchEvent::new(
        "dispatching_request",
        "tcp://127.0.0.1:9779",
        std::path::Path::new("/run/user/1000/weaver"),
        StructuredEventMetadata::new("observe", "get-card").with_size(128),
    );
    insta::with_settings!({
        redactions => vec![
            (".endpoint", "[endpoint]".into()),
            (".runtime_dir", "[runtime_dir]".into()),
            ("[\"weaverd.health\"]", "[health_path]".into()),
        ]
    }, {
        insta::assert_json_snapshot!(
            serde_json::from_str::<serde_json::Value>(
                &format_structured_event(&event)
            ).expect("structured event should serialize as JSON")
        );
    });
}

#[test]
fn serialize_request_too_large_event_snapshot() {
    let mut event = StructuredDispatchEvent::new(
        "request_too_large",
        "tcp://127.0.0.1:9779",
        std::path::Path::new("/run/user/1000/weaver"),
        StructuredEventMetadata::none()
            .with_size(2048)
            .with_max_size(1024),
    );
    event.patch = Some("--- a/foo.rs\n+++ b/foo.rs".to_owned());
    event.body = Some("some body".to_owned());
    event.source = Some("fn main() {}".to_owned());
    event.env = Some("SECRET=value".to_owned());
    event.full_payload = Some("{\"command\":\"redacted\"}".to_owned());
    insta::with_settings!({
        redactions => vec![
            (".endpoint", "[endpoint]".into()),
            (".runtime_dir", "[runtime_dir]".into()),
            ("[\"weaverd.health\"]", "[health_path]".into()),
        ]
    }, {
        insta::assert_json_snapshot!(
            serde_json::from_str::<serde_json::Value>(
                &format_structured_event(&event)
            ).expect("structured event should serialize as JSON")
        );
    });
}

#[test]
fn emit_structured_event_returns_payload_without_sensitive_request_data() {
    let temp_dir = std::env::temp_dir();
    let endpoint = temp_dir.join("weaverd.sock");
    let mut event = StructuredDispatchEvent::new(
        "request_too_large",
        endpoint.to_string_lossy().to_string(),
        &temp_dir,
        StructuredEventMetadata::new("observe", "apply-patch")
            .with_size(JSONL_REQUEST_MAX_LINE_BYTES + 1)
            .with_max_size(JSONL_REQUEST_MAX_LINE_BYTES),
    );
    event.patch = Some("sensitive patch".to_owned());
    event.body = Some("sensitive body".to_owned());
    event.source = Some("sensitive source".to_owned());
    event.env = Some("PATH=secret".to_owned());
    event.full_payload = Some("full json payload".to_owned());
    let payload = format_structured_event(&event);
    let value = serde_json::from_str::<serde_json::Value>(&payload)
        .expect("valid structured event payload");

    assert_eq!(
        value.get("size"),
        Some(&serde_json::json!(JSONL_REQUEST_MAX_LINE_BYTES + 1))
    );
    assert_eq!(
        value.get("max_size"),
        Some(&serde_json::json!(JSONL_REQUEST_MAX_LINE_BYTES))
    );
    assert_eq!(value.get("patch"), Some(&serde_json::json!("<redacted>")));
    assert_eq!(value.get("body"), Some(&serde_json::json!("<redacted>")));
    assert_eq!(value.get("source"), Some(&serde_json::json!("<redacted>")));
    assert_eq!(value.get("env"), Some(&serde_json::json!("<redacted>")));
    assert_eq!(
        value.get("fullPayload"),
        Some(&serde_json::json!("<redacted>"))
    );
    assert_eq!(value.get("domain"), Some(&serde_json::json!("observe")));
    let events = capture_events(|| {
        emit_structured_event(&event, "request_too_large rejection", true);
    });
    let emitted = events
        .iter()
        .find(|captured| {
            captured
                .fields
                .get("event")
                .is_some_and(|field_value| field_value == "request_too_large")
        })
        .expect("structured event should be emitted");

    assert_eq!(emitted.level, tracing::Level::ERROR);
    assert_eq!(emitted.target, DISPATCH_TARGET);
    assert_eq!(
        emitted.fields.get("message").map(String::as_str),
        Some("request_too_large rejection")
    );
    let emitted_payload = emitted
        .fields
        .get("payload")
        .expect("structured event should include payload");
    let emitted_value = serde_json::from_str::<serde_json::Value>(emitted_payload)
        .expect("emitted payload should be JSON");
    assert_eq!(
        emitted_value.get("patch"),
        Some(&serde_json::json!("<redacted>"))
    );
}
